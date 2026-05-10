/// Commands sent from `update()` (sync) to the async command handler task.
/// This is the bridge across the sync/async boundary for all mutation operations.
#[derive(Debug)]
pub enum Command {
    SubmitOrder {
        symbol: String,
        side: String,       // "buy" | "sell"
        order_type: String, // "market" | "limit"
        qty: Option<String>,
        price: Option<String>,
        time_in_force: String,
    },
    CancelOrder(String), // order id
    AddToWatchlist {
        watchlist_id: String,
        symbol: String,
    },
    RemoveFromWatchlist {
        watchlist_id: String,
        symbol: String,
    },
}
