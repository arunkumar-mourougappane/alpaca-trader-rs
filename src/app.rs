use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::layout::Rect;

const STATUS_MSG_TTL: Duration = Duration::from_secs(3);

/// A status bar message that may auto-expire.
///
/// Transient messages (e.g. "Order submitted", "Refreshing…") carry a 3-second TTL and are
/// cleared automatically on the next `Tick` after they expire. Persistent messages (errors,
/// "Loading…") set `expires_at = None` so they stay until replaced.
#[derive(Clone, Debug)]
pub struct StatusMessage {
    pub text: String,
    pub expires_at: Option<Instant>,
}

impl StatusMessage {
    /// Creates a transient message that auto-dismisses after 3 seconds.
    pub fn transient(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            expires_at: Some(Instant::now() + STATUS_MSG_TTL),
        }
    }

    /// Creates a persistent message that stays until explicitly replaced.
    pub fn persistent(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            expires_at: None,
        }
    }

    /// Returns `true` if the message text is empty (nothing to display).
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Clears the message text and removes any expiry.
    pub fn clear(&mut self) {
        self.text.clear();
        self.expires_at = None;
    }
}

impl Default for StatusMessage {
    fn default() -> Self {
        Self::persistent("")
    }
}

/// Allow `PartialEq` comparisons against string literals in tests.
impl PartialEq<str> for StatusMessage {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

impl PartialEq<&str> for StatusMessage {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

impl PartialEq<String> for StatusMessage {
    fn eq(&self, other: &String) -> bool {
        self.text == *other
    }
}

impl PartialEq<StatusMessage> for StatusMessage {
    fn eq(&self, other: &StatusMessage) -> bool {
        self.text == other.text
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use crate::config::{AlpacaConfig, AlpacaEnv};
    use crate::types::{Asset, Order, Watchlist};

    pub fn make_test_app() -> App {
        let (command_tx, _) = tokio::sync::mpsc::channel(1);
        let (symbol_tx, _) = tokio::sync::watch::channel(vec![]);
        App::new(
            AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: AlpacaEnv::Paper,
            },
            Arc::new(tokio::sync::Notify::new()),
            command_tx,
            symbol_tx,
        )
    }

    pub fn make_order(id: &str, status: &str) -> Order {
        Order {
            id: id.into(),
            symbol: "AAPL".into(),
            side: "buy".into(),
            qty: Some("10".into()),
            notional: None,
            order_type: "limit".into(),
            limit_price: None,
            status: status.into(),
            submitted_at: None,
            filled_at: None,
            filled_qty: "0".into(),
            time_in_force: "day".into(),
        }
    }

    pub fn make_asset(symbol: &str) -> Asset {
        Asset {
            id: format!("id-{symbol}"),
            symbol: symbol.into(),
            name: format!("{symbol} Corp"),
            exchange: "NASDAQ".into(),
            asset_class: "us_equity".into(),
            tradable: true,
            shortable: true,
            fractionable: true,
            easy_to_borrow: true,
        }
    }

    pub fn make_watchlist(symbols: &[&str]) -> Watchlist {
        Watchlist {
            id: "11111111-1111-1111-1111-111111111111".into(),
            name: "Test".into(),
            assets: symbols.iter().map(|s| make_asset(s)).collect(),
        }
    }
}

use ratatui::widgets::TableState;
use tokio::sync::{mpsc, watch, Notify};

use crate::commands::Command;
use crate::config::AlpacaConfig;
use crate::types::{AccountInfo, MarketClock, Order, Position, Quote, Snapshot, Watchlist};

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Account,
    Watchlist,
    Positions,
    Orders,
}

impl Tab {
    pub fn index(&self) -> usize {
        match self {
            Tab::Account => 0,
            Tab::Watchlist => 1,
            Tab::Positions => 2,
            Tab::Orders => 3,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Tab::Account,
            1 => Tab::Watchlist,
            2 => Tab::Positions,
            _ => Tab::Orders,
        }
    }

    pub fn next(&self) -> Self {
        Tab::from_index((self.index() + 1) % 4)
    }

    pub fn prev(&self) -> Self {
        Tab::from_index((self.index() + 3) % 4)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrdersSubTab {
    Open,
    Filled,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderField {
    Symbol,
    Side,
    OrderType,
    Qty,
    Price,
    TimeInForce,
    Submit,
}

impl OrderField {
    pub fn next(&self) -> Self {
        match self {
            OrderField::Symbol => OrderField::Side,
            OrderField::Side => OrderField::OrderType,
            OrderField::OrderType => OrderField::Qty,
            OrderField::Qty => OrderField::Price,
            OrderField::Price => OrderField::TimeInForce,
            OrderField::TimeInForce => OrderField::Submit,
            OrderField::Submit => OrderField::Symbol,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            OrderField::Symbol => OrderField::Submit,
            OrderField::Side => OrderField::Symbol,
            OrderField::OrderType => OrderField::Side,
            OrderField::Qty => OrderField::OrderType,
            OrderField::Price => OrderField::Qty,
            OrderField::TimeInForce => OrderField::Price,
            OrderField::Submit => OrderField::TimeInForce,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderEntryState {
    pub symbol: String,
    pub side_buy: bool,     // true = BUY, false = SELL
    pub market_order: bool, // true = MARKET, false = LIMIT
    pub gtc_order: bool,    // true = GTC, false = DAY
    pub qty_input: String,
    pub price_input: String,
    pub focused_field: OrderField,
}

impl OrderEntryState {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            side_buy: true,
            market_order: false,
            gtc_order: false,
            qty_input: String::new(),
            price_input: String::new(),
            focused_field: OrderField::Qty,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    CancelOrder(String),
    RemoveFromWatchlist {
        #[allow(dead_code)]
        watchlist_id: String,
        symbol: String,
    },
}

#[derive(Debug, Clone)]
pub enum Modal {
    Help,
    OrderEntry(OrderEntryState),
    SymbolDetail(String),
    Confirm {
        message: String,
        action: ConfirmAction,
        confirmed: bool,
    },
    AddSymbol {
        input: String,
        watchlist_id: String,
    },
}

/// Screen areas of interactive elements, populated by the renderer each frame.
/// Used by the mouse event handler to map click coordinates to actions.
#[derive(Default, Clone, Debug)]
pub struct HitAreas {
    /// The tab bar row. Each tab's exact rect is computed from label widths at hit-test time.
    pub tab_bar: Rect,
    /// Y coordinate of the first data row in the active list panel.
    /// Accounts for block border + header rows. `0` means no active list.
    pub list_data_start_y: u16,
    /// Orders panel sub-tab rects, one per sub-tab (Open / Filled / Cancelled), computed from
    /// the actual rendered label widths during each frame so dynamic counts are accounted for.
    pub orders_subtab_rects: Vec<Rect>,
    /// OrderEntry modal: clickable field rows keyed by [`OrderField`].
    pub modal_fields: Vec<(OrderField, Rect)>,
    /// OrderEntry modal: submit button row.
    pub modal_submit: Option<Rect>,
    /// Confirm modal: button row (left half = Yes, right half = No).
    pub modal_confirm_buttons: Option<Rect>,
}

pub struct App {
    pub config: AlpacaConfig,
    pub refresh_notify: Arc<Notify>,
    pub command_tx: mpsc::Sender<Command>,
    pub symbol_tx: watch::Sender<Vec<String>>,

    pub account: Option<AccountInfo>,
    pub positions: Vec<Position>,
    pub orders: Vec<Order>,
    pub quotes: HashMap<String, Quote>,
    pub watchlist: Option<Watchlist>,
    pub snapshots: HashMap<String, Snapshot>,
    pub clock: Option<MarketClock>,
    pub equity_history: Vec<u64>,
    /// Intraday 1-minute close prices in cents, keyed by ticker symbol.
    pub intraday_bars: HashMap<String, Vec<u64>>,

    pub active_tab: Tab,
    pub watchlist_state: TableState,
    pub positions_state: TableState,
    pub orders_state: TableState,
    pub orders_subtab: OrdersSubTab,

    pub modal: Option<Modal>,
    pub search_query: String,
    pub searching: bool,

    pub status_msg: StatusMessage,
    pub should_quit: bool,

    /// `true` while the market-data WebSocket is connected and authenticated.
    pub market_stream_ok: bool,
    /// `true` while the account WebSocket is connected and authenticated.
    pub account_stream_ok: bool,

    /// Interactive element positions from the last rendered frame.
    pub hit_areas: HitAreas,
}

impl App {
    pub fn new(
        config: AlpacaConfig,
        refresh_notify: Arc<Notify>,
        command_tx: mpsc::Sender<Command>,
        symbol_tx: watch::Sender<Vec<String>>,
    ) -> Self {
        Self {
            config,
            refresh_notify,
            command_tx,
            symbol_tx,
            account: None,
            positions: Vec::new(),
            orders: Vec::new(),
            quotes: HashMap::new(),
            watchlist: None,
            snapshots: HashMap::new(),
            clock: None,
            equity_history: Vec::new(),
            intraday_bars: HashMap::new(),
            active_tab: Tab::Account,
            watchlist_state: TableState::default(),
            positions_state: TableState::default(),
            orders_state: TableState::default(),
            orders_subtab: OrdersSubTab::Open,
            modal: None,
            search_query: String::new(),
            searching: false,
            status_msg: StatusMessage::persistent("Loading…"),
            should_quit: false,
            market_stream_ok: false,
            account_stream_ok: false,
            hit_areas: HitAreas::default(),
        }
    }

    pub fn filtered_orders(&self) -> Vec<&Order> {
        self.orders
            .iter()
            .filter(|o| match self.orders_subtab {
                OrdersSubTab::Open => {
                    matches!(
                        o.status.as_str(),
                        "new" | "pending_new" | "accepted" | "held" | "partially_filled"
                    )
                }
                OrdersSubTab::Filled => o.status == "filled",
                OrdersSubTab::Cancelled => {
                    matches!(
                        o.status.as_str(),
                        "canceled" | "expired" | "rejected" | "replaced"
                    )
                }
            })
            .collect()
    }

    pub fn selected_watchlist_symbol(&self) -> Option<String> {
        let wl = self.watchlist.as_ref()?;
        let assets = if self.searching {
            wl.assets
                .iter()
                .filter(|a| {
                    a.symbol
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase())
                        || a.name
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase())
                })
                .collect::<Vec<_>>()
        } else {
            wl.assets.iter().collect()
        };
        let i = self.watchlist_state.selected()?;
        assets.get(i).map(|a| a.symbol.clone())
    }

    pub fn selected_position_symbol(&self) -> Option<String> {
        let i = self.positions_state.selected()?;
        self.positions.get(i).map(|p| p.symbol.clone())
    }

    pub fn selected_order_id(&self) -> Option<String> {
        let orders = self.filtered_orders();
        let i = self.orders_state.selected()?;
        orders.get(i).map(|o| o.id.clone())
    }

    pub fn push_equity(&mut self) {
        if let Some(account) = &self.account {
            if let Ok(v) = account.equity.parse::<f64>() {
                self.equity_history.push((v * 100.0) as u64);
                if self.equity_history.len() > 120 {
                    self.equity_history.remove(0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use super::*;
    use crate::types::AccountInfo;

    // ── Tab navigation ────────────────────────────────────────────────────────

    #[test]
    fn tab_next_wraps_full_cycle() {
        assert_eq!(Tab::Account.next(), Tab::Watchlist);
        assert_eq!(Tab::Watchlist.next(), Tab::Positions);
        assert_eq!(Tab::Positions.next(), Tab::Orders);
        assert_eq!(Tab::Orders.next(), Tab::Account);
    }

    #[test]
    fn tab_prev_wraps_full_cycle() {
        assert_eq!(Tab::Account.prev(), Tab::Orders);
        assert_eq!(Tab::Orders.prev(), Tab::Positions);
        assert_eq!(Tab::Positions.prev(), Tab::Watchlist);
        assert_eq!(Tab::Watchlist.prev(), Tab::Account);
    }

    #[test]
    fn tab_from_index_all_variants() {
        assert_eq!(Tab::from_index(0), Tab::Account);
        assert_eq!(Tab::from_index(1), Tab::Watchlist);
        assert_eq!(Tab::from_index(2), Tab::Positions);
        assert_eq!(Tab::from_index(3), Tab::Orders);
        assert_eq!(Tab::from_index(4), Tab::Orders); // out-of-range → Orders
    }

    #[test]
    fn tab_index_all_variants() {
        assert_eq!(Tab::Account.index(), 0);
        assert_eq!(Tab::Watchlist.index(), 1);
        assert_eq!(Tab::Positions.index(), 2);
        assert_eq!(Tab::Orders.index(), 3);
    }

    // ── OrderField navigation ─────────────────────────────────────────────────

    #[test]
    fn order_field_next_full_cycle() {
        assert_eq!(OrderField::Symbol.next(), OrderField::Side);
        assert_eq!(OrderField::Side.next(), OrderField::OrderType);
        assert_eq!(OrderField::OrderType.next(), OrderField::Qty);
        assert_eq!(OrderField::Qty.next(), OrderField::Price);
        assert_eq!(OrderField::Price.next(), OrderField::TimeInForce);
        assert_eq!(OrderField::TimeInForce.next(), OrderField::Submit);
        assert_eq!(OrderField::Submit.next(), OrderField::Symbol);
    }

    #[test]
    fn order_field_prev_full_cycle() {
        assert_eq!(OrderField::Symbol.prev(), OrderField::Submit);
        assert_eq!(OrderField::Submit.prev(), OrderField::TimeInForce);
        assert_eq!(OrderField::TimeInForce.prev(), OrderField::Price);
        assert_eq!(OrderField::Price.prev(), OrderField::Qty);
        assert_eq!(OrderField::Qty.prev(), OrderField::OrderType);
        assert_eq!(OrderField::OrderType.prev(), OrderField::Side);
        assert_eq!(OrderField::Side.prev(), OrderField::Symbol);
    }

    // ── filtered_orders ───────────────────────────────────────────────────────

    #[test]
    fn filtered_orders_open_includes_correct_statuses() {
        let mut app = make_test_app();
        app.orders = vec![
            make_order("1", "accepted"),
            make_order("2", "pending_new"),
            make_order("3", "partially_filled"),
            make_order("4", "held"),
            make_order("5", "new"),
            make_order("6", "filled"),
            make_order("7", "canceled"),
        ];
        app.orders_subtab = OrdersSubTab::Open;
        let open = app.filtered_orders();
        assert_eq!(open.len(), 5);
        assert!(!open
            .iter()
            .any(|o| o.status == "filled" || o.status == "canceled"));
    }

    #[test]
    fn filtered_orders_filled_only() {
        let mut app = make_test_app();
        app.orders = vec![
            make_order("1", "filled"),
            make_order("2", "accepted"),
            make_order("3", "filled"),
        ];
        app.orders_subtab = OrdersSubTab::Filled;
        let filled = app.filtered_orders();
        assert_eq!(filled.len(), 2);
        assert!(filled.iter().all(|o| o.status == "filled"));
    }

    #[test]
    fn filtered_orders_cancelled_includes_all_terminal_statuses() {
        let mut app = make_test_app();
        app.orders = vec![
            make_order("1", "canceled"),
            make_order("2", "expired"),
            make_order("3", "rejected"),
            make_order("4", "replaced"),
            make_order("5", "filled"),
        ];
        app.orders_subtab = OrdersSubTab::Cancelled;
        let cancelled = app.filtered_orders();
        assert_eq!(cancelled.len(), 4);
    }

    #[test]
    fn filtered_orders_empty_returns_empty() {
        let mut app = make_test_app();
        app.orders_subtab = OrdersSubTab::Open;
        assert!(app.filtered_orders().is_empty());
    }

    // ── push_equity ───────────────────────────────────────────────────────────

    #[test]
    fn push_equity_parses_and_appends_cents() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            equity: "1000.50".into(),
            ..Default::default()
        });
        app.push_equity();
        assert_eq!(app.equity_history, vec![100050]);
    }

    #[test]
    fn push_equity_caps_at_120_entries() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            equity: "1".into(),
            ..Default::default()
        });
        for _ in 0..121 {
            app.push_equity();
        }
        assert_eq!(app.equity_history.len(), 120);
    }

    #[test]
    fn push_equity_ignores_non_numeric_string() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            equity: "N/A".into(),
            ..Default::default()
        });
        app.push_equity();
        assert!(app.equity_history.is_empty());
    }

    #[test]
    fn push_equity_no_account_is_noop() {
        let mut app = make_test_app();
        app.push_equity();
        assert!(app.equity_history.is_empty());
    }

    // ── selected_watchlist_symbol ─────────────────────────────────────────────

    #[test]
    fn selected_watchlist_symbol_returns_at_index() {
        let mut app = make_test_app();
        app.watchlist = Some(make_watchlist(&["AAPL", "TSLA", "NVDA"]));
        app.watchlist_state.select(Some(1));
        assert_eq!(app.selected_watchlist_symbol(), Some("TSLA".into()));
    }

    #[test]
    fn selected_watchlist_symbol_none_when_no_selection() {
        let mut app = make_test_app();
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        assert_eq!(app.selected_watchlist_symbol(), None);
    }

    #[test]
    fn selected_watchlist_symbol_with_search_filter() {
        let mut app = make_test_app();
        app.watchlist = Some(make_watchlist(&["AAPL", "TSLA", "AMD"]));
        app.searching = true;
        app.search_query = "ts".into();
        app.watchlist_state.select(Some(0)); // index 0 of the *filtered* list = TSLA
        assert_eq!(app.selected_watchlist_symbol(), Some("TSLA".into()));
    }
}
