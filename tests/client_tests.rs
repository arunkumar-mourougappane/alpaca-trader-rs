use alpaca_trader_rs::{
    client::AlpacaClient,
    config::{AlpacaConfig, AlpacaEnv},
    types::OrderRequest,
};
use serde_json::json;
use wiremock::{
    matchers::{header, method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

fn test_config(base_url: String) -> AlpacaConfig {
    AlpacaConfig {
        base_url,
        key: "PKTEST000".into(),
        secret: "secret000".into(),
        env: AlpacaEnv::Paper,
    }
}

fn account_json() -> serde_json::Value {
    json!({
        "status": "ACTIVE",
        "equity": "100000",
        "buying_power": "200000",
        "cash": "100000",
        "long_market_value": "0",
        "daytrade_count": 0,
        "pattern_day_trader": false,
        "currency": "USD"
    })
}

fn order_json(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "symbol": "AAPL",
        "side": "buy",
        "qty": "10",
        "order_type": "limit",
        "limit_price": "185.00",
        "status": "accepted",
        "submitted_at": "2026-05-09T10:00:00Z",
        "filled_qty": "0",
        "time_in_force": "day"
    })
}

fn watchlist_json(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": "Primary",
        "assets": [{
            "id": "asset-1",
            "symbol": "AAPL",
            "name": "Apple Inc.",
            "exchange": "NASDAQ",
            "class": "us_equity",
            "tradable": true,
            "shortable": true,
            "fractionable": true,
            "easy_to_borrow": true
        }]
    })
}

// ── Auth headers ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn auth_headers_present_on_every_request() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/account"))
        .and(header("APCA-API-KEY-ID", "PKTEST000"))
        .and(header("APCA-API-SECRET-KEY", "secret000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(account_json()))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let result = client.get_account().await;
    assert!(result.is_ok(), "request failed: {:?}", result.err());
}

// ── get_account ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_account_deserializes_all_fields() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/account"))
        .respond_with(ResponseTemplate::new(200).set_body_json(account_json()))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let acc = client.get_account().await.unwrap();

    assert_eq!(acc.status, "ACTIVE");
    assert_eq!(acc.equity, "100000");
    assert_eq!(acc.buying_power, "200000");
    assert_eq!(acc.daytrade_count, 0);
    assert!(!acc.pattern_day_trader);
}

#[tokio::test]
async fn get_account_500_returns_err() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/account"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    assert!(client.get_account().await.is_err());
}

// ── get_positions ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_positions_empty_returns_vec() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/positions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let positions = client.get_positions().await.unwrap();
    assert!(positions.is_empty());
}

#[tokio::test]
async fn get_positions_populated_deserializes() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/positions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "symbol": "AAPL",
            "qty": "10",
            "avg_entry_price": "170.00",
            "current_price": "185.00",
            "market_value": "1850.00",
            "unrealized_pl": "150.00",
            "unrealized_plpc": "0.0882",
            "side": "long"
        }])))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let positions = client.get_positions().await.unwrap();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].symbol, "AAPL");
    assert_eq!(positions[0].qty, "10");
}

// ── get_orders ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_orders_sends_status_query_param() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/orders"))
        .and(query_param("status", "all"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let result = client.get_orders("all").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn get_orders_notional_order_qty_is_none() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "id": "o1",
            "symbol": "AAPL",
            "side": "buy",
            "qty": null,
            "notional": "500",
            "order_type": "market",
            "status": "accepted",
            "filled_qty": "0",
            "time_in_force": "day"
        }])))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let orders = client.get_orders("all").await.unwrap();
    assert!(orders[0].qty.is_none());
    assert_eq!(orders[0].notional.as_deref(), Some("500"));
}

// ── submit_order ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn submit_order_posts_to_correct_path() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(order_json("new-order-id")))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let req = OrderRequest {
        symbol: "AAPL".into(),
        qty: Some("10".into()),
        notional: None,
        side: "buy".into(),
        order_type: "limit".into(),
        time_in_force: "day".into(),
        limit_price: Some("185.00".into()),
    };
    let order = client.submit_order(&req).await.unwrap();
    assert_eq!(order.id, "new-order-id");
    assert_eq!(order.symbol, "AAPL");
}

// ── cancel_order ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn cancel_order_sends_delete_to_correct_path() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/orders/order-abc-123"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let result = client.cancel_order("order-abc-123").await;
    assert!(result.is_ok());
}

// ── get_clock ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_clock_parses_is_open_false() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/clock"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "is_open": false,
            "next_open": "2026-05-12T13:30:00Z",
            "next_close": "2026-05-12T20:00:00Z",
            "timestamp": "2026-05-11T12:00:00Z"
        })))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let clock = client.get_clock().await.unwrap();
    assert!(!clock.is_open);
    assert_eq!(clock.next_open, "2026-05-12T13:30:00Z");
}

// ── list_watchlists ───────────────────────────────────────────────────────────

#[tokio::test]
async fn list_watchlists_returns_summaries_without_assets() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/watchlists"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": "wl-1", "name": "Primary"},
            {"id": "wl-2", "name": "Tech"}
        ])))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let summaries = client.list_watchlists().await.unwrap();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].id, "wl-1");
    assert_eq!(summaries[1].name, "Tech");
}

// ── get_watchlist ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_watchlist_returns_full_asset_list() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/watchlists/wl-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(watchlist_json("wl-1")))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let wl = client.get_watchlist("wl-1").await.unwrap();
    assert_eq!(wl.id, "wl-1");
    assert_eq!(wl.assets.len(), 1);
    assert_eq!(wl.assets[0].symbol, "AAPL");
    assert!(wl.assets[0].tradable);
}

// ── add_to_watchlist ──────────────────────────────────────────────────────────

#[tokio::test]
async fn add_to_watchlist_posts_to_correct_path() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/watchlists/wl-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(watchlist_json("wl-1")))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let wl = client.add_to_watchlist("wl-1", "AAPL").await.unwrap();
    assert_eq!(wl.id, "wl-1");
}

// ── remove_from_watchlist ─────────────────────────────────────────────────────

#[tokio::test]
async fn remove_from_watchlist_deletes_correct_path() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/watchlists/wl-1/AAPL"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "wl-1",
            "name": "Primary",
            "assets": []
        })))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let wl = client.remove_from_watchlist("wl-1", "AAPL").await.unwrap();
    assert!(wl.assets.is_empty());
}
