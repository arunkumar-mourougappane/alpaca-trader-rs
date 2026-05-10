//! Application event types that flow through the central event channel.
use crate::types::{AccountInfo, MarketClock, Order, Position, Quote, Watchlist};
use crossterm::event::{KeyEvent, MouseEvent};

/// Identifies which WebSocket stream a connection-status event concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    /// Real-time market data stream (quotes, trades).
    Market,
    /// Account/order update stream (trade confirmations, account changes).
    Account,
}

/// All events that flow through the application's event channel.
///
/// The event loop receives these from background tasks (REST pollers,
/// WebSocket streams, terminal input) and forwards them to `update()`.
#[derive(Debug)]
pub enum Event {
    /// A keyboard key was pressed.
    Input(KeyEvent),
    /// A mouse event occurred (click, scroll, etc.).
    Mouse(MouseEvent),
    /// The terminal was resized to the given `(columns, rows)`.
    Resize(u16, u16),
    /// Latest account snapshot from the REST poll.
    AccountUpdated(AccountInfo),
    /// Current open positions from the REST poll.
    PositionsUpdated(Vec<Position>),
    /// Current orders from the REST poll.
    OrdersUpdated(Vec<Order>),
    /// Market clock state from the REST poll.
    ClockUpdated(MarketClock),
    /// Updated watchlist (after an add/remove command or periodic poll).
    WatchlistUpdated(Watchlist),
    /// Latest NBBO quote received from the market data WebSocket.
    MarketQuote(Quote),
    /// Order update received from the account WebSocket stream.
    TradeUpdate(Order),
    /// A WebSocket stream successfully (re)connected.
    StreamConnected(StreamKind),
    /// A WebSocket stream disconnected; reconnection will be attempted.
    StreamDisconnected(StreamKind),
    /// Portfolio equity history loaded at startup for the sparkline.
    PortfolioHistoryLoaded(Vec<f64>),
    /// Periodic tick to trigger UI refresh / REST polls.
    Tick,
    /// One-shot status message to display in the status bar.
    StatusMsg(String),
    /// Request to quit the application.
    Quit,
}
