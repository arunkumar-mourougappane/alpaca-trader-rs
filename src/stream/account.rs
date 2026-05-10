//! Account and order update WebSocket stream.
//!
//! Connects to the Alpaca account stream endpoint, authenticates, and
//! forwards [`TradeUpdate`] events to the application event channel.
//! Reconnects automatically with a backoff delay on disconnection.
//!
//! [`TradeUpdate`]: crate::events::Event::TradeUpdate
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::config::AlpacaConfig;
use crate::events::{Event, StreamKind};
use crate::types::Order;

const MAX_BACKOFF_SECS: u64 = 30;

/// Connects to the Alpaca account stream, listens for trade updates (fills,
/// cancels, rejects), and emits `Event::TradeUpdate` on each state change.
///
/// Reconnects automatically with exponential backoff.
pub async fn run(tx: Sender<Event>, cancel: CancellationToken, config: AlpacaConfig) {
    // Derive WebSocket URL from REST base_url:
    //   https://paper-api.alpaca.markets/v2  →  wss://paper-api.alpaca.markets/stream
    let ws_url = config
        .base_url
        .strip_suffix("/v2")
        .unwrap_or(&config.base_url)
        .replace("https://", "wss://")
        + "/stream";

    run_inner(tx, cancel, config, &ws_url).await
}

async fn run_inner(
    tx: Sender<Event>,
    cancel: CancellationToken,
    config: AlpacaConfig,
    ws_url: &str,
) {
    let mut backoff = 1u64;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("account stream shutting down");
                return;
            }
            _ = async {} => {}
        }

        match run_once(&tx, &cancel, &config, ws_url).await {
            Ok(_) => return,
            Err(e) => {
                warn!(error = %e, backoff_secs = backoff, "account stream disconnected, reconnecting");
                let _ = tx
                    .send(Event::StreamDisconnected(StreamKind::Account))
                    .await;
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
    ws_url: &str,
) -> anyhow::Result<()> {
    info!(url = ws_url, "connecting to account stream");

    let (ws, _) = connect_async(ws_url).await?;
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
        debug!(msg = %text, "account stream auth response");
        if !text.contains("authorized") && !text.contains("authenticated") {
            anyhow::bail!("account stream auth failed: {text}");
        }
    }

    // Subscribe to trade updates
    let listen = json!({
        "action": "listen",
        "data": { "streams": ["trade_updates"] }
    });
    write.send(Message::Text(listen.to_string().into())).await?;
    info!("account stream subscribed to trade_updates");
    let _ = tx.send(Event::StreamConnected(StreamKind::Account)).await;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            msg = read.next() => {
                match msg {
                    None => anyhow::bail!("account stream closed"),
                    Some(Err(e)) => anyhow::bail!("account stream error: {e}"),
                    Some(Ok(Message::Text(text))) => {
                        process_message(tx, &text).await;
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

#[cfg(test)]
pub(crate) fn parse_trade_update(text: &str) -> Option<Order> {
    let v: Value = serde_json::from_str(text).ok()?;
    if v["stream"] != "trade_updates" {
        return None;
    }
    serde_json::from_value::<Order>(v["data"]["order"].clone()).ok()
}

async fn process_message(tx: &Sender<Event>, text: &str) {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return;
    };
    if v["stream"] != "trade_updates" {
        return;
    }
    let event_type = v["data"]["event"].as_str().unwrap_or("");
    let order_val = &v["data"]["order"];

    match serde_json::from_value::<Order>(order_val.clone()) {
        Ok(order) => {
            info!(
                order_id = %order.id,
                symbol   = %order.symbol,
                event    = %event_type,
                status   = %order.status,
                "trade update received"
            );
            let _ = tx.send(Event::TradeUpdate(order)).await;
        }
        Err(e) => {
            error!(error = %e, event = %event_type, "failed to parse trade update order");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trade_update_msg(event: &str, status: &str) -> String {
        format!(
            r#"{{
                "stream": "trade_updates",
                "data": {{
                    "event": "{event}",
                    "order": {{
                        "id": "order-123",
                        "symbol": "AAPL",
                        "side": "buy",
                        "qty": "10",
                        "order_type": "limit",
                        "limit_price": "185.00",
                        "status": "{status}",
                        "filled_qty": "0",
                        "time_in_force": "day"
                    }}
                }}
            }}"#
        )
    }

    #[test]
    fn parse_trade_update_fill() {
        let msg = trade_update_msg("fill", "filled");
        let order = parse_trade_update(&msg).expect("should parse");
        assert_eq!(order.id, "order-123");
        assert_eq!(order.symbol, "AAPL");
        assert_eq!(order.status, "filled");
        assert_eq!(order.side, "buy");
        assert_eq!(order.qty.as_deref(), Some("10"));
    }

    #[test]
    fn parse_trade_update_canceled() {
        let msg = trade_update_msg("canceled", "canceled");
        let order = parse_trade_update(&msg).expect("should parse");
        assert_eq!(order.status, "canceled");
    }

    #[test]
    fn parse_trade_update_wrong_stream_returns_none() {
        let msg = r#"{"stream":"account_updates","data":{"event":"fill","order":{}}}"#;
        assert!(parse_trade_update(msg).is_none());
    }

    #[test]
    fn parse_trade_update_invalid_json_returns_none() {
        assert!(parse_trade_update("not json").is_none());
    }

    #[test]
    fn parse_trade_update_missing_order_fields_returns_none() {
        // order object is missing required fields like id, symbol, etc.
        let msg = r#"{"stream":"trade_updates","data":{"event":"fill","order":{"bad":"data"}}}"#;
        assert!(parse_trade_update(msg).is_none());
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::config::AlpacaEnv;
    use futures::SinkExt;
    use tokio::sync::mpsc;
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

    fn trade_update_json() -> &'static str {
        r#"{
            "stream": "trade_updates",
            "data": {
                "event": "fill",
                "order": {
                    "id": "order-abc",
                    "symbol": "AAPL",
                    "side": "buy",
                    "qty": "5",
                    "order_type": "limit",
                    "limit_price": "185.00",
                    "status": "filled",
                    "filled_qty": "5",
                    "time_in_force": "day"
                }
            }
        }"#
    }

    #[tokio::test]
    async fn account_run_once_auth_success_emits_trade_update() {
        let (listener, url) = bind_local().await;

        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            let _ = ws.next().await; // consume auth
            ws.send(Message::Text(
                r#"{"stream":"authorization","data":{"status":"authorized"}}"#.into(),
            ))
            .await
            .unwrap();
            let _ = ws.next().await; // consume listen
            ws.send(Message::Text(trade_update_json().into()))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(500)).await;
        });

        let (tx, mut rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();
        let cancel2 = cancel.clone();
        let tx2 = tx.clone();
        let config = test_config();
        let url2 = url.clone();

        tokio::spawn(async move {
            run_once(&tx2, &cancel2, &config, &url2).await.ok();
            cancel2.cancel();
        });

        let order = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                match rx.recv().await? {
                    Event::TradeUpdate(o) => return Some(o),
                    _ => continue,
                }
            }
        })
        .await
        .expect("timed out waiting for TradeUpdate")
        .expect("channel closed");

        assert_eq!(order.id, "order-abc");
        assert_eq!(order.symbol, "AAPL");
        assert_eq!(order.status, "filled");
    }

    #[tokio::test]
    async fn account_run_once_auth_failure_returns_err() {
        let (listener, url) = bind_local().await;

        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            let _ = ws.next().await;
            ws.send(Message::Text(
                r#"{"stream":"authorization","data":{"status":"rejected"}}"#.into(),
            ))
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        let (tx, _rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();

        let result = run_once(&tx, &cancel, &test_config(), &url).await;
        assert!(result.is_err(), "auth failure should return Err");
    }

    #[tokio::test]
    async fn account_run_once_exits_cleanly_on_cancellation() {
        let (listener, url) = bind_local().await;
        let cancel = CancellationToken::new();
        let cancel2 = cancel.clone();

        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            let _ = ws.next().await;
            ws.send(Message::Text(
                r#"{"stream":"authorization","data":{"status":"authorized"}}"#.into(),
            ))
            .await
            .unwrap();
            let _ = ws.next().await; // consume listen
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        let (tx, _rx) = mpsc::channel(16);
        let config = test_config();
        let url2 = url.clone();

        let task = tokio::spawn(async move { run_once(&tx, &cancel2, &config, &url2).await });

        // Allow time for auth to complete and stream to enter the main loop
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
    async fn account_run_reconnects_after_server_close() {
        let (listener, url) = bind_local().await;

        tokio::spawn(async move {
            // First connection: authenticate then close
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            let _ = ws.next().await;
            ws.send(Message::Text(
                r#"{"stream":"authorization","data":{"status":"authorized"}}"#.into(),
            ))
            .await
            .unwrap();
            let _ = ws.next().await; // consume listen
            drop(ws); // close — triggers reconnect

            // Second connection: authenticate and send a trade update
            let (tcp2, _) = listener.accept().await.unwrap();
            let mut ws2 = accept_async(tcp2).await.unwrap();
            let _ = ws2.next().await;
            ws2.send(Message::Text(
                r#"{"stream":"authorization","data":{"status":"authorized"}}"#.into(),
            ))
            .await
            .unwrap();
            let _ = ws2.next().await;
            ws2.send(Message::Text(trade_update_json().into()))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(500)).await;
        });

        let (tx, mut rx) = mpsc::channel(32);
        let cancel = CancellationToken::new();
        let cancel2 = cancel.clone();

        let url2 = url.clone();
        tokio::spawn(async move {
            run_inner(tx, cancel2, test_config(), &url2).await;
        });

        let mut saw_disconnect = false;
        let mut saw_trade = false;
        tokio::time::timeout(Duration::from_secs(5), async {
            while !saw_disconnect || !saw_trade {
                match rx.recv().await {
                    Some(Event::StreamDisconnected(StreamKind::Account)) => {
                        saw_disconnect = true;
                    }
                    Some(Event::TradeUpdate(o)) if o.symbol == "AAPL" => {
                        saw_trade = true;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
        })
        .await
        .expect("should see disconnect + reconnect trade update within 5s");

        cancel.cancel();
        assert!(
            saw_disconnect,
            "should emit StreamDisconnected on first close"
        );
        assert!(saw_trade, "should emit TradeUpdate after reconnect");
    }
}
