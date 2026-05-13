//! Domain types used throughout the library and the binary crate.
use chrono::DateTime;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

    // ── Enum as_str ──────────────────────────────────────────────────────────

    #[test]
    fn order_side_buy_str() {
        assert_eq!(OrderSide::Buy.as_str(), "buy");
    }

    #[test]
    fn order_side_sell_str() {
        assert_eq!(OrderSide::Sell.as_str(), "sell");
    }

    #[test]
    fn order_type_market_str() {
        assert_eq!(OrderType::Market.as_str(), "market");
    }

    #[test]
    fn order_type_limit_str() {
        assert_eq!(OrderType::Limit.as_str(), "limit");
    }

    #[test]
    fn time_in_force_day_str() {
        assert_eq!(TimeInForce::Day.as_str(), "day");
    }

    #[test]
    fn time_in_force_gtc_str() {
        assert_eq!(TimeInForce::Gtc.as_str(), "gtc");
    }

    // ── Serde deserialization ─────────────────────────────────────────────────

    #[test]
    fn account_info_deserializes() {
        let json = r#"{
            "status": "ACTIVE",
            "equity": "100000",
            "buying_power": "200000",
            "cash": "100000",
            "long_market_value": "0",
            "daytrade_count": 0,
            "pattern_day_trader": false,
            "currency": "USD"
        }"#;
        let acc: AccountInfo = serde_json::from_str(json).unwrap();
        assert_eq!(acc.status, "ACTIVE");
        assert_eq!(acc.equity, "100000");
        assert_eq!(acc.buying_power, "200000");
        assert_eq!(acc.cash, "100000");
        assert_eq!(acc.daytrade_count, 0);
        assert!(!acc.pattern_day_trader);
        assert_eq!(acc.currency, "USD");
        assert!(acc.portfolio_value.is_none());
    }

    #[test]
    fn order_notional_qty_null() {
        let json = r#"{
            "id": "abc",
            "symbol": "AAPL",
            "side": "buy",
            "qty": null,
            "notional": "500",
            "order_type": "market",
            "status": "accepted",
            "filled_qty": "0",
            "time_in_force": "day"
        }"#;
        let order: Order = serde_json::from_str(json).unwrap();
        assert!(order.qty.is_none());
        assert_eq!(order.notional.as_deref(), Some("500"));
        assert!(order.limit_price.is_none());
    }

    #[test]
    fn watchlist_empty_assets_default() {
        let json = r#"{"id": "wl1", "name": "Test"}"#;
        let wl: Watchlist = serde_json::from_str(json).unwrap();
        assert!(wl.assets.is_empty());
    }

    // ── OrderRequest serde rename ─────────────────────────────────────────────

    #[test]
    fn order_request_serializes_type_field() {
        let req = OrderRequest {
            symbol: "AAPL".into(),
            qty: Some("10".into()),
            notional: None,
            side: "buy".into(),
            order_type: "limit".into(),
            time_in_force: "day".into(),
            limit_price: Some("185.00".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(
            json.contains("\"type\""),
            "body should use 'type' key: {json}"
        );
        assert!(
            !json.contains("\"order_type\""),
            "body must not use 'order_type': {json}"
        );
    }

    #[test]
    fn order_request_omits_none_fields() {
        let req = OrderRequest {
            symbol: "TSLA".into(),
            qty: None,
            notional: Some("1000".into()),
            side: "buy".into(),
            order_type: "market".into(),
            time_in_force: "day".into(),
            limit_price: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("\"qty\""), "qty should be omitted: {json}");
        assert!(
            !json.contains("\"limit_price\""),
            "limit_price should be omitted: {json}"
        );
        assert!(
            json.contains("\"notional\""),
            "notional should be present: {json}"
        );
    }

    // ── MarketClock::market_state ─────────────────────────────────────────────

    fn make_clock(
        is_open: bool,
        timestamp: &str,
        next_open: &str,
        next_close: &str,
    ) -> MarketClock {
        MarketClock {
            is_open,
            timestamp: timestamp.into(),
            next_open: next_open.into(),
            next_close: next_close.into(),
        }
    }

    #[test]
    fn market_state_open() {
        // is_open=true always returns Open regardless of timestamps
        let clock = make_clock(
            true,
            "2026-05-12T15:00:00Z",
            "2026-05-13T13:30:00Z",
            "2026-05-12T20:00:00Z",
        );
        assert_eq!(clock.market_state(), MarketState::Open);
    }

    #[test]
    fn market_state_pre_market() {
        // 2 hours before next open → PRE-MARKET
        let next_open = "2026-05-13T13:30:00Z"; // 9:30 AM ET
        let now = "2026-05-13T11:30:00Z"; // 7:30 AM ET, 2h before open
        let clock = make_clock(false, now, next_open, "2026-05-13T20:00:00Z");
        assert_eq!(clock.market_state(), MarketState::PreMarket);
    }

    #[test]
    fn market_state_pre_market_boundary() {
        // Exactly 4 hours before next open → still PRE-MARKET
        let next_open = "2026-05-13T13:30:00Z";
        let now = "2026-05-13T09:30:00Z"; // exactly 4h before
        let clock = make_clock(false, now, next_open, "2026-05-13T20:00:00Z");
        assert_eq!(clock.market_state(), MarketState::PreMarket);
    }

    #[test]
    fn market_state_after_hours() {
        // 5 hours after close (4 PM → 9 PM), ~16.5h before next open
        let next_open = "2026-05-13T13:30:00Z"; // next morning 9:30 AM ET
        let now = "2026-05-12T21:00:00Z"; // 5 PM ET (17:00 UTC−4 = 21:00 UTC)
        let clock = make_clock(false, now, next_open, "2026-05-13T20:00:00Z");
        assert_eq!(clock.market_state(), MarketState::AfterHours);
    }

    #[test]
    fn market_state_closed_weekend() {
        // Friday close to Monday open: ~65.5 hours → CLOSED
        let next_open = "2026-05-18T13:30:00Z"; // Monday 9:30 AM ET
        let now = "2026-05-16T00:00:00Z"; // Saturday midnight UTC
        let clock = make_clock(false, now, next_open, "2026-05-18T20:00:00Z");
        assert_eq!(clock.market_state(), MarketState::Closed);
    }

    #[test]
    fn market_state_closed_overnight() {
        // ~22h before open (e.g. 11 PM ET before next 9:30 AM) → CLOSED
        let next_open = "2026-05-13T13:30:00Z";
        let now = "2026-05-12T15:30:00Z"; // ~22h before
        let clock = make_clock(false, now, next_open, "2026-05-13T20:00:00Z");
        assert_eq!(clock.market_state(), MarketState::Closed);
    }

    #[test]
    fn market_state_invalid_timestamp_falls_back_to_closed() {
        let clock = make_clock(false, "not-a-date", "also-bad", "2026-05-13T20:00:00Z");
        assert_eq!(clock.market_state(), MarketState::Closed);
    }

    #[test]
    fn market_state_as_str_values() {
        assert_eq!(MarketState::Open.as_str(), "OPEN");
        assert_eq!(MarketState::PreMarket.as_str(), "PRE-MARKET");
        assert_eq!(MarketState::AfterHours.as_str(), "AFTER-HOURS");
        assert_eq!(MarketState::Closed.as_str(), "CLOSED");
    }
}

/// Snapshot of account balances and status from `GET /account`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AccountInfo {
    /// Account status (e.g., `"ACTIVE"`).
    pub status: String,
    /// Total account equity as a dollar string.
    pub equity: String,
    /// Buying power available for new orders, as a dollar string.
    pub buying_power: String,
    /// Cash balance, as a dollar string.
    pub cash: String,
    /// Total long market value of all positions, as a dollar string.
    pub long_market_value: String,
    /// Number of day trades made in the rolling 5-business-day window.
    pub daytrade_count: u32,
    /// Whether the account has been flagged as a pattern day trader.
    pub pattern_day_trader: bool,
    /// Account currency (typically `"USD"`).
    pub currency: String,
    /// Total portfolio value; may be absent on some account types.
    #[serde(default)]
    pub portfolio_value: Option<String>,
}

/// A single open or closed position held in the account.
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    /// Ticker symbol (e.g., `"AAPL"`).
    pub symbol: String,
    /// Number of shares held (positive = long, may be fractional).
    pub qty: String,
    /// Average cost basis per share, as a dollar string.
    pub avg_entry_price: String,
    /// Current price per share, as a dollar string.
    pub current_price: String,
    /// Current market value of the entire position, as a dollar string.
    pub market_value: String,
    /// Total unrealised profit/loss, as a dollar string.
    pub unrealized_pl: String,
    /// Total unrealised profit/loss percentage, as a decimal string.
    pub unrealized_plpc: String,
    /// Position side: `"long"` or `"short"`.
    pub side: String,
    /// Asset class (e.g., `"us_equity"`, `"crypto"`).
    #[serde(default)]
    pub asset_class: String,
}

/// An order placed with the broker (open, filled, cancelled, etc.).
#[derive(Debug, Clone, Deserialize)]
pub struct Order {
    /// Unique order ID assigned by Alpaca.
    pub id: String,
    /// Ticker symbol the order is for.
    pub symbol: String,
    /// Order direction: `"buy"` or `"sell"`.
    pub side: String,
    /// Whole-share quantity. Mutually exclusive with `notional`.
    #[serde(default)]
    pub qty: Option<String>,
    /// Dollar notional amount. Mutually exclusive with `qty`.
    #[serde(default)]
    pub notional: Option<String>,
    /// Order type: `"market"`, `"limit"`, etc.
    pub order_type: String,
    /// Limit price for limit orders; absent for market orders.
    #[serde(default)]
    pub limit_price: Option<String>,
    /// Current order status (e.g., `"new"`, `"filled"`, `"canceled"`).
    pub status: String,
    /// ISO 8601 timestamp of when the order was submitted.
    #[serde(default)]
    pub submitted_at: Option<String>,
    /// ISO 8601 timestamp of when the order was fully filled.
    #[serde(default)]
    pub filled_at: Option<String>,
    /// Number of shares filled so far.
    pub filled_qty: String,
    /// Time-in-force: `"day"`, `"gtc"`, etc.
    pub time_in_force: String,
}

/// Direction of an order.
#[derive(Debug, Clone, PartialEq)]
pub enum OrderSide {
    /// Buy (long) order.
    Buy,
    /// Sell (short or close-long) order.
    Sell,
}

impl OrderSide {
    /// Returns the lowercase API string for this side: `"buy"` or `"sell"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        }
    }
}

/// Execution type of an order.
#[derive(Debug, Clone, PartialEq)]
pub enum OrderType {
    /// Execute immediately at the best available price.
    Market,
    /// Execute only at the specified limit price or better.
    Limit,
}

impl OrderType {
    /// Returns the lowercase API string: `"market"` or `"limit"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
        }
    }
}

/// How long an order remains active before it is automatically cancelled.
#[derive(Debug, Clone)]
pub enum TimeInForce {
    /// Active only for the current trading day.
    Day,
    /// Active until explicitly cancelled (Good Till Cancelled).
    Gtc,
}

impl TimeInForce {
    /// Returns the lowercase API string: `"day"` or `"gtc"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            TimeInForce::Day => "day",
            TimeInForce::Gtc => "gtc",
        }
    }
}

/// Request body sent to `POST /orders`.
///
/// Either `qty` or `notional` must be set; not both.
#[derive(Debug, Clone, Serialize)]
pub struct OrderRequest {
    /// Ticker symbol to trade.
    pub symbol: String,
    /// Whole-share quantity; omitted when using notional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qty: Option<String>,
    /// Dollar notional; omitted when using share quantity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notional: Option<String>,
    /// Order direction — `"buy"` or `"sell"`.
    pub side: String,
    /// Order type — `"market"`, `"limit"`, etc.
    #[serde(rename = "type")]
    pub order_type: String,
    /// Time-in-force — `"day"`, `"gtc"`, etc.
    pub time_in_force: String,
    /// Required for limit orders; omitted for market orders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<String>,
}

/// Current market clock from `GET /clock`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MarketClock {
    /// `true` if the primary US equity market is currently open.
    pub is_open: bool,
    /// ISO 8601 timestamp of the next market open.
    pub next_open: String,
    /// ISO 8601 timestamp of the next market close.
    pub next_close: String,
    /// Current server timestamp in ISO 8601 format.
    pub timestamp: String,
}

/// The current trading session state derived from [`MarketClock`] fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketState {
    /// Primary session — 9:30 AM – 4:00 PM ET.
    Open,
    /// Pre-market session — up to 4 hours before the next open.
    PreMarket,
    /// After-hours session — between close and ~20 hours before next open.
    AfterHours,
    /// Market is closed (overnight / weekend).
    Closed,
}

impl MarketState {
    /// Display string shown in the header.
    pub fn as_str(&self) -> &'static str {
        match self {
            MarketState::Open => "OPEN",
            MarketState::PreMarket => "PRE-MARKET",
            MarketState::AfterHours => "AFTER-HOURS",
            MarketState::Closed => "CLOSED",
        }
    }
}

impl MarketClock {
    /// Derive the current [`MarketState`] from the clock's fields.
    ///
    /// Decision tree:
    /// 1. `is_open == true` → [`MarketState::Open`]
    /// 2. Parse `timestamp` (current ET time) and `next_open` (next session open).
    /// 3. `time_to_open ≤ 4 h` → [`MarketState::PreMarket`]  (pre-market starts ~4 AM)
    /// 4. `4 h < time_to_open ≤ 20 h` → [`MarketState::AfterHours`]  (same trading day, ~4–8 PM ET)
    /// 5. Otherwise → [`MarketState::Closed`]  (overnight / weekend gap > 20 h)
    ///
    /// Falls back to [`MarketState::Closed`] when the timestamp strings cannot be parsed.
    pub fn market_state(&self) -> MarketState {
        if self.is_open {
            return MarketState::Open;
        }
        let now = DateTime::parse_from_rfc3339(&self.timestamp).ok();
        let next_open = DateTime::parse_from_rfc3339(&self.next_open).ok();
        match (now, next_open) {
            (Some(now), Some(open)) => {
                let secs = (open - now).num_seconds();
                if secs <= 0 {
                    // next_open is in the past — shouldn't happen normally
                    MarketState::Closed
                } else if secs <= 4 * 3600 {
                    MarketState::PreMarket
                } else if secs <= 20 * 3600 {
                    MarketState::AfterHours
                } else {
                    MarketState::Closed
                }
            }
            _ => MarketState::Closed,
        }
    }
}

/// Latest NBBO quote for a symbol from the market data stream.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Quote {
    /// Ticker symbol this quote is for.
    pub symbol: String,
    /// Ask price (best offer), absent before the first quote arrives.
    #[serde(default)]
    pub ap: Option<f64>,
    /// Bid price (best bid), absent before the first quote arrives.
    #[serde(default)]
    pub bp: Option<f64>,
    /// Ask size in round lots.
    #[serde(default)]
    pub as_: Option<u64>,
    /// Bid size in round lots.
    #[serde(default)]
    pub bs: Option<u64>,
}

/// Brief watchlist descriptor returned by `GET /watchlists`.
///
/// For the full asset list use [`Watchlist`] via [`AlpacaClient::get_watchlist`].
///
/// [`AlpacaClient::get_watchlist`]: crate::client::AlpacaClient::get_watchlist
#[derive(Debug, Clone, Deserialize)]
pub struct WatchlistSummary {
    /// Unique watchlist identifier (UUID).
    pub id: String,
    /// Human-readable watchlist name.
    pub name: String,
}

/// Full watchlist including its constituent assets.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Watchlist {
    /// Unique watchlist identifier (UUID).
    pub id: String,
    /// Human-readable watchlist name.
    pub name: String,
    /// Ordered list of assets in this watchlist.
    #[serde(default)]
    pub assets: Vec<Asset>,
}

/// An individual asset returned inside a [`Watchlist`] or from `GET /assets`.
#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    /// Unique asset identifier (UUID).
    pub id: String,
    /// Ticker symbol (e.g., `"AAPL"`).
    pub symbol: String,
    /// Full company or asset name.
    pub name: String,
    /// Exchange on which the asset trades (e.g., `"NASDAQ"`).
    pub exchange: String,
    /// Asset class (e.g., `"us_equity"`, `"crypto"`).
    #[serde(rename = "class")]
    pub asset_class: String,
    /// Whether the asset is currently tradable via the API.
    pub tradable: bool,
    /// Whether the asset can be sold short.
    pub shortable: bool,
    /// Whether fractional-share quantities are supported.
    pub fractionable: bool,
    /// Whether the asset is easy to borrow for shorting.
    #[serde(default)]
    pub easy_to_borrow: bool,
}

/// Response from `GET /v2/account/portfolio/history`.
///
/// The `equity` array contains one value per time bucket; entries are `null`
/// when the market was closed during that interval.
#[derive(Debug, Clone, Deserialize)]
pub struct PortfolioHistory {
    /// Dollar equity values per time bucket; `None` means market was closed.
    pub equity: Vec<Option<f64>>,
}
