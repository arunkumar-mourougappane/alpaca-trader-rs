use crate::types::{AccountInfo, MarketClock, Order, Position, Quote, Watchlist};
use crossterm::event::{KeyEvent, MouseEvent};

#[derive(Debug)]
pub enum Event {
    Input(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    // REST poll results
    AccountUpdated(AccountInfo),
    PositionsUpdated(Vec<Position>),
    OrdersUpdated(Vec<Order>),
    ClockUpdated(MarketClock),
    WatchlistUpdated(Watchlist),
    // WebSocket (Phase 2)
    MarketQuote(Quote),
    TradeUpdate(Order),
    // Control
    Tick,
    StatusMsg(String),
    Quit,
}
