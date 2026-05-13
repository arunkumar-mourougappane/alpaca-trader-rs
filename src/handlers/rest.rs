use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc::Sender, Notify};
use tokio_util::sync::CancellationToken;

use crate::client::AlpacaClient;
use crate::events::Event;

pub async fn run(
    tx: Sender<Event>,
    cancel: CancellationToken,
    client: Arc<AlpacaClient>,
    refresh_notify: Arc<Notify>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                poll_all(&client, &tx).await;
            }
            _ = refresh_notify.notified() => {
                poll_all(&client, &tx).await;
                let _ = tx.send(Event::StatusMsg(String::new())).await;
            }
            _ = cancel.cancelled() => break,
        }
    }
}

pub async fn poll_once(tx: Sender<Event>, client: Arc<AlpacaClient>) {
    tokio::join!(poll_all(&client, &tx), poll_portfolio_history(&client, &tx));
}

async fn poll_all(client: &AlpacaClient, tx: &Sender<Event>) {
    tokio::join!(
        poll_account(client, tx),
        poll_positions(client, tx),
        poll_orders(client, tx),
        poll_clock(client, tx),
        poll_watchlist(client, tx),
    );
}

async fn poll_account(client: &AlpacaClient, tx: &Sender<Event>) {
    match client.get_account().await {
        Ok(a) => {
            let _ = tx.send(Event::AccountUpdated(a)).await;
        }
        Err(e) => {
            let _ = tx
                .send(Event::StatusMsg(format!("Account error: {}", e)))
                .await;
        }
    }
}

async fn poll_positions(client: &AlpacaClient, tx: &Sender<Event>) {
    match client.get_positions().await {
        Ok(p) => {
            let _ = tx.send(Event::PositionsUpdated(p)).await;
        }
        Err(e) => {
            let _ = tx
                .send(Event::StatusMsg(format!("Positions error: {}", e)))
                .await;
        }
    }
}

async fn poll_orders(client: &AlpacaClient, tx: &Sender<Event>) {
    match client.get_orders("all").await {
        Ok(o) => {
            let _ = tx.send(Event::OrdersUpdated(o)).await;
        }
        Err(e) => {
            let _ = tx
                .send(Event::StatusMsg(format!("Orders error: {}", e)))
                .await;
        }
    }
}

async fn poll_clock(client: &AlpacaClient, tx: &Sender<Event>) {
    if let Ok(c) = client.get_clock().await {
        let _ = tx.send(Event::ClockUpdated(c)).await;
    }
}

async fn poll_watchlist(client: &AlpacaClient, tx: &Sender<Event>) {
    let summaries = match client.list_watchlists().await {
        Ok(s) => s,
        Err(e) => {
            let _ = tx
                .send(Event::StatusMsg(format!("Watchlist error: {}", e)))
                .await;
            return;
        }
    };
    if summaries.is_empty() {
        return;
    }
    match client.get_watchlist(&summaries[0].id).await {
        Ok(w) => {
            let symbols: Vec<String> = w.assets.iter().map(|a| a.symbol.clone()).collect();
            let _ = tx.send(Event::WatchlistUpdated(w)).await;
            poll_snapshots(client, tx, &symbols).await;
        }
        Err(e) => {
            let _ = tx
                .send(Event::StatusMsg(format!("Watchlist error: {}", e)))
                .await;
        }
    }
}

async fn poll_snapshots(client: &AlpacaClient, tx: &Sender<Event>, symbols: &[String]) {
    if symbols.is_empty() {
        return;
    }
    match client.get_snapshots(symbols).await {
        Ok(snapshots) => {
            let _ = tx.send(Event::SnapshotsUpdated(snapshots)).await;
        }
        Err(e) => {
            tracing::warn!("Snapshots unavailable: {}", e);
        }
    }
}

async fn poll_portfolio_history(client: &AlpacaClient, tx: &Sender<Event>) {
    match client.get_portfolio_history().await {
        Ok(h) => {
            let data: Vec<f64> = h.equity.into_iter().flatten().collect();
            if !data.is_empty() {
                let _ = tx.send(Event::PortfolioHistoryLoaded(data)).await;
            }
        }
        Err(e) => {
            tracing::warn!("Portfolio history unavailable: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::AlpacaClient;
    use crate::config::{AlpacaConfig, AlpacaEnv};
    use crate::events::Event;
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_config(base_url: String) -> AlpacaConfig {
        AlpacaConfig {
            base_url,
            key: "PKTEST".into(),
            secret: "secret".into(),
            env: AlpacaEnv::Paper,
        }
    }

    async fn mount_all(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ACTIVE", "equity": "100000", "buying_power": "200000",
                "cash": "100000", "long_market_value": "0",
                "daytrade_count": 0, "pattern_day_trader": false, "currency": "USD"
            })))
            .mount(server)
            .await;

        Mock::given(method("GET"))
            .and(path("/positions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(server)
            .await;

        Mock::given(method("GET"))
            .and(path("/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(server)
            .await;

        Mock::given(method("GET"))
            .and(path("/clock"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "is_open": false,
                "next_open": "2026-05-12T13:30:00Z",
                "next_close": "2026-05-12T20:00:00Z",
                "timestamp": "2026-05-11T12:00:00Z"
            })))
            .mount(server)
            .await;

        Mock::given(method("GET"))
            .and(path("/watchlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"id": "wl-id-1", "name": "Primary"}
            ])))
            .mount(server)
            .await;

        Mock::given(method("GET"))
            .and(path("/watchlists/wl-id-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "wl-id-1", "name": "Primary", "assets": []
            })))
            .mount(server)
            .await;

        Mock::given(method("GET"))
            .and(path("/stocks/snapshots"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(server)
            .await;

        Mock::given(method("GET"))
            .and(path("/account/portfolio/history"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "equity": [100000.0, 100100.5, null, 100200.0],
                "timestamp": [1000, 1060, 1120, 1180],
                "profit_loss": [0.0, 100.5, null, 200.0],
                "profit_loss_pct": [0.0, 0.001, null, 0.002],
                "base_value": 100000.0,
                "timeframe": "1Min"
            })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn poll_once_sends_all_five_event_types() {
        let server = MockServer::start().await;
        mount_all(&server).await;

        let client = Arc::new(AlpacaClient::new(test_config(server.uri())));
        let (tx, mut rx) = mpsc::channel(32);
        poll_once(tx, client).await;

        let mut events = vec![];
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }

        assert!(
            events.iter().any(|e| matches!(e, Event::AccountUpdated(_))),
            "missing AccountUpdated"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::PositionsUpdated(_))),
            "missing PositionsUpdated"
        );
        assert!(
            events.iter().any(|e| matches!(e, Event::OrdersUpdated(_))),
            "missing OrdersUpdated"
        );
        assert!(
            events.iter().any(|e| matches!(e, Event::ClockUpdated(_))),
            "missing ClockUpdated"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::WatchlistUpdated(_))),
            "missing WatchlistUpdated"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::PortfolioHistoryLoaded(_))),
            "missing PortfolioHistoryLoaded"
        );
    }

    #[tokio::test]
    async fn poll_once_account_error_sends_status_msg() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/account"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/positions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/clock"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/watchlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;

        let client = Arc::new(AlpacaClient::new(test_config(server.uri())));
        let (tx, mut rx) = mpsc::channel(32);
        poll_once(tx, client).await;

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StatusMsg(m) if m.contains("Account error"))));
    }

    #[tokio::test]
    async fn poll_once_empty_watchlist_list_skips_watchlist_fetch() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ACTIVE", "equity": "0", "buying_power": "0",
                "cash": "0", "long_market_value": "0",
                "daytrade_count": 0, "pattern_day_trader": false, "currency": "USD"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/positions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/clock"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "is_open": false, "next_open": "", "next_close": "", "timestamp": ""
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/watchlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;

        let client = Arc::new(AlpacaClient::new(test_config(server.uri())));
        let (tx, mut rx) = mpsc::channel(32);
        poll_once(tx, client).await;

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(!events
            .iter()
            .any(|e| matches!(e, Event::WatchlistUpdated(_))));
    }

    #[tokio::test]
    async fn run_cancels_cleanly() {
        let server = MockServer::start().await;
        let client = Arc::new(AlpacaClient::new(test_config(server.uri())));
        let (tx, _rx) = mpsc::channel(32);
        let cancel = CancellationToken::new();
        let notify = Arc::new(Notify::new());

        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(run(tx, cancel_clone, client, notify));

        // Cancel immediately and wait — should not hang
        cancel.cancel();
        tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("run() did not exit within 2 seconds")
            .unwrap();
    }

    #[tokio::test]
    async fn poll_once_sends_portfolio_history_with_nulls_filtered() {
        let server = MockServer::start().await;
        mount_all(&server).await;

        let client = Arc::new(AlpacaClient::new(test_config(server.uri())));
        let (tx, mut rx) = mpsc::channel(32);
        poll_once(tx, client).await;

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        let history_event = events
            .iter()
            .find_map(|e| {
                if let Event::PortfolioHistoryLoaded(data) = e {
                    Some(data)
                } else {
                    None
                }
            })
            .expect("PortfolioHistoryLoaded should be emitted");

        // mount_all provides [100000.0, 100100.5, null, 100200.0]
        // null is filtered out → 3 values
        assert_eq!(history_event.len(), 3);
        assert!((history_event[0] - 100000.0).abs() < 0.01);
        assert!((history_event[1] - 100100.5).abs() < 0.01);
        assert!((history_event[2] - 100200.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn poll_once_portfolio_history_error_is_silently_ignored() {
        let server = MockServer::start().await;
        mount_all(&server).await;

        // Override portfolio history with a 500 error by pointing at a fresh server
        // that has no mocks (all unmocked paths → wiremock returns 404).
        // We only need to confirm no PortfolioHistoryLoaded arrives when the call fails.
        let err_server = MockServer::start().await;
        // Mount all except portfolio history on err_server so other events arrive.
        Mock::given(method("GET"))
            .and(path("/account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ACTIVE", "equity": "100000", "buying_power": "200000",
                "cash": "100000", "long_market_value": "0",
                "daytrade_count": 0, "pattern_day_trader": false, "currency": "USD"
            })))
            .mount(&err_server)
            .await;
        Mock::given(method("GET"))
            .and(path("/positions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&err_server)
            .await;
        Mock::given(method("GET"))
            .and(path("/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&err_server)
            .await;
        Mock::given(method("GET"))
            .and(path("/clock"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "is_open": false, "next_open": "", "next_close": "", "timestamp": ""
            })))
            .mount(&err_server)
            .await;
        Mock::given(method("GET"))
            .and(path("/watchlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&err_server)
            .await;
        // No mock for /account/portfolio/history → wiremock returns 500-ish

        let client = Arc::new(AlpacaClient::new(test_config(err_server.uri())));
        let (tx, mut rx) = mpsc::channel(32);
        poll_once(tx, client).await;

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::PortfolioHistoryLoaded(_))),
            "portfolio history error must not emit PortfolioHistoryLoaded"
        );
    }

    #[tokio::test]
    async fn poll_once_portfolio_history_all_null_does_not_emit_event() {
        let server = MockServer::start().await;

        // Minimal mocks so poll_all doesn't fail loudly
        Mock::given(method("GET"))
            .and(path("/account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ACTIVE", "equity": "0", "buying_power": "0",
                "cash": "0", "long_market_value": "0",
                "daytrade_count": 0, "pattern_day_trader": false, "currency": "USD"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/positions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/clock"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "is_open": false, "next_open": "", "next_close": "", "timestamp": ""
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/watchlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        // All equity values are null (market closed all day)
        Mock::given(method("GET"))
            .and(path("/account/portfolio/history"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "equity": [null, null, null],
                "timestamp": [1000, 1060, 1120],
                "profit_loss": [null, null, null],
                "profit_loss_pct": [null, null, null],
                "base_value": 0.0,
                "timeframe": "1Min"
            })))
            .mount(&server)
            .await;

        let client = Arc::new(AlpacaClient::new(test_config(server.uri())));
        let (tx, mut rx) = mpsc::channel(32);
        poll_once(tx, client).await;

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::PortfolioHistoryLoaded(_))),
            "all-null equity must not emit PortfolioHistoryLoaded"
        );
    }

    #[tokio::test]
    async fn poll_watchlist_with_symbols_emits_snapshots_updated() {
        use wiremock::matchers::query_param;

        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/watchlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"id": "wl-snap-1", "name": "Snap Test"}
            ])))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/watchlists/wl-snap-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "wl-snap-1",
                "name": "Snap Test",
                "assets": [
                    {
                        "id": "asset-aapl",
                        "symbol": "AAPL",
                        "name": "Apple Inc",
                        "exchange": "NASDAQ",
                        "class": "us_equity",
                        "tradable": true,
                        "shortable": true,
                        "fractionable": true,
                        "easy_to_borrow": true
                    }
                ]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/stocks/snapshots"))
            .and(query_param("symbols", "AAPL"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "AAPL": {
                    "dailyBar": { "c": 175.5, "v": 1234567.0 },
                    "prevDailyBar": { "c": 170.0, "v": 987654.0 }
                }
            })))
            .mount(&server)
            .await;

        let client = Arc::new(AlpacaClient::new(test_config(server.uri())));
        let (tx, mut rx) = mpsc::channel(32);
        poll_watchlist(&client, &tx).await;

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();

        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::WatchlistUpdated(_))),
            "must emit WatchlistUpdated"
        );
        let snap_event = events
            .iter()
            .find_map(|e| {
                if let Event::SnapshotsUpdated(s) = e {
                    Some(s)
                } else {
                    None
                }
            })
            .expect("must emit SnapshotsUpdated");

        let aapl = snap_event.get("AAPL").expect("AAPL snapshot expected");
        let daily = aapl.daily_bar.as_ref().expect("dailyBar expected");
        assert!((daily.v - 1_234_567.0).abs() < 1.0);
        let prev = aapl.prev_daily_bar.as_ref().expect("prevDailyBar expected");
        assert!((prev.c - 170.0).abs() < 0.01);
    }
}
