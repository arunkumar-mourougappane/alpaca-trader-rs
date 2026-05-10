use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc::Sender, watch};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::config::AlpacaConfig;
use crate::events::Event;
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
    mut symbol_rx: watch::Receiver<Vec<String>>,
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

        match run_once(&tx, &cancel, &config, &mut symbol_rx).await {
            Ok(_) => {
                // clean shutdown requested
                return;
            }
            Err(e) => {
                warn!(error = %e, backoff_secs = backoff, "market stream disconnected, reconnecting");
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
) -> anyhow::Result<()> {
    info!(url = DATA_URL, "connecting to market data stream");

    let (ws, _) = connect_async(DATA_URL).await?;
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

    let mut prev_symbols = symbols;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),

            // Re-subscribe when watchlist changes
            _ = symbol_rx.changed() => {
                let new_symbols = symbol_rx.borrow().clone();
                if new_symbols != prev_symbols {
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
