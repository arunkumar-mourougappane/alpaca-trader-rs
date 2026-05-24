//! Application event types that flow through the central event channel.
use std::collections::HashMap;

use crate::types::{AccountInfo, MarketClock, Order, Position, Quote, Snapshot, Watchlist};
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
    ///
    /// `event_type` is the Alpaca event field (e.g. `"fill"`, `"partial_fill"`,
    /// `"canceled"`, `"rejected"`).  The UI uses it to render status bar
    /// notifications with appropriate icons and messages.
    TradeUpdate {
        /// The updated order state.
        order: Order,
        /// Alpaca event type string (e.g. `"fill"`, `"partial_fill"`, `"rejected"`).
        event_type: String,
    },
    /// A WebSocket stream successfully (re)connected.
    StreamConnected(StreamKind),
    /// A WebSocket stream disconnected and is waiting to retry.
    ///
    /// `attempt` is the 1-based reconnect attempt number. The UI uses this to
    /// display a "reconnecting… (N)" indicator while back-off is in progress.
    StreamReconnecting {
        /// Which stream is reconnecting.
        kind: StreamKind,
        /// 1-based attempt number (1 = first retry after the initial disconnect).
        attempt: u32,
    },
    /// A WebSocket stream disconnected permanently (max reconnect attempts reached).
    StreamDisconnected(StreamKind),
    /// Portfolio equity history loaded at startup for the sparkline.
    PortfolioHistoryLoaded(Vec<f64>),
    /// Latest market snapshots (daily bars + prev close) for watchlist symbols.
    SnapshotsUpdated(HashMap<String, Snapshot>),
    /// Intraday 1-minute bar close prices (as cents) for a single symbol.
    ///
    /// Used to render the intraday sparkline inside the symbol-detail modal.
    IntradayBarsReceived {
        /// Ticker symbol these bars belong to.
        symbol: String,
        /// Close prices in integer cents, oldest first.
        bars: Vec<u64>,
    },
    /// Watchlists are not available in paper trading mode.
    ///
    /// Emitted once when the REST handler detects `client.is_paper()` so the
    /// UI can display a clear, persistent explanation instead of "Loading…".
    WatchlistUnavailable,
    /// A REST fetch has been dispatched (one per in-flight request).
    ///
    /// Increments `App::pending_requests` so the UI can show a spinner.
    FetchStarted,
    /// A REST fetch has completed (success or error).
    ///
    /// Decrements `App::pending_requests`; when it reaches zero,
    /// `App::last_updated` is recorded with the current local time.
    FetchComplete,
    /// Periodic tick to trigger UI refresh / REST polls.
    Tick,
    /// One-shot status message to display in the status bar.
    StatusMsg(String),
    /// Request to quit the application.
    Quit,
}
