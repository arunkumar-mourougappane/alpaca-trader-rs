//! Commands sent across the sync/async boundary from the UI to the async handler task.
/// Commands sent from `update()` (sync) to the async command-handler task.
///
/// This enum bridges the sync/async boundary for all mutation operations
/// initiated from key-press handlers in the UI layer.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Command {
    /// Submit a new order to the broker.
    SubmitOrder {
        /// Ticker symbol to trade (e.g., `"AAPL"`).
        symbol: String,
        /// Order direction: `"buy"` or `"sell"`.
        side: String,
        /// Execution type: `"market"`, `"limit"`, `"stop"`, `"stop_limit"`, `"trailing_stop"`.
        order_type: String,
        /// Whole-share quantity; `None` when using notional.
        qty: Option<String>,
        /// Limit price for limit and stop-limit orders; `None` otherwise.
        limit_price: Option<String>,
        /// Stop trigger price for stop and stop-limit orders; `None` otherwise.
        stop_price: Option<String>,
        /// Dollar trail amount for trailing-stop orders; `None` otherwise.
        trail_price: Option<String>,
        /// Percentage trail for trailing-stop orders; `None` otherwise.
        trail_percent: Option<String>,
        /// Time-in-force: `"day"` or `"gtc"`.
        time_in_force: String,
        /// Allow execution during extended hours; only valid for limit/day orders.
        extended_hours: bool,
        /// Take-profit limit price for bracket orders; `None` for simple orders.
        take_profit_price: Option<String>,
        /// Stop-loss stop price for bracket orders; `None` for simple orders.
        stop_loss_price: Option<String>,
        /// Stop-loss limit price for bracket orders (makes it a stop-limit leg); `None` for market SL.
        stop_loss_limit_price: Option<String>,
    },
    /// Cancel an open order identified by its Alpaca order ID.
    CancelOrder(String),
    /// Add a symbol to a watchlist.
    AddToWatchlist {
        /// UUID of the target watchlist.
        watchlist_id: String,
        /// Ticker symbol to add.
        symbol: String,
    },
    /// Remove a symbol from a watchlist.
    RemoveFromWatchlist {
        /// UUID of the target watchlist.
        watchlist_id: String,
        /// Ticker symbol to remove.
        symbol: String,
    },
    /// Fetch intraday (1-minute) bars for the given ticker to populate the sparkline.
    FetchIntradayBars(String),
    /// Fetch portfolio equity history for the given Alpaca API parameters.
    FetchPortfolioHistory {
        /// Alpaca `period` query param (e.g., `"1D"`, `"1W"`, `"1M"`, `"YTD"`).
        period: String,
        /// Alpaca `timeframe` query param (e.g., `"1Min"`, `"1H"`, `"1D"`).
        timeframe: String,
    },
}
