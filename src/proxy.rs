//! Proxy-aware connection setup for the WebSocket streams.
//!
//! REST traffic gets its proxy from `reqwest`; `tokio-tungstenite` dials TCP
//! directly, so the market and account streams use [`connect_ws`] instead of
//! `connect_async`. Both proxy kinds hand back the same [`TcpStream`] once
//! their handshake is done, so TLS and the WebSocket upgrade then run over it
//! exactly as they would over a direct connection.
//!
//! Routing follows the REST client: the `[proxy]` preferences win, then the
//! `ALL_PROXY` / `HTTPS_PROXY` / `HTTP_PROXY` / `NO_PROXY` environment
//! variables, then a direct connection.
//!
//! Supported proxies: `http://` (HTTP CONNECT, optional basic auth),
//! `socks5://` (local DNS) and `socks5h://` (proxy-side DNS), both with
//! optional username/password. `https://` proxies are rejected rather than
//! silently bypassed.
use std::net::IpAddr;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use hyper_util::client::proxy::matcher::Matcher;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::http::{header::HeaderValue, Uri};
use tokio_tungstenite::{client_async_tls, MaybeTlsStream, WebSocketStream};

use crate::prefs::ProxySection;

/// A connected, upgraded WebSocket — the same type `connect_async` returns.
pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// How long the TCP connect plus proxy handshake may take before giving up.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

/// Upper bound on the proxy's CONNECT response head.
const MAX_CONNECT_HEAD: usize = 8 * 1024;

/// Which proxy, if any, a WebSocket connection should go through.
pub struct ProxyRules {
    matcher: Option<Matcher>,
    /// A proxy URL was configured but is unusable. Connecting directly would
    /// silently bypass a required proxy, so every connect fails with this
    /// message instead.
    config_error: Option<String>,
}

impl ProxyRules {
    /// Never use a proxy. Used by tests so they do not depend on the environment.
    pub fn direct() -> Self {
        Self {
            matcher: None,
            config_error: None,
        }
    }

    /// Preferences first (`socks5` wins over `http`), then the environment.
    pub fn from_prefs_and_env(cfg: &ProxySection) -> Self {
        let non_empty = |s: &Option<String>| s.clone().filter(|v| !v.trim().is_empty());
        let Some(url) = non_empty(&cfg.socks5).or_else(|| non_empty(&cfg.http)) else {
            return Self {
                matcher: Some(Matcher::from_env()),
                config_error: None,
            };
        };
        let url = url.trim().to_string();
        // The matcher silently drops URLs it does not understand, which would
        // send traffic direct, so check the URL here.
        let config_error = match url.parse::<Uri>() {
            Ok(u) if u.host().is_none() => Some("invalid proxy URL in preferences".to_string()),
            Ok(u) => match u.scheme_str() {
                Some("http" | "https" | "socks5" | "socks5h") => None,
                Some(other) => Some(format!("unsupported proxy scheme {other:?} in preferences")),
                None => Some("invalid proxy URL in preferences".to_string()),
            },
            Err(_) => Some("invalid proxy URL in preferences".to_string()),
        };
        let mut builder = Matcher::builder().all(url);
        if let Some(no_proxy) = non_empty(&cfg.no_proxy) {
            builder = builder.no(no_proxy);
        }
        Self {
            matcher: Some(builder.build()),
            config_error,
        }
    }

    fn route(&self, wss: bool, host: &str) -> Result<Route> {
        if let Some(msg) = &self.config_error {
            bail!("{msg}");
        }
        let Some(matcher) = &self.matcher else {
            return Ok(Route::Direct);
        };
        // The matcher only understands http(s) destinations.
        let dst: Uri = format!("{}://{host}", if wss { "https" } else { "http" })
            .parse()
            .context("invalid WebSocket host")?;
        let Some(hit) = matcher.intercept(&dst) else {
            return Ok(Route::Direct);
        };

        let uri = hit.uri();
        let proxy_host = uri.host().context("proxy URL has no host")?;
        let kind = match uri.scheme_str() {
            Some("http") => Kind::HttpConnect {
                auth: hit.basic_auth().cloned(),
            },
            Some(s @ ("socks5" | "socks5h")) => Kind::Socks5 {
                auth: hit.raw_auth().map(|(u, p)| (u.to_string(), p.to_string())),
                remote_dns: s == "socks5h",
            },
            Some("https") => bail!(
                "https:// proxies are not supported for WebSocket streams; \
                 use an http:// or socks5:// proxy URL"
            ),
            other => bail!("unsupported proxy scheme {other:?}"),
        };
        let default_port = if matches!(kind, Kind::HttpConnect { .. }) {
            80
        } else {
            1080
        };
        Ok(Route::Proxy {
            addr: format!("{proxy_host}:{}", uri.port_u16().unwrap_or(default_port)),
            kind,
        })
    }
}

/// Preferences that send loopback traffic direct without reading the
/// environment, so stream tests do not depend on a developer's `HTTP_PROXY`.
#[cfg(test)]
pub(crate) fn hermetic_prefs() -> crate::prefs::AppPrefs {
    crate::prefs::AppPrefs {
        proxy: ProxySection {
            http: Some("http://127.0.0.1:1".into()),
            no_proxy: Some("127.0.0.1".into()),
            ..Default::default()
        },
        ..Default::default()
    }
}

enum Route {
    Direct,
    Proxy { addr: String, kind: Kind },
}

enum Kind {
    HttpConnect {
        auth: Option<HeaderValue>,
    },
    Socks5 {
        auth: Option<(String, String)>,
        remote_dns: bool,
    },
}

/// Open a WebSocket to `url`, through a proxy when `rules` selects one.
pub async fn connect_ws(url: &str, rules: &ProxyRules) -> Result<WsStream> {
    connect_ws_with_timeout(url, rules, HANDSHAKE_TIMEOUT).await
}

async fn connect_ws_with_timeout(
    url: &str,
    rules: &ProxyRules,
    timeout: Duration,
) -> Result<WsStream> {
    let uri: Uri = url.parse().context("invalid WebSocket URL")?;
    let host = uri.host().context("WebSocket URL has no host")?;
    let wss = uri.scheme_str() == Some("wss");
    let port = uri.port_u16().unwrap_or(if wss { 443 } else { 80 });

    let Route::Proxy { addr, kind } = rules.route(wss, host)? else {
        let (ws, _) = tokio_tungstenite::connect_async(url).await?;
        return Ok(ws);
    };

    // Log the proxy host only: the URL may embed credentials.
    tracing::info!(proxy = %addr, "connecting WebSocket through proxy");
    let tcp = tokio::time::timeout(timeout, async {
        let mut tcp = TcpStream::connect(&addr)
            .await
            .context("could not reach proxy")?;
        match &kind {
            Kind::HttpConnect { auth } => {
                http_connect(&mut tcp, &format!("{host}:{port}"), auth.as_ref()).await?
            }
            Kind::Socks5 { auth, remote_dns } => {
                socks5_connect(
                    &mut tcp,
                    host,
                    port,
                    auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
                    *remote_dns,
                )
                .await?
            }
        }
        anyhow::Ok(tcp)
    })
    .await
    .context("timed out connecting through proxy")??;

    let (ws, _) = client_async_tls(url, tcp).await?;
    Ok(ws)
}

/// Establish a tunnel with `CONNECT authority`, optionally authenticated.
async fn http_connect(
    tcp: &mut TcpStream,
    authority: &str,
    auth: Option<&HeaderValue>,
) -> Result<()> {
    let mut req = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n");
    if let Some(value) = auth {
        req.push_str(&format!(
            "Proxy-Authorization: {}\r\n",
            value.to_str().context("invalid proxy credentials")?
        ));
    }
    req.push_str("\r\n");
    tcp.write_all(req.as_bytes()).await?;

    // Read byte by byte so nothing past the blank line (the start of the TLS
    // handshake) is consumed.
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() >= MAX_CONNECT_HEAD {
            bail!("proxy CONNECT response too large");
        }
        if tcp.read(&mut byte).await? == 0 {
            bail!("proxy closed the connection during CONNECT");
        }
        head.push(byte[0]);
    }
    let status_line = head.split(|&b| b == b'\r').next().unwrap_or_default();
    let status = String::from_utf8_lossy(status_line);
    let code = status.split_whitespace().nth(1).unwrap_or("");
    if !code.starts_with('2') {
        bail!(
            "proxy refused CONNECT: {}{}",
            status.trim(),
            if code == "407" {
                " (check proxy credentials)"
            } else {
                ""
            }
        );
    }
    Ok(())
}

/// SOCKS5 (RFC 1928) CONNECT with optional username/password (RFC 1929).
///
/// With `remote_dns` the hostname is sent to the proxy to resolve; otherwise
/// it is resolved here and the IP address is sent.
async fn socks5_connect(
    tcp: &mut TcpStream,
    host: &str,
    port: u16,
    auth: Option<(&str, &str)>,
    remote_dns: bool,
) -> Result<()> {
    // Method negotiation.
    let methods: &[u8] = if auth.is_some() {
        &[0x00, 0x02]
    } else {
        &[0x00]
    };
    let mut greeting = vec![0x05, methods.len() as u8];
    greeting.extend_from_slice(methods);
    tcp.write_all(&greeting).await?;

    let mut reply = [0u8; 2];
    tcp.read_exact(&mut reply).await?;
    if reply[0] != 0x05 {
        bail!("proxy is not a SOCKS5 server");
    }
    match reply[1] {
        0x00 => {}
        0x02 => {
            let (user, pass) = auth.context("SOCKS5 proxy requires credentials")?;
            if user.len() > 255 || pass.len() > 255 {
                bail!("SOCKS5 credentials too long");
            }
            let mut msg = vec![0x01, user.len() as u8];
            msg.extend_from_slice(user.as_bytes());
            msg.push(pass.len() as u8);
            msg.extend_from_slice(pass.as_bytes());
            tcp.write_all(&msg).await?;
            let mut status = [0u8; 2];
            tcp.read_exact(&mut status).await?;
            if status[1] != 0x00 {
                bail!("SOCKS5 authentication failed (check proxy credentials)");
            }
        }
        0xFF => bail!("SOCKS5 proxy accepts none of our authentication methods"),
        other => bail!("SOCKS5 proxy chose unsupported method {other:#04x}"),
    }

    // CONNECT request.
    let mut req = vec![0x05, 0x01, 0x00];
    let literal = host
        .trim_matches(|c| c == '[' || c == ']')
        .parse::<IpAddr>();
    let ip = match literal {
        Ok(ip) => Some(ip),
        Err(_) if remote_dns => None,
        Err(_) => Some(
            tokio::net::lookup_host((host, port))
                .await
                .context("could not resolve host")?
                .next()
                .context("host resolved to no addresses")?
                .ip(),
        ),
    };
    match ip {
        Some(IpAddr::V4(v4)) => {
            req.push(0x01);
            req.extend_from_slice(&v4.octets());
        }
        Some(IpAddr::V6(v6)) => {
            req.push(0x04);
            req.extend_from_slice(&v6.octets());
        }
        None => {
            if host.len() > 255 {
                bail!("hostname too long for SOCKS5");
            }
            req.push(0x03);
            req.push(host.len() as u8);
            req.extend_from_slice(host.as_bytes());
        }
    }
    req.extend_from_slice(&port.to_be_bytes());
    tcp.write_all(&req).await?;

    // Reply: VER REP RSV ATYP BND.ADDR BND.PORT
    let mut head = [0u8; 4];
    tcp.read_exact(&mut head).await?;
    if head[1] != 0x00 {
        bail!(
            "SOCKS5 proxy refused the connection: {}",
            socks5_reason(head[1])
        );
    }
    let bound_len = match head[3] {
        0x01 => 4,
        0x04 => 16,
        0x03 => {
            let mut len = [0u8; 1];
            tcp.read_exact(&mut len).await?;
            len[0] as usize
        }
        other => bail!("SOCKS5 reply has unknown address type {other:#04x}"),
    };
    let mut rest = vec![0u8; bound_len + 2];
    tcp.read_exact(&mut rest).await?;
    Ok(())
}

fn socks5_reason(code: u8) -> &'static str {
    match code {
        0x01 => "general failure",
        0x02 => "connection not allowed by ruleset",
        0x03 => "network unreachable",
        0x04 => "host unreachable",
        0x05 => "connection refused",
        0x06 => "TTL expired",
        0x07 => "command not supported",
        0x08 => "address type not supported",
        _ => "unknown error",
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use futures::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;

    use super::*;

    // ── Test servers ──────────────────────────────────────────────────────────

    /// WebSocket server that greets every client with "hello" and closes.
    async fn spawn_ws_server() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            // Two connections is the most any test makes.
            for _ in 0..2 {
                let (tcp, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
                    let _ = ws.send(Message::Text("hello".into())).await;
                    let _ = ws.close(None).await;
                });
            }
        });
        addr
    }

    async fn bridge(mut client: TcpStream, to: SocketAddr) {
        let mut server = TcpStream::connect(to).await.unwrap();
        let _ = tokio::io::copy_bidirectional(&mut client, &mut server).await;
    }

    /// HTTP CONNECT proxy answering with `status_line`; on a 2xx it tunnels to
    /// `to`. Resolves with the request head the client sent.
    async fn spawn_connect_proxy(
        status_line: &'static str,
        to: SocketAddr,
    ) -> (SocketAddr, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut head = Vec::new();
            let mut buf = [0u8; 256];
            while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = s.read(&mut buf).await.unwrap();
                assert!(n > 0, "client closed before finishing CONNECT");
                head.extend_from_slice(&buf[..n]);
            }
            s.write_all(format!("{status_line}\r\n\r\n").as_bytes())
                .await
                .unwrap();
            if status_line.contains(" 200 ") {
                bridge(s, to).await;
            }
            String::from_utf8_lossy(&head).into_owned()
        });
        (addr, handle)
    }

    #[derive(Clone, Copy, Default)]
    enum Bound {
        #[default]
        V4,
        V6,
        Domain,
        Unknown,
    }

    #[derive(Clone, Default)]
    struct SocksOpts {
        require_auth: Option<(&'static str, &'static str)>,
        method_override: Option<u8>,
        version_override: Option<u8>,
        reply_code: u8,
        bound: Bound,
        to: Option<SocketAddr>,
    }

    #[derive(Debug, Default)]
    struct Seen {
        creds: Option<(String, String)>,
        atyp: u8,
        host: String,
        port: u16,
    }

    /// Minimal SOCKS5 server. Resolves with what the client asked for.
    async fn spawn_socks_proxy(opts: SocksOpts) -> (SocketAddr, tokio::task::JoinHandle<Seen>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let mut seen = Seen::default();
            let (mut s, _) = listener.accept().await.unwrap();
            let mut hdr = [0u8; 2];
            s.read_exact(&mut hdr).await.unwrap();
            let mut methods = vec![0u8; hdr[1] as usize];
            s.read_exact(&mut methods).await.unwrap();

            let chosen = opts
                .method_override
                .unwrap_or(if opts.require_auth.is_some() { 2 } else { 0 });
            s.write_all(&[opts.version_override.unwrap_or(5), chosen])
                .await
                .unwrap();
            if chosen == 2 {
                let mut b = [0u8; 2];
                s.read_exact(&mut b).await.unwrap();
                let mut user = vec![0u8; b[1] as usize];
                s.read_exact(&mut user).await.unwrap();
                let mut plen = [0u8; 1];
                s.read_exact(&mut plen).await.unwrap();
                let mut pass = vec![0u8; plen[0] as usize];
                s.read_exact(&mut pass).await.unwrap();
                let (user, pass) = (
                    String::from_utf8(user).unwrap(),
                    String::from_utf8(pass).unwrap(),
                );
                let ok = opts.require_auth == Some((user.as_str(), pass.as_str()));
                seen.creds = Some((user, pass));
                s.write_all(&[1, if ok { 0 } else { 1 }]).await.unwrap();
                if !ok {
                    return seen;
                }
            } else if chosen != 0 || opts.version_override.is_some() {
                return seen;
            }

            let mut req = [0u8; 4];
            s.read_exact(&mut req).await.unwrap();
            seen.atyp = req[3];
            seen.host = match req[3] {
                1 => {
                    let mut a = [0u8; 4];
                    s.read_exact(&mut a).await.unwrap();
                    std::net::Ipv4Addr::from(a).to_string()
                }
                4 => {
                    let mut a = [0u8; 16];
                    s.read_exact(&mut a).await.unwrap();
                    std::net::Ipv6Addr::from(a).to_string()
                }
                _ => {
                    let mut n = [0u8; 1];
                    s.read_exact(&mut n).await.unwrap();
                    let mut h = vec![0u8; n[0] as usize];
                    s.read_exact(&mut h).await.unwrap();
                    String::from_utf8(h).unwrap()
                }
            };
            let mut p = [0u8; 2];
            s.read_exact(&mut p).await.unwrap();
            seen.port = u16::from_be_bytes(p);

            let mut reply = vec![5, opts.reply_code, 0];
            match opts.bound {
                Bound::V4 => reply.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0]),
                Bound::V6 => {
                    reply.push(4);
                    reply.extend_from_slice(&[0u8; 18]);
                }
                Bound::Domain => reply.extend_from_slice(&[3, 3, b'a', b'b', b'c', 0, 0]),
                Bound::Unknown => reply.push(9),
            }
            s.write_all(&reply).await.unwrap();
            if opts.reply_code == 0 {
                if let Some(to) = opts.to {
                    bridge(s, to).await;
                }
            }
            seen
        });
        (addr, handle)
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn prefs(http: Option<&str>, socks5: Option<&str>, no_proxy: Option<&str>) -> ProxySection {
        ProxySection {
            http: http.map(Into::into),
            socks5: socks5.map(Into::into),
            no_proxy: no_proxy.map(Into::into),
        }
    }

    async fn read_hello(mut ws: WsStream) {
        let msg = ws.next().await.unwrap().unwrap();
        assert_eq!(&*msg.into_text().unwrap(), "hello");
    }

    fn err_text<T>(result: Result<T>) -> String {
        result
            .err()
            .map(|e| format!("{e:#}"))
            .expect("expected an error")
    }

    async fn connect_err(url: &str, rules: &ProxyRules) -> String {
        err_text(connect_ws(url, rules).await)
    }

    fn route_err(rules: &ProxyRules, wss: bool, host: &str) -> String {
        err_text(rules.route(wss, host))
    }

    impl Route {
        fn into_proxy(self) -> Option<(String, Kind)> {
            match self {
                Route::Proxy { addr, kind } => Some((addr, kind)),
                Route::Direct => None,
            }
        }
    }

    impl Kind {
        /// `(credentials, remote_dns)` for a SOCKS5 kind.
        fn into_socks5(self) -> Option<(Option<(String, String)>, bool)> {
            match self {
                Kind::Socks5 { auth, remote_dns } => Some((auth, remote_dns)),
                Kind::HttpConnect { .. } => None,
            }
        }
    }

    fn proxy_route(rules: &ProxyRules, host: &str) -> (String, Kind) {
        rules
            .route(true, host)
            .unwrap()
            .into_proxy()
            .expect("expected a proxy route")
    }

    /// Environment with every proxy variable cleared except those given.
    fn proxy_env(
        https: Option<&str>,
        no_proxy: Option<&str>,
    ) -> Vec<(&'static str, Option<String>)> {
        let mut vars: Vec<(&'static str, Option<String>)> = [
            "ALL_PROXY",
            "all_proxy",
            "https_proxy",
            "no_proxy",
            "REQUEST_METHOD",
        ]
        .iter()
        .map(|k| (*k, None))
        .collect();
        vars.push(("HTTPS_PROXY", https.map(Into::into)));
        vars.push(("NO_PROXY", no_proxy.map(Into::into)));
        vars
    }

    // ── Routing ───────────────────────────────────────────────────────────────

    #[test]
    fn route_is_direct_without_a_proxy() {
        let route = ProxyRules::direct()
            .route(true, "stream.data.alpaca.markets")
            .unwrap();
        assert!(route.into_proxy().is_none());
    }

    #[test]
    fn route_uses_http_proxy_with_default_port() {
        let rules = ProxyRules::from_prefs_and_env(&prefs(Some("http://proxy.corp"), None, None));
        let (addr, kind) = proxy_route(&rules, "stream.data.alpaca.markets");
        assert_eq!(addr, "proxy.corp:80");
        assert!(matches!(kind, Kind::HttpConnect { auth: None }));
        // An HTTP CONNECT proxy is not a SOCKS5 one.
        let (_, kind) = proxy_route(&rules, "stream.data.alpaca.markets");
        assert!(kind.into_socks5().is_none());
    }

    #[test]
    fn route_prefers_socks5_over_http_and_flags_remote_dns() {
        let rules = ProxyRules::from_prefs_and_env(&prefs(
            Some("http://web.corp:3128"),
            Some("socks5h://u:p@sox.corp"),
            None,
        ));
        let (addr, kind) = proxy_route(&rules, "api.alpaca.markets");
        assert_eq!(addr, "sox.corp:1080");
        let (auth, remote_dns) = kind.into_socks5().expect("SOCKS5 kind");
        assert!(remote_dns);
        assert_eq!(auth, Some(("u".into(), "p".into())));

        let local = ProxyRules::from_prefs_and_env(&prefs(None, Some("socks5://sox.corp"), None));
        let (_, kind) = proxy_route(&local, "api.alpaca.markets");
        let (auth, remote_dns) = kind.into_socks5().expect("SOCKS5 kind");
        assert!(!remote_dns);
        assert!(auth.is_none());
    }

    #[test]
    fn route_honours_no_proxy_from_prefs() {
        let rules = ProxyRules::from_prefs_and_env(&prefs(
            Some("http://proxy.corp:3128"),
            None,
            Some("localhost,.alpaca.markets"),
        ));
        let bypass = rules.route(true, "stream.data.alpaca.markets").unwrap();
        assert!(bypass.into_proxy().is_none());
        let (addr, _) = proxy_route(&rules, "example.com");
        assert_eq!(addr, "proxy.corp:3128");
    }

    #[test]
    fn route_rejects_https_unsupported_and_invalid_proxies() {
        let route_err_for = |url: &str| {
            let rules = ProxyRules::from_prefs_and_env(&prefs(Some(url), None, None));
            route_err(&rules, true, "a.example")
        };
        assert!(route_err_for("https://p.corp:443").contains("not supported"));
        // Never falls back to a direct connection.
        assert!(route_err_for("ftp://p.corp").contains("unsupported proxy scheme"));
        assert!(route_err_for("socks4://p.corp").contains("unsupported proxy scheme"));
        assert!(route_err_for("not a url").contains("invalid proxy URL"));
        assert!(route_err_for("proxy.corp:8080").contains("invalid proxy URL"));
        assert!(route_err_for("http://").contains("invalid proxy URL"));
    }

    #[test]
    fn route_reads_environment_when_prefs_are_blank() {
        temp_env::with_vars(proxy_env(Some("http://envproxy.corp:3128"), None), || {
            let rules = ProxyRules::from_prefs_and_env(&ProxySection::default());
            let (addr, _) = proxy_route(&rules, "stream.data.alpaca.markets");
            assert_eq!(addr, "envproxy.corp:3128");
            // Saved preferences still win over the environment.
            let over =
                ProxyRules::from_prefs_and_env(&prefs(Some("http://cfg.corp:1"), None, None));
            let (addr, _) = proxy_route(&over, "stream.data.alpaca.markets");
            assert_eq!(addr, "cfg.corp:1");
        });
        temp_env::with_vars(
            proxy_env(Some("http://envproxy.corp:3128"), Some(".alpaca.markets")),
            || {
                let rules = ProxyRules::from_prefs_and_env(&ProxySection::default());
                let route = rules.route(true, "stream.data.alpaca.markets").unwrap();
                assert!(route.into_proxy().is_none());
            },
        );
    }

    #[test]
    fn route_rejects_an_environment_proxy_it_cannot_speak() {
        // hyper-util understands SOCKS4, which we do not implement.
        temp_env::with_vars(proxy_env(Some("socks4://envsocks.corp:1080"), None), || {
            let rules = ProxyRules::from_prefs_and_env(&ProxySection::default());
            assert!(route_err(&rules, true, "stream.data.alpaca.markets")
                .contains("unsupported proxy scheme"));
        });
    }

    // ── Direct and argument errors ────────────────────────────────────────────

    #[tokio::test]
    async fn connects_directly_without_a_proxy() {
        let ws = spawn_ws_server().await;
        let stream = connect_ws(&format!("ws://{ws}/"), &ProxyRules::direct())
            .await
            .unwrap();
        read_hello(stream).await;
    }

    #[tokio::test]
    async fn no_proxy_host_bypasses_an_unreachable_proxy() {
        let ws = spawn_ws_server().await;
        let rules = ProxyRules::from_prefs_and_env(&prefs(
            Some("http://127.0.0.1:1"),
            None,
            Some("127.0.0.1"),
        ));
        read_hello(connect_ws(&format!("ws://{ws}/"), &rules).await.unwrap()).await;
    }

    #[tokio::test]
    async fn rejects_malformed_urls() {
        let rules = ProxyRules::direct();
        assert!(connect_err("not a url", &rules)
            .await
            .contains("invalid WebSocket URL"));
        assert!(connect_err("/just/a/path", &rules)
            .await
            .contains("no host"));
    }

    #[tokio::test]
    async fn invalid_proxy_url_fails_instead_of_connecting_directly() {
        let ws = spawn_ws_server().await;
        let rules = ProxyRules::from_prefs_and_env(&prefs(Some("not a url"), None, None));
        let err = connect_err(&format!("ws://{ws}/"), &rules).await;
        assert!(err.contains("invalid proxy URL"), "{err}");
    }

    #[tokio::test]
    async fn unreachable_proxy_is_reported() {
        let rules = ProxyRules::from_prefs_and_env(&prefs(Some("http://127.0.0.1:1"), None, None));
        let err = connect_err("ws://alpaca.test:80/", &rules).await;
        assert!(err.contains("could not reach proxy"), "{err}");
    }

    #[tokio::test]
    async fn stalled_proxy_times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stub = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let _ = s.read(&mut [0u8; 512]).await; // take the CONNECT, then stall
            let _ = s.read(&mut [0u8; 1]).await; // until the client gives up
        });
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(Some(&format!("http://{addr}")), None, None));
        let err = err_text(
            connect_ws_with_timeout("ws://alpaca.test:80/", &rules, Duration::from_millis(100))
                .await,
        );
        assert!(err.contains("timed out"), "{err}");
        stub.await.unwrap(); // the stub exits once the client gives up
    }

    // ── HTTP CONNECT ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn http_connect_tunnels_the_websocket() {
        let ws = spawn_ws_server().await;
        let (proxy, head) = spawn_connect_proxy("HTTP/1.1 200 Connection established", ws).await;
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(Some(&format!("http://{proxy}")), None, None));
        // The host does not exist: only the tunnel can reach the server.
        read_hello(
            connect_ws("ws://alpaca.test:9999/stream", &rules)
                .await
                .unwrap(),
        )
        .await;
        let head = head.await.unwrap();
        assert!(
            head.starts_with("CONNECT alpaca.test:9999 HTTP/1.1\r\n"),
            "{head}"
        );
        assert!(!head.contains("Proxy-Authorization"), "{head}");
    }

    #[tokio::test]
    async fn http_connect_sends_basic_credentials() {
        let ws = spawn_ws_server().await;
        let (proxy, head) = spawn_connect_proxy("HTTP/1.1 200 OK", ws).await;
        let rules = ProxyRules::from_prefs_and_env(&prefs(
            Some(&format!("http://user:pass@{proxy}")),
            None,
            None,
        ));
        read_hello(connect_ws("ws://alpaca.test:9999/", &rules).await.unwrap()).await;
        let head = head.await.unwrap();
        assert!(
            head.contains("Proxy-Authorization: Basic dXNlcjpwYXNz"),
            "{head}"
        );
    }

    #[tokio::test]
    async fn http_connect_reports_refusal_and_auth_hint() {
        let dead = "127.0.0.1:1".parse().unwrap();
        let (proxy, _) =
            spawn_connect_proxy("HTTP/1.1 407 Proxy Authentication Required", dead).await;
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(Some(&format!("http://{proxy}")), None, None));
        let err = connect_err("ws://alpaca.test:80/", &rules).await;
        assert!(err.contains("407") && err.contains("credentials"), "{err}");

        let (proxy, _) = spawn_connect_proxy("HTTP/1.1 403 Forbidden", dead).await;
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(Some(&format!("http://{proxy}")), None, None));
        let err = connect_err("ws://alpaca.test:80/", &rules).await;
        assert!(err.contains("403") && !err.contains("credentials"), "{err}");
    }

    #[tokio::test]
    async fn http_connect_handles_a_proxy_that_hangs_up_or_floods() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 512];
            let _ = s.read(&mut buf).await; // take the CONNECT, then hang up
        });
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(Some(&format!("http://{addr}")), None, None));
        let err = connect_err("ws://alpaca.test:80/", &rules).await;
        assert!(err.contains("closed the connection"), "{err}");

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let _ = s.write_all(&vec![b'a'; MAX_CONNECT_HEAD + 100]).await;
            let _ = s.read(&mut [0u8; 1]).await; // until the client gives up
        });
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(Some(&format!("http://{addr}")), None, None));
        let err = connect_err("ws://alpaca.test:80/", &rules).await;
        assert!(err.contains("too large"), "{err}");
    }

    // ── SOCKS5 ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn socks5h_sends_the_hostname_to_the_proxy() {
        let ws = spawn_ws_server().await;
        let (proxy, seen) = spawn_socks_proxy(SocksOpts {
            to: Some(ws),
            ..Default::default()
        })
        .await;
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(None, Some(&format!("socks5h://{proxy}")), None));
        read_hello(connect_ws("ws://alpaca.test:9999/", &rules).await.unwrap()).await;
        let seen = seen.await.unwrap();
        assert_eq!(
            (seen.atyp, seen.host.as_str(), seen.port),
            (3, "alpaca.test", 9999)
        );
        assert!(seen.creds.is_none());
    }

    #[tokio::test]
    async fn socks5_resolves_locally_and_sends_an_address() {
        let ws = spawn_ws_server().await;
        let (proxy, seen) = spawn_socks_proxy(SocksOpts {
            to: Some(ws),
            bound: Bound::V6,
            ..Default::default()
        })
        .await;
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(None, Some(&format!("socks5://{proxy}")), None));
        let url = format!("ws://localhost:{}/", ws.port());
        read_hello(connect_ws(&url, &rules).await.unwrap()).await;
        let seen = seen.await.unwrap();
        assert!(seen.atyp == 1 || seen.atyp == 4, "atyp {}", seen.atyp);
        assert_eq!(seen.port, ws.port());
    }

    #[tokio::test]
    async fn socks5_sends_ip_literals_as_addresses() {
        let ws = spawn_ws_server().await;
        let (proxy, seen) = spawn_socks_proxy(SocksOpts {
            to: Some(ws),
            bound: Bound::Domain,
            ..Default::default()
        })
        .await;
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(None, Some(&format!("socks5h://{proxy}")), None));
        read_hello(connect_ws("ws://[::1]:9999/", &rules).await.unwrap()).await;
        let seen = seen.await.unwrap();
        assert_eq!((seen.atyp, seen.host.as_str()), (4, "::1"));

        let (proxy, seen) = spawn_socks_proxy(SocksOpts {
            to: Some(ws),
            ..Default::default()
        })
        .await;
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(None, Some(&format!("socks5://{proxy}")), None));
        read_hello(connect_ws("ws://192.0.2.7:9999/", &rules).await.unwrap()).await;
        let seen = seen.await.unwrap();
        assert_eq!((seen.atyp, seen.host.as_str()), (1, "192.0.2.7"));
    }

    #[tokio::test]
    async fn socks5_authenticates_with_username_and_password() {
        let ws = spawn_ws_server().await;
        let (proxy, seen) = spawn_socks_proxy(SocksOpts {
            require_auth: Some(("user", "pass")),
            to: Some(ws),
            ..Default::default()
        })
        .await;
        let rules = ProxyRules::from_prefs_and_env(&prefs(
            None,
            Some(&format!("socks5h://user:pass@{proxy}")),
            None,
        ));
        read_hello(connect_ws("ws://alpaca.test:80/", &rules).await.unwrap()).await;
        assert_eq!(
            seen.await.unwrap().creds,
            Some(("user".into(), "pass".into()))
        );
    }

    #[tokio::test]
    async fn socks5_reports_bad_credentials() {
        let (proxy, _) = spawn_socks_proxy(SocksOpts {
            require_auth: Some(("user", "right")),
            ..Default::default()
        })
        .await;
        let rules = ProxyRules::from_prefs_and_env(&prefs(
            None,
            Some(&format!("socks5h://user:wrong@{proxy}")),
            None,
        ));
        let err = connect_err("ws://alpaca.test:80/", &rules).await;
        assert!(err.contains("authentication failed"), "{err}");
    }

    #[tokio::test]
    async fn socks5_reports_handshake_failures() {
        let cases: [(SocksOpts, &str); 5] = [
            (
                SocksOpts {
                    require_auth: Some(("u", "p")),
                    ..Default::default()
                },
                "requires credentials",
            ),
            (
                SocksOpts {
                    method_override: Some(0xFF),
                    ..Default::default()
                },
                "none of our authentication methods",
            ),
            (
                SocksOpts {
                    method_override: Some(0x03),
                    ..Default::default()
                },
                "unsupported method",
            ),
            (
                SocksOpts {
                    version_override: Some(4),
                    ..Default::default()
                },
                "not a SOCKS5 server",
            ),
            (
                SocksOpts {
                    bound: Bound::Unknown,
                    ..Default::default()
                },
                "unknown address type",
            ),
        ];
        for (opts, expected) in cases {
            let (proxy, _) = spawn_socks_proxy(opts).await;
            let rules = ProxyRules::from_prefs_and_env(&prefs(
                None,
                Some(&format!("socks5h://{proxy}")),
                None,
            ));
            let err = connect_err("ws://alpaca.test:80/", &rules).await;
            assert!(err.contains(expected), "expected {expected:?}, got {err}");
        }
    }

    #[tokio::test]
    async fn socks5_maps_reply_codes_to_reasons() {
        for (code, reason) in [
            (1u8, "general failure"),
            (2, "not allowed"),
            (3, "network unreachable"),
            (4, "host unreachable"),
            (5, "connection refused"),
            (6, "TTL expired"),
            (7, "command not supported"),
            (8, "address type not supported"),
            (99, "unknown error"),
        ] {
            let (proxy, _) = spawn_socks_proxy(SocksOpts {
                reply_code: code,
                ..Default::default()
            })
            .await;
            let rules = ProxyRules::from_prefs_and_env(&prefs(
                None,
                Some(&format!("socks5h://{proxy}")),
                None,
            ));
            let err = connect_err("ws://alpaca.test:80/", &rules).await;
            assert!(err.contains(reason), "code {code}: {err}");
        }
    }

    #[tokio::test]
    async fn socks5_rejects_oversized_inputs() {
        let (proxy, _) = spawn_socks_proxy(SocksOpts {
            require_auth: Some(("u", "p")),
            ..Default::default()
        })
        .await;
        let long = "x".repeat(300);
        let rules = ProxyRules::from_prefs_and_env(&prefs(
            None,
            Some(&format!("socks5h://{long}:p@{proxy}")),
            None,
        ));
        let err = connect_err("ws://alpaca.test:80/", &rules).await;
        assert!(err.contains("credentials too long"), "{err}");

        let (proxy, _) = spawn_socks_proxy(SocksOpts::default()).await;
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(None, Some(&format!("socks5h://{proxy}")), None));
        let err = connect_err(&format!("ws://{}.test:80/", "a".repeat(300)), &rules).await;
        assert!(err.contains("hostname too long"), "{err}");
    }

    #[tokio::test]
    async fn socks5_reports_unresolvable_hosts_when_resolving_locally() {
        let (proxy, _) = spawn_socks_proxy(SocksOpts::default()).await;
        let rules =
            ProxyRules::from_prefs_and_env(&prefs(None, Some(&format!("socks5://{proxy}")), None));
        let err = connect_err("ws://no-such-host.invalid:80/", &rules).await;
        assert!(err.contains("could not resolve host"), "{err}");
    }
}
