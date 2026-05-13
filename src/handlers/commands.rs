use std::sync::Arc;

use tokio::sync::{mpsc::Receiver, mpsc::Sender, Notify};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument, warn};

use crate::client::AlpacaClient;
use crate::commands::Command;

use crate::events::Event;
use crate::types::{OrderRequest, TimeInForce};

/// Exposed for testing — executes a single command without the loop.
#[cfg(test)]
pub(crate) async fn execute_one(
    cmd: Command,
    tx: &Sender<Event>,
    client: &AlpacaClient,
    refresh_notify: &Notify,
) {
    handle(cmd, tx, client, refresh_notify).await;
}

pub async fn run(
    mut rx: Receiver<Command>,
    tx: Sender<Event>,
    client: Arc<AlpacaClient>,
    refresh_notify: Arc<Notify>,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            Some(cmd) = rx.recv() => {
                handle(cmd, &tx, &client, &refresh_notify).await;
            }
            _ = cancel.cancelled() => break,
        }
    }
}

#[instrument(skip(tx, client, refresh_notify))]
async fn handle(cmd: Command, tx: &Sender<Event>, client: &AlpacaClient, refresh_notify: &Notify) {
    match cmd {
        Command::SubmitOrder {
            symbol,
            side,
            order_type,
            qty,
            price,
            time_in_force,
        } => {
            let tif = match time_in_force.as_str() {
                "gtc" => TimeInForce::Gtc,
                _ => TimeInForce::Day,
            };
            let req = OrderRequest {
                symbol: symbol.clone(),
                qty,
                notional: None,
                side,
                order_type,
                time_in_force: tif.as_str().into(),
                limit_price: price,
            };
            info!(symbol = %symbol, "submitting order");
            match client.submit_order(&req).await {
                Ok(o) => {
                    info!(order_id = %o.id, status = %o.status, "order accepted");
                    let _ = tx.send(Event::StatusMsg("Order submitted".into())).await;
                }
                Err(e) => {
                    error!(error = %e, "order submission failed");
                    let _ = tx.send(Event::StatusMsg(format!("Order error: {e}"))).await;
                }
            }
            refresh_notify.notify_one();
        }

        Command::CancelOrder(id) => {
            info!(order_id = %id, "cancelling order");
            match client.cancel_order(&id).await {
                Ok(_) => {
                    info!(order_id = %id, "order cancelled");
                    let _ = tx.send(Event::StatusMsg("Order cancelled".into())).await;
                }
                Err(e) => {
                    warn!(error = %e, order_id = %id, "cancel failed (may already be filled)");
                    let _ = tx
                        .send(Event::StatusMsg(format!("Cancel error: {e}")))
                        .await;
                }
            }
            refresh_notify.notify_one();
        }

        Command::AddToWatchlist {
            watchlist_id,
            symbol,
        } => {
            info!(symbol = %symbol, "adding to watchlist");
            match client.add_to_watchlist(&watchlist_id, &symbol).await {
                Ok(wl) => {
                    info!(symbol = %symbol, "added to watchlist");
                    let _ = tx.send(Event::WatchlistUpdated(wl)).await;
                    let _ = tx.send(Event::StatusMsg(format!("Added {symbol}"))).await;
                }
                Err(e) => {
                    error!(error = %e, "add to watchlist failed");
                    let _ = tx
                        .send(Event::StatusMsg(format!("Watchlist error: {e}")))
                        .await;
                }
            }
        }

        Command::RemoveFromWatchlist {
            watchlist_id,
            symbol,
        } => {
            info!(symbol = %symbol, "removing from watchlist");
            match client.remove_from_watchlist(&watchlist_id, &symbol).await {
                Ok(wl) => {
                    info!(symbol = %symbol, "removed from watchlist");
                    let _ = tx.send(Event::WatchlistUpdated(wl)).await;
                    let _ = tx.send(Event::StatusMsg(format!("Removed {symbol}"))).await;
                }
                Err(e) => {
                    error!(error = %e, "remove from watchlist failed");
                    let _ = tx
                        .send(Event::StatusMsg(format!("Watchlist error: {e}")))
                        .await;
                }
            }
        }

        Command::FetchIntradayBars(symbol) => {
            info!(symbol = %symbol, "fetching intraday bars");
            match client.get_intraday_bars(&symbol).await {
                Ok(bars) => {
                    let cents: Vec<u64> = bars.iter().map(|b| (b.c * 100.0) as u64).collect();
                    let _ = tx
                        .send(Event::IntradayBarsReceived {
                            symbol,
                            bars: cents,
                        })
                        .await;
                }
                Err(e) => {
                    warn!(error = %e, symbol = %symbol, "intraday bars fetch failed");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AlpacaConfig, AlpacaEnv};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::{mpsc, Notify};
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    fn test_config(base_url: String) -> AlpacaConfig {
        AlpacaConfig {
            base_url,
            key: "PKTEST".into(),
            secret: "secret".into(),
            env: AlpacaEnv::Paper,
        }
    }

    fn order_response() -> serde_json::Value {
        json!({
            "id": "new-order-id",
            "symbol": "AAPL",
            "side": "buy",
            "qty": "10",
            "order_type": "limit",
            "limit_price": "185.00",
            "status": "accepted",
            "filled_qty": "0",
            "time_in_force": "day"
        })
    }

    fn watchlist_response() -> serde_json::Value {
        json!({ "id": "wl-1", "name": "Primary", "assets": [] })
    }

    async fn collect_events(rx: &mut mpsc::Receiver<Event>) -> Vec<Event> {
        let mut events = vec![];
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }
        events
    }

    #[tokio::test]
    async fn submit_order_sends_accepted_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(order_response()))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(test_config(server.uri()));
        let (tx, mut rx) = mpsc::channel(16);
        let notify = Arc::new(Notify::new());

        execute_one(
            Command::SubmitOrder {
                symbol: "AAPL".into(),
                side: "buy".into(),
                order_type: "limit".into(),
                qty: Some("10".into()),
                price: Some("185.00".into()),
                time_in_force: "day".into(),
            },
            &tx,
            &client,
            &notify,
        )
        .await;

        let events = collect_events(&mut rx).await;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::StatusMsg(m) if m == "Order submitted")),
            "expected 'Order submitted' status"
        );
    }

    #[tokio::test]
    async fn submit_order_api_error_sends_error_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/orders"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(test_config(server.uri()));
        let (tx, mut rx) = mpsc::channel(16);
        let notify = Arc::new(Notify::new());

        execute_one(
            Command::SubmitOrder {
                symbol: "AAPL".into(),
                side: "buy".into(),
                order_type: "market".into(),
                qty: Some("5".into()),
                price: None,
                time_in_force: "day".into(),
            },
            &tx,
            &client,
            &notify,
        )
        .await;

        let events = collect_events(&mut rx).await;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::StatusMsg(m) if m.contains("Order error"))),
            "expected error status on 500"
        );
    }

    #[tokio::test]
    async fn cancel_order_sends_cancelled_status() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/orders/order-abc"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(test_config(server.uri()));
        let (tx, mut rx) = mpsc::channel(16);
        let notify = Arc::new(Notify::new());

        execute_one(
            Command::CancelOrder("order-abc".into()),
            &tx,
            &client,
            &notify,
        )
        .await;

        let events = collect_events(&mut rx).await;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::StatusMsg(m) if m == "Order cancelled")),
            "expected 'Order cancelled' status"
        );
    }

    #[tokio::test]
    async fn add_to_watchlist_emits_watchlist_updated() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/watchlists/wl-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(watchlist_response()))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(test_config(server.uri()));
        let (tx, mut rx) = mpsc::channel(16);
        let notify = Arc::new(Notify::new());

        execute_one(
            Command::AddToWatchlist {
                watchlist_id: "wl-1".into(),
                symbol: "NVDA".into(),
            },
            &tx,
            &client,
            &notify,
        )
        .await;

        let events = collect_events(&mut rx).await;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::WatchlistUpdated(_))),
            "expected WatchlistUpdated event"
        );
    }

    #[tokio::test]
    async fn remove_from_watchlist_emits_watchlist_updated() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/watchlists/wl-1/TLRY"))
            .respond_with(ResponseTemplate::new(200).set_body_json(watchlist_response()))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(test_config(server.uri()));
        let (tx, mut rx) = mpsc::channel(16);
        let notify = Arc::new(Notify::new());

        execute_one(
            Command::RemoveFromWatchlist {
                watchlist_id: "wl-1".into(),
                symbol: "TLRY".into(),
            },
            &tx,
            &client,
            &notify,
        )
        .await;

        let events = collect_events(&mut rx).await;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::WatchlistUpdated(_))),
            "expected WatchlistUpdated event"
        );
    }

    #[tokio::test]
    async fn fetch_intraday_bars_emits_intraday_bars_received() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/stocks/AMD/bars"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "bars": [
                    {"t": "2026-05-12T13:30:00Z", "o": 141.10, "h": 143.20, "l": 140.85, "c": 142.85, "v": 1000000},
                    {"t": "2026-05-12T13:31:00Z", "o": 142.85, "h": 144.00, "l": 142.50, "c": 143.50, "v": 500000}
                ],
                "symbol": "AMD",
                "next_page_token": null
            })))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(test_config(server.uri()));
        let (tx, mut rx) = mpsc::channel(16);
        let notify = Arc::new(Notify::new());

        execute_one(
            Command::FetchIntradayBars("AMD".into()),
            &tx,
            &client,
            &notify,
        )
        .await;

        let events = collect_events(&mut rx).await;
        let received = events
            .iter()
            .find(|e| matches!(e, Event::IntradayBarsReceived { symbol, .. } if symbol == "AMD"));
        assert!(received.is_some(), "expected IntradayBarsReceived for AMD");
        if let Some(Event::IntradayBarsReceived { bars, .. }) = received {
            assert_eq!(bars.len(), 2);
            assert_eq!(bars[0], 14285); // $142.85 → 14285 cents
            assert_eq!(bars[1], 14350); // $143.50 → 14350 cents
        }
    }

    #[tokio::test]
    async fn fetch_intraday_bars_api_error_does_not_emit_event() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/stocks/ERR/bars"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(test_config(server.uri()));
        let (tx, mut rx) = mpsc::channel(16);
        let notify = Arc::new(Notify::new());

        execute_one(
            Command::FetchIntradayBars("ERR".into()),
            &tx,
            &client,
            &notify,
        )
        .await;

        let events = collect_events(&mut rx).await;
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::IntradayBarsReceived { .. })),
            "error response should not emit IntradayBarsReceived"
        );
    }
}
