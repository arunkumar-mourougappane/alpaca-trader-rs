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
        assert!(json.contains("\"type\""), "body should use 'type' key: {json}");
        assert!(!json.contains("\"order_type\""), "body must not use 'order_type': {json}");
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
        assert!(!json.contains("\"limit_price\""), "limit_price should be omitted: {json}");
        assert!(json.contains("\"notional\""), "notional should be present: {json}");
    }
}

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
