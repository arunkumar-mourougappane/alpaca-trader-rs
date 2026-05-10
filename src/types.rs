use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AccountInfo {
    pub status: String,
    pub equity: String,
    pub buying_power: String,
    pub cash: String,
    pub long_market_value: String,
    pub daytrade_count: u32,
    pub pattern_day_trader: bool,
    pub currency: String,
    #[serde(default)]
    pub portfolio_value: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub qty: String,
    pub avg_entry_price: String,
    pub current_price: String,
    pub market_value: String,
    pub unrealized_pl: String,
    pub unrealized_plpc: String,
    pub side: String,
    #[serde(default)]
    pub asset_class: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Order {
    pub id: String,
    pub symbol: String,
    pub side: String,
    #[serde(default)]
    pub qty: Option<String>,
    #[serde(default)]
    pub notional: Option<String>,
    pub order_type: String,
    #[serde(default)]
    pub limit_price: Option<String>,
    pub status: String,
    #[serde(default)]
    pub submitted_at: Option<String>,
    #[serde(default)]
    pub filled_at: Option<String>,
    pub filled_qty: String,
    pub time_in_force: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderType {
    Market,
    Limit,
}

impl OrderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
        }
    }
}

#[derive(Debug, Clone)]
pub enum TimeInForce {
    Day,
    Gtc,
}

impl TimeInForce {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimeInForce::Day => "day",
            TimeInForce::Gtc => "gtc",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderRequest {
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qty: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notional: Option<String>,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub time_in_force: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MarketClock {
    pub is_open: bool,
    pub next_open: String,
    pub next_close: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Quote {
    pub symbol: String,
    #[serde(default)]
    pub ap: Option<f64>,
    #[serde(default)]
    pub bp: Option<f64>,
    #[serde(default)]
    pub as_: Option<u64>,
    #[serde(default)]
    pub bs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WatchlistSummary {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Watchlist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub exchange: String,
    #[serde(rename = "class")]
    pub asset_class: String,
    pub tradable: bool,
    pub shortable: bool,
    pub fractionable: bool,
    #[serde(default)]
    pub easy_to_borrow: bool,
}
