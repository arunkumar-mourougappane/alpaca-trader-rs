//! Async HTTP client wrapping the Alpaca Markets REST API.
use std::collections::HashMap;

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue};

use crate::config::{AlpacaConfig, AlpacaEnv};
use crate::types::{
    AccountInfo, BarsResponse, MarketClock, MinuteBar, Order, OrderRequest, PortfolioHistory,
    Position, Snapshot, Watchlist, WatchlistSummary,
};

/// Async HTTP client for the Alpaca Markets REST API.
///
/// All methods require valid credentials in the [`AlpacaConfig`] provided at
/// construction time. Each call sets the `APCA-API-KEY-ID` and
/// `APCA-API-SECRET-KEY` request headers automatically.
pub struct AlpacaClient {
    http: reqwest::Client,
    config: AlpacaConfig,
}

impl AlpacaClient {
    /// Create a new client using the given configuration.
    pub fn new(config: AlpacaConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
        }
    }

    /// Returns `true` when this client is configured for the paper trading environment.
    ///
    /// Useful for skipping API calls that are unsupported on the paper endpoint
    /// (e.g., `/v2/watchlists`).
    pub fn is_paper(&self) -> bool {
        self.config.env == AlpacaEnv::Paper
    }

    /// Returns `true` when dry-run mode is active.
    ///
    /// In dry-run mode, order-submission calls are simulated locally and never
    /// forwarded to the Alpaca API. All read-only calls still reach the
    /// configured endpoint.
    pub fn is_dry_run(&self) -> bool {
        self.config.dry_run
    }

    fn auth_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "APCA-API-KEY-ID",
            HeaderValue::from_str(&self.config.key)
                .context("API key contains invalid header characters")?,
        );
        headers.insert(
            "APCA-API-SECRET-KEY",
            HeaderValue::from_str(&self.config.secret)
                .context("API secret contains invalid header characters")?,
        );
        Ok(headers)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url, path)
    }

    /// Build a URL against the Alpaca market-data API (`data.alpaca.markets`).
    ///
    /// In production the data API lives on a separate host from the broker API.
    /// For local tests (base URL is not `alpaca.markets`) we fall back to the
    /// configured base URL so wiremock mocks work without a second server.
    fn data_url(&self, path: &str) -> String {
        if self.config.base_url.contains("alpaca.markets") {
            format!("https://data.alpaca.markets/v2{}", path)
        } else {
            format!("{}{}", self.config.base_url, path)
        }
    }

    /// Fetch the current account snapshot (`GET /account`).
    pub async fn get_account(&self) -> Result<AccountInfo> {
        self.http
            .get(self.url("/account"))
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("GET /account request failed")?
            .json::<AccountInfo>()
            .await
            .context("GET /account parse failed")
    }

    /// Fetch all current open positions (`GET /positions`).
    pub async fn get_positions(&self) -> Result<Vec<Position>> {
        self.http
            .get(self.url("/positions"))
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("GET /positions request failed")?
            .json::<Vec<Position>>()
            .await
            .context("GET /positions parse failed")
    }

    /// Fetch orders filtered by `status` (`GET /orders?status=<status>&limit=100`).
    ///
    /// Common values for `status`: `"open"`, `"closed"`, `"all"`.
    pub async fn get_orders(&self, status: &str) -> Result<Vec<Order>> {
        self.http
            .get(self.url("/orders"))
            .query(&[("status", status), ("limit", "100")])
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("GET /orders request failed")?
            .json::<Vec<Order>>()
            .await
            .context("GET /orders parse failed")
    }

    /// Submit a new order (`POST /orders`).
    ///
    /// Returns the created [`Order`] with its assigned `id` and initial status.
    pub async fn submit_order(&self, req: &OrderRequest) -> Result<Order> {
        self.http
            .post(self.url("/orders"))
            .headers(self.auth_headers()?)
            .json(req)
            .send()
            .await
            .context("POST /orders request failed")?
            .json::<Order>()
            .await
            .context("POST /orders parse failed")
    }

    /// Cancel an open order by its ID (`DELETE /orders/{id}`).
    pub async fn cancel_order(&self, id: &str) -> Result<()> {
        self.http
            .delete(self.url(&format!("/orders/{}", id)))
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("DELETE /orders/{id} request failed")?;
        Ok(())
    }

    /// Fetch the current market clock (`GET /clock`).
    ///
    /// Returns whether the market is open and the next open/close times.
    pub async fn get_clock(&self) -> Result<MarketClock> {
        self.http
            .get(self.url("/clock"))
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("GET /clock request failed")?
            .json::<MarketClock>()
            .await
            .context("GET /clock parse failed")
    }

    /// List all watchlists for the account (`GET /watchlists`).
    ///
    /// Returns summaries (id + name only). Use [`get_watchlist`] to fetch full asset lists.
    ///
    /// [`get_watchlist`]: AlpacaClient::get_watchlist
    pub async fn list_watchlists(&self) -> Result<Vec<WatchlistSummary>> {
        self.http
            .get(self.url("/watchlists"))
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("GET /watchlists request failed")?
            .json::<Vec<WatchlistSummary>>()
            .await
            .context("GET /watchlists parse failed")
    }

    /// Fetch a watchlist including its full asset list (`GET /watchlists/{id}`).
    pub async fn get_watchlist(&self, id: &str) -> Result<Watchlist> {
        self.http
            .get(self.url(&format!("/watchlists/{}", id)))
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("GET /watchlists/{id} request failed")?
            .json::<Watchlist>()
            .await
            .context("GET /watchlists/{id} parse failed")
    }

    /// Add a symbol to a watchlist (`POST /watchlists/{id}`).
    ///
    /// Returns the updated [`Watchlist`] with the new symbol included.
    pub async fn add_to_watchlist(&self, id: &str, symbol: &str) -> Result<Watchlist> {
        let body = serde_json::json!({ "symbol": symbol });
        self.http
            .post(self.url(&format!("/watchlists/{}", id)))
            .headers(self.auth_headers()?)
            .json(&body)
            .send()
            .await
            .context("POST /watchlists/{id} request failed")?
            .json::<Watchlist>()
            .await
            .context("POST /watchlists/{id} parse failed")
    }

    /// Remove a symbol from a watchlist (`DELETE /watchlists/{id}/{symbol}`).
    ///
    /// Returns the updated [`Watchlist`] with the symbol removed.
    pub async fn remove_from_watchlist(&self, id: &str, symbol: &str) -> Result<Watchlist> {
        self.http
            .delete(self.url(&format!("/watchlists/{}/{}", id, symbol)))
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("DELETE /watchlists/{id}/{symbol} request failed")?
            .json::<Watchlist>()
            .await
            .context("DELETE /watchlists/{id}/{symbol} parse failed")
    }

    /// Fetch portfolio equity history (`GET /account/portfolio/history`).
    ///
    /// `period` and `timeframe` map directly to the Alpaca API query parameters.
    /// Typical combinations:
    /// - `("1D",  "1Min")` — intraday 1-minute bars (default)
    /// - `("1W",  "1H")`  — past week, hourly bars
    /// - `("1M",  "1D")`  — past month, daily bars
    /// - `("YTD", "1D")`  — year-to-date, daily bars
    pub async fn get_portfolio_history(
        &self,
        period: &str,
        timeframe: &str,
    ) -> Result<PortfolioHistory> {
        self.http
            .get(self.url("/account/portfolio/history"))
            .query(&[("timeframe", timeframe), ("period", period)])
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("GET /account/portfolio/history request failed")?
            .json::<PortfolioHistory>()
            .await
            .context("GET /account/portfolio/history parse failed")
    }

    /// Fetch latest market snapshots for multiple symbols from the data API
    /// (`GET /v2/stocks/snapshots?symbols=...&feed=iex`).
    ///
    /// Returns a map of symbol → [`Snapshot`] with daily bar data (volume) and
    /// the previous day's bar (previous close for Change% computation).
    /// Returns an empty map if `symbols` is empty.
    pub async fn get_snapshots(&self, symbols: &[String]) -> Result<HashMap<String, Snapshot>> {
        if symbols.is_empty() {
            return Ok(HashMap::new());
        }
        let symbols_param = symbols.join(",");
        self.http
            .get(self.data_url("/stocks/snapshots"))
            .query(&[("symbols", symbols_param.as_str()), ("feed", "iex")])
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("GET /stocks/snapshots request failed")?
            .json::<HashMap<String, Snapshot>>()
            .await
            .context("GET /stocks/snapshots parse failed")
    }

    /// Fetch intraday 1-minute bars for a single symbol from the data API.
    ///
    /// Returns bars for the current UTC calendar date, IEX feed, oldest first.
    /// The caller converts the raw bars to sparkline-ready `u64` cent values.
    pub async fn get_intraday_bars(&self, symbol: &str) -> Result<Vec<MinuteBar>> {
        use chrono::Utc;
        let today = Utc::now().format("%Y-%m-%d").to_string();
        Ok(self
            .http
            .get(self.data_url(&format!("/stocks/{symbol}/bars")))
            .query(&[
                ("timeframe", "1Min"),
                ("start", today.as_str()),
                ("feed", "iex"),
                ("limit", "400"),
            ])
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("GET /stocks/{symbol}/bars request failed")?
            .json::<BarsResponse>()
            .await
            .context("GET /stocks/{symbol}/bars parse failed")?
            .bars)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AlpacaConfig, AlpacaEnv};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn paper_config(base_url: String) -> AlpacaConfig {
        AlpacaConfig {
            base_url,
            key: "PKTEST".into(),
            secret: "secret".into(),
            env: AlpacaEnv::Paper,
            dry_run: false,
        }
    }

    fn live_config(base_url: String) -> AlpacaConfig {
        AlpacaConfig {
            base_url,
            key: "AKTEST".into(),
            secret: "secret".into(),
            env: AlpacaEnv::Live,
            dry_run: false,
        }
    }

    // ── Unit tests ────────────────────────────────────────────────────────────

    #[test]
    fn is_paper_true_for_paper_env() {
        let client = AlpacaClient::new(paper_config("http://localhost".into()));
        assert!(client.is_paper());
    }

    #[test]
    fn is_paper_false_for_live_env() {
        let client = AlpacaClient::new(live_config("http://localhost".into()));
        assert!(!client.is_paper());
    }

    #[test]
    fn is_dry_run_false_by_default() {
        let client = AlpacaClient::new(paper_config("http://localhost".into()));
        assert!(!client.is_dry_run());
    }

    #[test]
    fn is_dry_run_true_when_set() {
        let config = AlpacaConfig {
            base_url: "http://localhost".into(),
            key: "k".into(),
            secret: "s".into(),
            env: AlpacaEnv::Paper,
            dry_run: true,
        };
        let client = AlpacaClient::new(config);
        assert!(client.is_dry_run());
    }

    #[test]
    fn data_url_uses_data_alpaca_markets_for_production() {
        let config = AlpacaConfig {
            base_url: "https://paper-api.alpaca.markets".into(),
            key: "k".into(),
            secret: "s".into(),
            env: AlpacaEnv::Paper,
            dry_run: false,
        };
        let client = AlpacaClient::new(config);
        assert_eq!(
            client.data_url("/stocks/snapshots"),
            "https://data.alpaca.markets/v2/stocks/snapshots"
        );
    }

    #[test]
    fn data_url_uses_base_url_for_non_production() {
        let client = AlpacaClient::new(paper_config("http://localhost:9999".into()));
        assert_eq!(
            client.data_url("/stocks/snapshots"),
            "http://localhost:9999/stocks/snapshots"
        );
    }

    #[tokio::test]
    async fn get_snapshots_returns_empty_map_without_request_when_no_symbols() {
        // No mock server — any HTTP would panic/fail; proves no request is made.
        let client = AlpacaClient::new(paper_config("http://127.0.0.1:1".into()));
        let result = client.get_snapshots(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    // ── HTTP integration tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn get_account_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/account"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ACTIVE", "equity": "100000", "buying_power": "200000",
                "cash": "50000", "long_market_value": "50000",
                "daytrade_count": 0, "pattern_day_trader": false, "currency": "USD"
            })))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(paper_config(server.uri()));
        let acct = client.get_account().await.unwrap();
        assert_eq!(acct.status, "ACTIVE");
        assert_eq!(acct.equity, "100000");
    }

    #[tokio::test]
    async fn get_account_returns_error_on_bad_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/account"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(paper_config(server.uri()));
        let err = client.get_account().await.unwrap_err();
        assert!(err.to_string().contains("parse failed"));
    }

    #[tokio::test]
    async fn get_positions_parses_empty_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/positions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(paper_config(server.uri()));
        let positions = client.get_positions().await.unwrap();
        assert!(positions.is_empty());
    }

    #[tokio::test]
    async fn get_orders_parses_empty_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(paper_config(server.uri()));
        let orders = client.get_orders("open").await.unwrap();
        assert!(orders.is_empty());
    }

    #[tokio::test]
    async fn submit_order_sends_post_and_parses_response() {
        use crate::types::OrderRequest;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/orders"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "ord-1", "symbol": "AAPL", "side": "buy",
                "order_type": "market", "status": "accepted",
                "filled_qty": "0", "time_in_force": "day"
            })))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(paper_config(server.uri()));
        let req = OrderRequest {
            symbol: "AAPL".into(),
            qty: Some("1".into()),
            notional: None,
            side: "buy".into(),
            order_type: "market".into(),
            time_in_force: "day".into(),
            limit_price: None,
        };
        let order = client.submit_order(&req).await.unwrap();
        assert_eq!(order.symbol, "AAPL");
        assert_eq!(order.status, "accepted");
    }

    #[tokio::test]
    async fn cancel_order_succeeds_on_204() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/orders/order-abc"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(paper_config(server.uri()));
        client.cancel_order("order-abc").await.unwrap();
    }

    #[tokio::test]
    async fn get_clock_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/clock"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "is_open": true,
                "next_open": "2026-05-13T13:30:00Z",
                "next_close": "2026-05-12T20:00:00Z",
                "timestamp": "2026-05-12T15:00:00Z"
            })))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(paper_config(server.uri()));
        let clock = client.get_clock().await.unwrap();
        assert!(clock.is_open);
    }

    #[tokio::test]
    async fn list_watchlists_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/watchlists"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"id": "wl-1", "name": "My List"}
            ])))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(live_config(server.uri()));
        let lists = client.list_watchlists().await.unwrap();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].name, "My List");
    }

    #[tokio::test]
    async fn get_watchlist_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/watchlists/wl-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "wl-1", "name": "My List", "assets": []
            })))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(live_config(server.uri()));
        let wl = client.get_watchlist("wl-1").await.unwrap();
        assert_eq!(wl.name, "My List");
        assert!(wl.assets.is_empty());
    }

    #[tokio::test]
    async fn add_to_watchlist_returns_updated_watchlist() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/watchlists/wl-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "wl-1", "name": "My List",
                "assets": [{
                    "id": "a-1", "symbol": "AAPL", "name": "Apple Inc.",
                    "exchange": "NASDAQ", "class": "us_equity",
                    "tradable": true, "shortable": true, "fractionable": true
                }]
            })))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(live_config(server.uri()));
        let wl = client.add_to_watchlist("wl-1", "AAPL").await.unwrap();
        assert_eq!(wl.assets.len(), 1);
        assert_eq!(wl.assets[0].symbol, "AAPL");
    }

    #[tokio::test]
    async fn remove_from_watchlist_returns_updated_watchlist() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/watchlists/wl-1/AAPL"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "wl-1", "name": "My List", "assets": []
            })))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(live_config(server.uri()));
        let wl = client.remove_from_watchlist("wl-1", "AAPL").await.unwrap();
        assert!(wl.assets.is_empty());
    }

    #[tokio::test]
    async fn get_portfolio_history_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/account/portfolio/history"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "equity": [100000.0, 100100.0, null],
                "timestamp": [1000, 2000, 3000],
                "base_value": 100000.0
            })))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(paper_config(server.uri()));
        let history = client.get_portfolio_history("1D", "1Min").await.unwrap();
        assert_eq!(history.equity.len(), 3);
        assert_eq!(history.equity[0], Some(100000.0));
        assert_eq!(history.equity[2], None);
    }

    #[tokio::test]
    async fn get_snapshots_with_symbols_calls_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/stocks/snapshots"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(paper_config(server.uri()));
        let result = client.get_snapshots(&["AAPL".to_string()]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn get_intraday_bars_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/stocks/AAPL/bars"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "bars": [{"c": 195.5}],
                "symbol": "AAPL",
                "next_page_token": null
            })))
            .mount(&server)
            .await;

        let client = AlpacaClient::new(paper_config(server.uri()));
        let bars = client.get_intraday_bars("AAPL").await.unwrap();
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].c, 195.5);
    }
}
