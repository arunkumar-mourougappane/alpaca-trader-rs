//! Commands sent across the sync/async boundary from the UI to the async handler task.
/// Commands sent from `update()` (sync) to the async command-handler task.
///
/// This enum bridges the sync/async boundary for all mutation operations
/// initiated from key-press handlers in the UI layer.
#[derive(Debug)]
pub enum Command {
    /// Submit a new order to the broker.
    SubmitOrder {
        /// Ticker symbol to trade (e.g., `"AAPL"`).
        symbol: String,
        /// Order direction: `"buy"` or `"sell"`.
        side: String,
        /// Execution type: `"market"` or `"limit"`.
        order_type: String,
        /// Whole-share quantity; `None` when using notional.
        qty: Option<String>,
        /// Limit price for limit orders; `None` for market orders.
        price: Option<String>,
        /// Time-in-force: `"day"` or `"gtc"`.
        time_in_force: String,
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
}
