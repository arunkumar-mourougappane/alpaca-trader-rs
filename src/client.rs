//! Async HTTP client wrapping the Alpaca Markets REST API.
use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue};

use crate::config::AlpacaConfig;
use crate::types::{
    AccountInfo, MarketClock, Order, OrderRequest, PortfolioHistory, Position, Watchlist,
    WatchlistSummary,
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

    /// Fetch intraday portfolio equity history (`GET /account/portfolio/history`).
    ///
    /// Requests 1-minute bars for the current trading day. Equity values are
    /// `None` for buckets when the market was closed.
    pub async fn get_portfolio_history(&self) -> Result<PortfolioHistory> {
        self.http
            .get(self.url("/account/portfolio/history"))
            .query(&[("timeframe", "1Min"), ("period", "1D")])
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("GET /account/portfolio/history request failed")?
            .json::<PortfolioHistory>()
            .await
            .context("GET /account/portfolio/history parse failed")
    }
}
