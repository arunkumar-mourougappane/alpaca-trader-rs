//! Real-time market data WebSocket stream.
//!
//! Connects to the Alpaca market data stream endpoint, authenticates,
//! subscribes to quotes for the symbols in the active watchlist, and
//! forwards [`MarketQuote`] events to the application event channel.
//! Reconnects automatically with a backoff delay on disconnection.
//!
//! [`MarketQuote`]: crate::events::Event::MarketQuote
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc::Sender, watch};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::config::AlpacaConfig;
use crate::events::{Event, StreamKind};
use crate::types::Quote;

const DATA_URL: &str = "wss://stream.data.alpaca.markets/v2/iex";
const MAX_BACKOFF_SECS: u64 = 30;

/// Connects to the Alpaca market data WebSocket (IEX free tier), subscribes to
/// quotes for the given symbols, and emits `Event::MarketQuote` on every tick.
///
/// Reconnects automatically with exponential backoff. Symbol subscriptions are
/// updated whenever a new list arrives on `symbol_rx`.
pub async fn run(
    tx: Sender<Event>,
    cancel: CancellationToken,
    config: AlpacaConfig,
    symbol_rx: watch::Receiver<Vec<String>>,
) {
    run_inner(tx, cancel, config, symbol_rx, DATA_URL).await
}

async fn run_inner(
    tx: Sender<Event>,
    cancel: CancellationToken,
    config: AlpacaConfig,
    mut symbol_rx: watch::Receiver<Vec<String>>,
    url: &str,
) {
    let mut backoff = 1u64;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("market stream shutting down");
                return;
            }
            _ = async {} => {}
        }

        let symbols = symbol_rx.borrow().clone();
        if symbols.is_empty() {
            // Wait until we have symbols to subscribe to
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = symbol_rx.changed() => continue,
            }
        }

        match run_once(&tx, &cancel, &config, &mut symbol_rx, url).await {
            Ok(_) => {
                // clean shutdown requested
                return;
            }
            Err(e) => {
                warn!(error = %e, backoff_secs = backoff, "market stream disconnected, reconnecting");
                let _ = tx.send(Event::StreamDisconnected(StreamKind::Market)).await;
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
                }
                backoff = (backoff * 2).min(MAX_BACKOFF_SECS);
            }
        }
    }
}

async fn run_once(
    tx: &Sender<Event>,
    cancel: &CancellationToken,
    config: &AlpacaConfig,
    symbol_rx: &mut watch::Receiver<Vec<String>>,
    url: &str,
) -> anyhow::Result<()> {
    info!(url = url, "connecting to market data stream");

    let (ws, _) = connect_async(url).await?;
    let (mut write, mut read) = ws.split();

    // Authenticate
    let auth = json!({
        "action": "auth",
        "key": config.key,
        "secret": config.secret
    });
    write.send(Message::Text(auth.to_string().into())).await?;

    // Wait for auth confirmation
    if let Some(Ok(msg)) = read.next().await {
        let text = msg.into_text().unwrap_or_default();
        debug!(msg = %text, "market stream auth response");
        if !text.contains("authenticated") && !text.contains("already authenticated") {
            anyhow::bail!("market stream auth failed: {text}");
        }
    }

    // Subscribe to current symbols
    let symbols = symbol_rx.borrow().clone();
    subscribe(&mut write, &symbols).await?;
    info!(count = symbols.len(), "subscribed to market quotes");
    let _ = tx.send(Event::StreamConnected(StreamKind::Market)).await;

    let mut prev_symbols = symbols;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),

            // Re-subscribe when watchlist changes
            _ = symbol_rx.changed() => {
                let new_symbols = symbol_rx.borrow().clone();
                if new_symbols != prev_symbols {
                    // Unsubscribe symbols that were removed from the watchlist.
                    // The Alpaca IEX protocol merges subscriptions — a new
                    // subscribe does NOT replace the existing set, so an
                    // explicit unsubscribe is required for removed symbols.
                    let removed: Vec<String> = prev_symbols
                        .iter()
                        .filter(|s| !new_symbols.contains(s))
                        .cloned()
                        .collect();
                    if !removed.is_empty() {
                        unsubscribe(&mut write, &removed).await?;
                        info!(count = removed.len(), "unsubscribed removed symbols");
                    }
                    subscribe(&mut write, &new_symbols).await?;
                    info!(count = new_symbols.len(), "updated market quote subscriptions");
                    prev_symbols = new_symbols;
                }
            }

            // Receive market data messages
            msg = read.next() => {
                match msg {
                    None => anyhow::bail!("market stream closed"),
                    Some(Err(e)) => anyhow::bail!("market stream error: {e}"),
                    Some(Ok(Message::Text(text))) => {
                        process_messages(tx, &text).await;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        write.send(Message::Pong(data)).await?;
                    }
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

async fn subscribe(
    write: &mut (impl SinkExt<Message, Error = impl std::fmt::Display> + Unpin),
    symbols: &[String],
) -> anyhow::Result<()> {
    let sub = json!({
        "action": "subscribe",
        "quotes": symbols
    });
    write
        .send(Message::Text(sub.to_string().into()))
        .await
        .map_err(|e| anyhow::anyhow!("subscribe send failed: {e}"))
}

async fn unsubscribe(
    write: &mut (impl SinkExt<Message, Error = impl std::fmt::Display> + Unpin),
    symbols: &[String],
) -> anyhow::Result<()> {
    let unsub = json!({
        "action": "unsubscribe",
        "quotes": symbols
    });
    write
        .send(Message::Text(unsub.to_string().into()))
        .await
        .map_err(|e| anyhow::anyhow!("unsubscribe send failed: {e}"))
}

#[cfg(test)]
pub(crate) fn parse_quotes(text: &str) -> Vec<Quote> {
    let Ok(msgs) = serde_json::from_str::<Vec<Value>>(text) else {
        return vec![];
    };
    msgs.into_iter()
        .filter(|m| m["T"] == "q")
        .map(|m| Quote {
            symbol: m["S"].as_str().unwrap_or("").to_string(),
            ap: m["ap"].as_f64(),
            bp: m["bp"].as_f64(),
            as_: m["as"].as_u64(),
            bs: m["bs"].as_u64(),
        })
        .collect()
}

async fn process_messages(tx: &Sender<Event>, text: &str) {
    let Ok(msgs) = serde_json::from_str::<Vec<Value>>(text) else {
        return;
    };
    for m in msgs {
        if m["T"] == "q" {
            let quote = Quote {
                symbol: m["S"].as_str().unwrap_or("").to_string(),
                ap: m["ap"].as_f64(),
                bp: m["bp"].as_f64(),
                as_: m["as"].as_u64(),
                bs: m["bs"].as_u64(),
            };
            debug!(symbol = %quote.symbol, ask = ?quote.ap, bid = ?quote.bp, "quote received");
            let _ = tx.send(Event::MarketQuote(quote)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quotes_extracts_ask_and_bid() {
        let text = r#"[{"T":"q","S":"AAPL","ap":185.50,"bp":185.49,"as":100,"bs":150}]"#;
        let quotes = parse_quotes(text);
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].symbol, "AAPL");
        assert_eq!(quotes[0].ap, Some(185.50));
        assert_eq!(quotes[0].bp, Some(185.49));
        assert_eq!(quotes[0].as_, Some(100));
        assert_eq!(quotes[0].bs, Some(150));
    }

    #[test]
    fn parse_quotes_ignores_non_quote_messages() {
        let text = r#"[
            {"T":"t","S":"AAPL","p":185.51,"s":200},
            {"T":"q","S":"TSLA","ap":180.0,"bp":179.9},
            {"T":"b","S":"AAPL","o":185.0,"h":186.0,"l":184.0,"c":185.5,"v":10000}
        ]"#;
        let quotes = parse_quotes(text);
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].symbol, "TSLA");
    }

    #[test]
    fn parse_quotes_multiple_symbols() {
        let text = r#"[
            {"T":"q","S":"AAPL","ap":185.0,"bp":184.9},
            {"T":"q","S":"TSLA","ap":200.0,"bp":199.9}
        ]"#;
        let quotes = parse_quotes(text);
        assert_eq!(quotes.len(), 2);
        assert_eq!(quotes[0].symbol, "AAPL");
        assert_eq!(quotes[1].symbol, "TSLA");
    }

    #[test]
    fn parse_quotes_empty_array() {
        let quotes = parse_quotes("[]");
        assert!(quotes.is_empty());
    }

    #[test]
    fn parse_quotes_invalid_json_returns_empty() {
        let quotes = parse_quotes("not json");
        assert!(quotes.is_empty());
    }

    #[test]
    fn parse_quotes_missing_optional_fields() {
        // ap and bp are optional — should parse without panicking
        let text = r#"[{"T":"q","S":"AAPL"}]"#;
        let quotes = parse_quotes(text);
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].symbol, "AAPL");
        assert!(quotes[0].ap.is_none());
        assert!(quotes[0].bp.is_none());
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::config::AlpacaEnv;
    use futures::SinkExt;
    use tokio::sync::{mpsc, watch};
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    fn test_config() -> AlpacaConfig {
        AlpacaConfig {
            base_url: String::new(),
            key: "test-key".to_string(),
            secret: "test-secret".to_string(),
            env: AlpacaEnv::Paper,
        }
    }

    async fn bind_local() -> (tokio::net::TcpListener, String) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, format!("ws://127.0.0.1:{}", port))
    }

    #[tokio::test]
    async fn market_run_once_auth_success_emits_quote() {
        let (listener, url) = bind_local().await;

        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            let _ = ws.next().await; // consume auth
            ws.send(Message::Text(
                r#"[{"T":"success","msg":"authenticated"}]"#.into(),
            ))
            .await
            .unwrap();
            let _ = ws.next().await; // consume subscribe
            ws.send(Message::Text(
                r#"[{"T":"q","S":"AAPL","ap":185.0,"bp":184.9}]"#.into(),
            ))
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_millis(500)).await;
        });

        let (tx, mut rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();
        let (_sym_tx, mut sym_rx) = watch::channel(vec!["AAPL".to_string()]);

        let cancel2 = cancel.clone();
        let url2 = url.clone();
        let tx2 = tx.clone();
        tokio::spawn(async move {
            run_once(&tx2, &cancel2, &test_config(), &mut sym_rx, &url2)
                .await
                .ok();
            cancel2.cancel();
        });

        let quote = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                match rx.recv().await? {
                    Event::MarketQuote(q) => return Some(q),
                    _ => continue,
                }
            }
        })
        .await
        .expect("timed out waiting for MarketQuote")
        .expect("channel closed");

        assert_eq!(quote.symbol, "AAPL");
        assert_eq!(quote.ap, Some(185.0));
        assert_eq!(quote.bp, Some(184.9));
    }

    #[tokio::test]
    async fn market_run_once_auth_failure_returns_err() {
        let (listener, url) = bind_local().await;

        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            let _ = ws.next().await;
            ws.send(Message::Text(
                r#"[{"T":"error","msg":"invalid credentials"}]"#.into(),
            ))
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        let (tx, _rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();
        let (_sym_tx, mut sym_rx) = watch::channel(vec!["AAPL".to_string()]);

        let result = run_once(&tx, &cancel, &test_config(), &mut sym_rx, &url).await;
        assert!(result.is_err(), "auth failure should return Err");
    }

    #[tokio::test]
    async fn market_run_once_exits_cleanly_on_cancellation() {
        let (listener, url) = bind_local().await;
        let cancel = CancellationToken::new();
        let cancel2 = cancel.clone();

        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            let _ = ws.next().await;
            ws.send(Message::Text(
                r#"[{"T":"success","msg":"authenticated"}]"#.into(),
            ))
            .await
            .unwrap();
            let _ = ws.next().await; // consume subscribe
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        let (tx, _rx) = mpsc::channel(16);
        let (_sym_tx, mut sym_rx) = watch::channel(vec!["AAPL".to_string()]);
        let config = test_config();
        let url2 = url.clone();

        let task =
            tokio::spawn(async move { run_once(&tx, &cancel2, &config, &mut sym_rx, &url2).await });

        // Give the stream time to authenticate and enter the main loop
        tokio::time::sleep(Duration::from_millis(150)).await;
        cancel.cancel();

        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("task should finish within 1s after cancellation");
        assert!(
            matches!(result.unwrap(), Ok(())),
            "cancellation should return Ok"
        );
    }

    #[tokio::test]
    async fn market_run_reconnects_after_server_close() {
        let (listener, url) = bind_local().await;

        tokio::spawn(async move {
            // First connection: authenticate then close
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            let _ = ws.next().await;
            ws.send(Message::Text(
                r#"[{"T":"success","msg":"authenticated"}]"#.into(),
            ))
            .await
            .unwrap();
            let _ = ws.next().await; // consume subscribe
            drop(ws); // close — triggers reconnect

            // Second connection: send a quote
            let (tcp2, _) = listener.accept().await.unwrap();
            let mut ws2 = accept_async(tcp2).await.unwrap();
            let _ = ws2.next().await;
            ws2.send(Message::Text(
                r#"[{"T":"success","msg":"authenticated"}]"#.into(),
            ))
            .await
            .unwrap();
            let _ = ws2.next().await;
            ws2.send(Message::Text(
                r#"[{"T":"q","S":"TSLA","ap":200.0,"bp":199.9}]"#.into(),
            ))
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_millis(500)).await;
        });

        let (tx, mut rx) = mpsc::channel(32);
        let cancel = CancellationToken::new();
        let cancel2 = cancel.clone();
        let (_sym_tx, sym_rx) = watch::channel(vec!["TSLA".to_string()]);

        let url2 = url.clone();
        tokio::spawn(async move {
            run_inner(tx, cancel2, test_config(), sym_rx, &url2).await;
        });

        let mut saw_disconnect = false;
        let mut saw_quote = false;
        tokio::time::timeout(Duration::from_secs(5), async {
            while !saw_disconnect || !saw_quote {
                match rx.recv().await {
                    Some(Event::StreamDisconnected(StreamKind::Market)) => {
                        saw_disconnect = true;
                    }
                    Some(Event::MarketQuote(q)) if q.symbol == "TSLA" => {
                        saw_quote = true;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
        })
        .await
        .expect("should see disconnect + reconnect quote within 5s");

        cancel.cancel();
        assert!(
            saw_disconnect,
            "should emit StreamDisconnected on first close"
        );
        assert!(saw_quote, "should emit MarketQuote after reconnect");
    }

    /// Verifies that when a symbol is removed from the watchlist, an
    /// `{"action":"unsubscribe","quotes":[...]}` message is sent to the server
    /// before the subsequent subscribe message.
    #[tokio::test]
    async fn market_run_once_sends_unsubscribe_on_symbol_removal() {
        let (listener, url) = bind_local().await;

        // Channel to capture the raw WebSocket frames the client sends
        let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<String>(16);

        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();

            // Consume auth
            let _ = ws.next().await;
            ws.send(Message::Text(
                r#"[{"T":"success","msg":"authenticated"}]"#.into(),
            ))
            .await
            .unwrap();

            // Record every client-sent text frame
            while let Some(Ok(Message::Text(t))) = ws.next().await {
                let _ = msg_tx.send(t.to_string()).await;
            }
        });

        let (tx, _rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();
        let cancel2 = cancel.clone();

        // Start with AAPL + TSLA
        let (sym_tx, mut sym_rx) = watch::channel(vec!["AAPL".to_string(), "TSLA".to_string()]);

        let url2 = url.clone();
        let config = test_config();
        tokio::spawn(async move {
            run_once(&tx, &cancel2, &config, &mut sym_rx, &url2)
                .await
                .ok();
        });

        // Wait for the initial subscribe to be processed
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Remove TSLA — only AAPL remains
        sym_tx.send(vec!["AAPL".to_string()]).unwrap();

        // Collect messages for a moment then cancel
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel.cancel();

        // Gather all captured frames
        let mut frames = vec![];
        while let Ok(m) = msg_rx.try_recv() {
            frames.push(m);
        }

        // Verify an unsubscribe for TSLA was sent
        let unsub_frame = frames
            .iter()
            .find(|f| f.contains("unsubscribe") && f.contains("TSLA"));
        assert!(
            unsub_frame.is_some(),
            "expected an unsubscribe frame for TSLA; got: {frames:?}"
        );

        // Verify the unsubscribe came before the next subscribe
        let unsub_pos = frames
            .iter()
            .position(|f| f.contains("unsubscribe") && f.contains("TSLA"))
            .unwrap();
        let resub_pos = frames
            .iter()
            .skip(unsub_pos + 1)
            .position(|f| f.contains("subscribe") && f.contains("AAPL"))
            .map(|i| i + unsub_pos + 1);
        assert!(
            resub_pos.is_some(),
            "expected a re-subscribe for AAPL after the unsubscribe; got: {frames:?}"
        );
    }

    /// Verifies that adding a new symbol (without removing any) does NOT
    /// send an unsubscribe message — only a subscribe.
    #[tokio::test]
    async fn market_run_once_no_unsubscribe_on_symbol_addition() {
        let (listener, url) = bind_local().await;

        let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<String>(16);

        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            let _ = ws.next().await;
            ws.send(Message::Text(
                r#"[{"T":"success","msg":"authenticated"}]"#.into(),
            ))
            .await
            .unwrap();
            while let Some(Ok(Message::Text(t))) = ws.next().await {
                let _ = msg_tx.send(t.to_string()).await;
            }
        });

        let (tx, _rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();
        let cancel2 = cancel.clone();

        let (sym_tx, mut sym_rx) = watch::channel(vec!["AAPL".to_string()]);

        let url2 = url.clone();
        let config = test_config();
        tokio::spawn(async move {
            run_once(&tx, &cancel2, &config, &mut sym_rx, &url2)
                .await
                .ok();
        });

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Add TSLA — no removal
        sym_tx
            .send(vec!["AAPL".to_string(), "TSLA".to_string()])
            .unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel.cancel();

        let mut frames = vec![];
        while let Ok(m) = msg_rx.try_recv() {
            frames.push(m);
        }

        let has_unsub = frames.iter().any(|f| f.contains("unsubscribe"));
        assert!(
            !has_unsub,
            "no unsubscribe should be sent when only adding symbols; got: {frames:?}"
        );
    }
}
