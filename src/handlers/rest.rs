use std::sync::Arc;
use std::time::Duration;

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
        AlpacaConfig { base_url, key: "PKTEST".into(), secret: "secret".into(), env: AlpacaEnv::Paper }
    }

    async fn mount_all(server: &MockServer) {
        Mock::given(method("GET")).and(path("/account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ACTIVE", "equity": "100000", "buying_power": "200000",
                "cash": "100000", "long_market_value": "0",
                "daytrade_count": 0, "pattern_day_trader": false, "currency": "USD"
            }))).mount(server).await;

        Mock::given(method("GET")).and(path("/positions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(server).await;

        Mock::given(method("GET")).and(path("/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(server).await;

        Mock::given(method("GET")).and(path("/clock"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "is_open": false,
                "next_open": "2026-05-12T13:30:00Z",
                "next_close": "2026-05-12T20:00:00Z",
                "timestamp": "2026-05-11T12:00:00Z"
            }))).mount(server).await;

        Mock::given(method("GET")).and(path("/watchlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"id": "wl-id-1", "name": "Primary"}
            ]))).mount(server).await;

        Mock::given(method("GET")).and(path("/watchlists/wl-id-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "wl-id-1", "name": "Primary", "assets": []
            }))).mount(server).await;
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

        assert!(events.iter().any(|e| matches!(e, Event::AccountUpdated(_))), "missing AccountUpdated");
        assert!(events.iter().any(|e| matches!(e, Event::PositionsUpdated(_))), "missing PositionsUpdated");
        assert!(events.iter().any(|e| matches!(e, Event::OrdersUpdated(_))), "missing OrdersUpdated");
        assert!(events.iter().any(|e| matches!(e, Event::ClockUpdated(_))), "missing ClockUpdated");
        assert!(events.iter().any(|e| matches!(e, Event::WatchlistUpdated(_))), "missing WatchlistUpdated");
    }

    #[tokio::test]
    async fn poll_once_account_error_sends_status_msg() {
        let server = MockServer::start().await;

        Mock::given(method("GET")).and(path("/account"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/positions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/clock"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/watchlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server).await;

        let client = Arc::new(AlpacaClient::new(test_config(server.uri())));
        let (tx, mut rx) = mpsc::channel(32);
        poll_once(tx, client).await;

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(events.iter().any(|e| matches!(e, Event::StatusMsg(m) if m.contains("Account error"))));
    }

    #[tokio::test]
    async fn poll_once_empty_watchlist_list_skips_watchlist_fetch() {
        let server = MockServer::start().await;

        Mock::given(method("GET")).and(path("/account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ACTIVE", "equity": "0", "buying_power": "0",
                "cash": "0", "long_market_value": "0",
                "daytrade_count": 0, "pattern_day_trader": false, "currency": "USD"
            }))).mount(&server).await;
        Mock::given(method("GET")).and(path("/positions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server).await;
        Mock::given(method("GET")).and(path("/clock"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "is_open": false, "next_open": "", "next_close": "", "timestamp": ""
            }))).mount(&server).await;
        Mock::given(method("GET")).and(path("/watchlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server).await;

        let client = Arc::new(AlpacaClient::new(test_config(server.uri())));
        let (tx, mut rx) = mpsc::channel(32);
        poll_once(tx, client).await;

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(!events.iter().any(|e| matches!(e, Event::WatchlistUpdated(_))));
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
}

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
    poll_all(&client, &tx).await;
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
            let _ = tx.send(Event::WatchlistUpdated(w)).await;
        }
        Err(e) => {
            let _ = tx
                .send(Event::StatusMsg(format!("Watchlist error: {}", e)))
                .await;
        }
    }
}
