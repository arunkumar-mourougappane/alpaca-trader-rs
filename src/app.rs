use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use ratatui::layout::Rect;

/// Maximum number of status messages held in the queue at once.
const STATUS_QUEUE_CAP: usize = 5;

/// Minimum gap between equity-history samples pushed from streaming quotes.
///
/// Prevents flooding `equity_history` when many quotes arrive in rapid succession.
const EQUITY_STREAM_INTERVAL: Duration = Duration::from_secs(1);

///
/// Transient messages (e.g. "Order submitted", "Refreshing…") carry a TTL and are
/// cleared automatically on the next `Tick` after they expire. Persistent messages (errors,
/// "Loading…") set `expires_at = None` so they stay until replaced.
#[derive(Clone, Debug)]
pub struct StatusMessage {
    pub text: String,
    pub expires_at: Option<Instant>,
}

impl StatusMessage {
    /// Creates a transient message that auto-dismisses after the given duration.
    pub fn with_ttl(text: impl Into<String>, ttl: Duration) -> Self {
        Self {
            text: text.into(),
            expires_at: Some(Instant::now() + ttl),
        }
    }

    /// Creates a persistent message that stays until explicitly replaced.
    pub fn persistent(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            expires_at: None,
        }
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
                dry_run: false,
            },
            crate::prefs::AppPrefs::default(),
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
            filled_avg_price: None,
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
use crate::prefs::AppPrefs;
use crate::types::{
    AccountInfo, MarketClock, Order, Position, PriceAlert, Quote, Snapshot, Watchlist,
};
use crate::ui::theme::Theme;

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

/// The side of an order: buy, sell (close long), or sell short (open short).
#[derive(Debug, Clone, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
    SellShort,
}

impl OrderSide {
    /// Returns the next side in the cycle (used by left/right toggle in the order form).
    pub fn cycle_next(&self) -> Self {
        match self {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::SellShort,
            OrderSide::SellShort => OrderSide::Buy,
        }
    }

    /// Returns the previous side in the cycle.
    pub fn cycle_prev(&self) -> Self {
        match self {
            OrderSide::Buy => OrderSide::SellShort,
            OrderSide::Sell => OrderSide::Buy,
            OrderSide::SellShort => OrderSide::Sell,
        }
    }

    /// The wire value sent to the Alpaca API.
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
            OrderSide::SellShort => "sell_short",
        }
    }
}

/// Extended order execution type for the order entry form.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FullOrderType {
    /// Market order — executes immediately at the best available price.
    Market,
    /// Limit order — executes only at the specified limit price or better.
    #[default]
    Limit,
    /// Stop order — triggers a market order when the stop price is reached.
    Stop,
    /// Stop-limit order — triggers a limit order when the stop price is reached.
    StopLimit,
    /// Trailing stop — follows the price by a fixed dollar amount or percentage.
    TrailingStop,
}

impl FullOrderType {
    /// The wire string sent to the Alpaca API (`type` field).
    pub fn as_str(&self) -> &'static str {
        match self {
            FullOrderType::Market => "market",
            FullOrderType::Limit => "limit",
            FullOrderType::Stop => "stop",
            FullOrderType::StopLimit => "stop_limit",
            FullOrderType::TrailingStop => "trailing_stop",
        }
    }

    /// Returns the next type in the cycle (Market → Limit → Stop → StopLimit → TrailingStop → Market).
    pub fn cycle_next(&self) -> Self {
        match self {
            FullOrderType::Market => FullOrderType::Limit,
            FullOrderType::Limit => FullOrderType::Stop,
            FullOrderType::Stop => FullOrderType::StopLimit,
            FullOrderType::StopLimit => FullOrderType::TrailingStop,
            FullOrderType::TrailingStop => FullOrderType::Market,
        }
    }

    /// Returns the previous type in the cycle.
    pub fn cycle_prev(&self) -> Self {
        match self {
            FullOrderType::Market => FullOrderType::TrailingStop,
            FullOrderType::Limit => FullOrderType::Market,
            FullOrderType::Stop => FullOrderType::Limit,
            FullOrderType::StopLimit => FullOrderType::Stop,
            FullOrderType::TrailingStop => FullOrderType::StopLimit,
        }
    }
}

/// Whether a trailing stop is defined by dollar amount or percentage.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TrailType {
    /// Dollar amount trail (e.g., $5.00).
    #[default]
    Price,
    /// Percentage trail (e.g., 2.0%).
    Percent,
}

impl TrailType {
    /// Toggle between the two variants.
    pub fn toggle(&self) -> Self {
        match self {
            TrailType::Price => TrailType::Percent,
            TrailType::Percent => TrailType::Price,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderField {
    Symbol,
    Side,
    OrderType,
    Qty,
    /// Limit price — visible for [`FullOrderType::Limit`] and [`FullOrderType::StopLimit`].
    Price,
    /// Stop trigger price — visible for [`FullOrderType::Stop`] and [`FullOrderType::StopLimit`].
    StopPrice,
    /// Trail dollar/percent amount — visible for [`FullOrderType::TrailingStop`].
    TrailAmount,
    /// Toggle between $ and % trail — visible for [`FullOrderType::TrailingStop`].
    TrailMode,
    /// Extended-hours trading flag — visible for [`FullOrderType::Limit`] only.
    ExtendedHours,
    TimeInForce,
    /// Bracket order checkbox — visible for Market and Limit order types, non-short sides.
    Bracket,
    /// Take-profit limit price — visible when bracket is enabled.
    TakeProfit,
    /// Stop-loss stop price — visible when bracket is enabled.
    StopLoss,
    /// Stop-loss limit price (optional) — visible when bracket is enabled.
    StopLossLimit,
    Submit,
}

impl OrderField {
    pub fn next(&self) -> Self {
        match self {
            OrderField::Symbol => OrderField::Side,
            OrderField::Side => OrderField::OrderType,
            OrderField::OrderType => OrderField::Qty,
            OrderField::Qty => OrderField::Price,
            OrderField::Price => OrderField::StopPrice,
            OrderField::StopPrice => OrderField::TrailAmount,
            OrderField::TrailAmount => OrderField::TrailMode,
            OrderField::TrailMode => OrderField::ExtendedHours,
            OrderField::ExtendedHours => OrderField::TimeInForce,
            OrderField::TimeInForce => OrderField::Bracket,
            OrderField::Bracket => OrderField::TakeProfit,
            OrderField::TakeProfit => OrderField::StopLoss,
            OrderField::StopLoss => OrderField::StopLossLimit,
            OrderField::StopLossLimit => OrderField::Submit,
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
            OrderField::StopPrice => OrderField::Price,
            OrderField::TrailAmount => OrderField::StopPrice,
            OrderField::TrailMode => OrderField::TrailAmount,
            OrderField::ExtendedHours => OrderField::TrailMode,
            OrderField::TimeInForce => OrderField::ExtendedHours,
            OrderField::Bracket => OrderField::TimeInForce,
            OrderField::TakeProfit => OrderField::Bracket,
            OrderField::StopLoss => OrderField::TakeProfit,
            OrderField::StopLossLimit => OrderField::StopLoss,
            OrderField::Submit => OrderField::StopLossLimit,
        }
    }

    /// Returns whether this field is shown for the given order type and bracket state.
    pub fn is_visible_for(&self, order_type: &FullOrderType, bracket: bool) -> bool {
        match self {
            OrderField::Symbol
            | OrderField::Side
            | OrderField::OrderType
            | OrderField::Qty
            | OrderField::TimeInForce
            | OrderField::Submit => true,
            OrderField::Price => {
                matches!(order_type, FullOrderType::Limit | FullOrderType::StopLimit)
            }
            OrderField::StopPrice => {
                matches!(order_type, FullOrderType::Stop | FullOrderType::StopLimit)
            }
            OrderField::TrailAmount | OrderField::TrailMode => {
                matches!(order_type, FullOrderType::TrailingStop)
            }
            OrderField::ExtendedHours => matches!(order_type, FullOrderType::Limit),
            OrderField::Bracket => {
                matches!(order_type, FullOrderType::Market | FullOrderType::Limit)
            }
            OrderField::TakeProfit | OrderField::StopLoss | OrderField::StopLossLimit => {
                matches!(order_type, FullOrderType::Market | FullOrderType::Limit) && bracket
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderEntryState {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: FullOrderType,
    pub gtc_order: bool, // true = GTC, false = DAY
    pub qty_input: String,
    pub price_input: String,
    pub stop_price_input: String,
    pub trail_input: String,
    pub trail_type: TrailType,
    pub extended_hours: bool,
    pub focused_field: OrderField,
    pub bracket: bool,
    pub take_profit_price: String,
    pub stop_loss_price: String,
    pub stop_loss_limit_price: String,
}

impl OrderEntryState {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            side: OrderSide::Buy,
            order_type: FullOrderType::Limit,
            gtc_order: false,
            qty_input: String::new(),
            price_input: String::new(),
            stop_price_input: String::new(),
            trail_input: String::new(),
            trail_type: TrailType::Price,
            extended_hours: false,
            focused_field: OrderField::Qty,
            bracket: false,
            take_profit_price: String::new(),
            stop_loss_price: String::new(),
            stop_loss_limit_price: String::new(),
        }
    }

    /// Builder method to set the order side, allowing one-liner construction.
    pub fn with_side(mut self, side: OrderSide) -> Self {
        self.side = side;
        self
    }

    /// Advance focus to the next field that is visible for the current order type and bracket state.
    pub fn next_field(&self) -> OrderField {
        let mut f = self.focused_field.next();
        // Submit is always visible, so this loop always terminates.
        while !f.is_visible_for(&self.order_type, self.bracket) {
            f = f.next();
        }
        f
    }

    /// Retreat focus to the previous field that is visible for the current order type and bracket state.
    pub fn prev_field(&self) -> OrderField {
        let mut f = self.focused_field.prev();
        while !f.is_visible_for(&self.order_type, self.bracket) {
            f = f.prev();
        }
        f
    }
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    CancelOrder(String),
}

/// Focusable input field in the [`Modal::SetAlert`] dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum AlertField {
    /// "Above" threshold input.
    Above,
    /// "Below" threshold input.
    Below,
}

impl AlertField {
    /// Toggle to the other field.
    pub fn toggle(&self) -> Self {
        match self {
            AlertField::Above => AlertField::Below,
            AlertField::Below => AlertField::Above,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Modal {
    Help,
    About,
    OrderEntry(OrderEntryState),
    SymbolDetail(String),
    Confirm {
        message: String,
        action: ConfirmAction,
        confirmed: bool,
    },
    /// Dedicated confirmation dialog for removing a symbol from the watchlist.
    ///
    /// Shows a focused modal with `[y] Yes` / `[n / Esc] No` buttons.
    /// On confirmation the `RemoveFromWatchlist` command is dispatched.
    ConfirmRemoveWatchlist {
        symbol: String,
        watchlist_id: String,
    },
    AddSymbol {
        input: String,
        watchlist_id: String,
    },
    /// Floating search input for looking up any ticker symbol globally.
    ///
    /// User types a symbol and presses `Enter` to open [`Modal::SymbolDetail`]
    /// for that symbol (fetching intraday bars) without adding it to the watchlist.
    /// `Esc` dismisses without action.
    GlobalSearch {
        /// The symbol characters typed so far (uppercased).
        query: String,
    },
    /// Dedicated position detail view for a held position.
    ///
    /// Shows an intraday chart, position P/L summary, and related open orders
    /// for the symbol. `Esc` dismisses; `o` opens a new order for the symbol.
    PositionDetail {
        /// Ticker symbol whose position is being viewed.
        symbol: String,
    },
    /// Price alert configuration dialog for a watchlist symbol.
    ///
    /// The user enters optional upper and/or lower price thresholds.
    /// On `Enter`/Save the thresholds are stored in [`App::price_alerts`].
    /// On `Esc` the dialog is dismissed without changes.
    SetAlert {
        /// The watchlist symbol this alert applies to.
        symbol: String,
        /// Text currently typed in the "above" price field.
        above_input: String,
        /// Text currently typed in the "below" price field.
        below_input: String,
        /// Which input field currently has keyboard focus.
        focused: AlertField,
    },
}

/// Date range for the equity-history chart.
///
/// Controls both the API query parameters (`period` / `timeframe`) sent to
/// Alpaca and the x-axis labels shown in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EquityRange {
    /// Intraday (current trading day), 1-minute bars.
    #[default]
    OneDay,
    /// Past week, hourly bars.
    OneWeek,
    /// Past month, daily bars.
    OneMonth,
    /// Year-to-date, daily bars.
    Ytd,
}

impl EquityRange {
    /// Cycle to the next range in order: 1D → 1W → 1M → YTD → 1D.
    pub fn cycle(self) -> Self {
        match self {
            Self::OneDay => Self::OneWeek,
            Self::OneWeek => Self::OneMonth,
            Self::OneMonth => Self::Ytd,
            Self::Ytd => Self::OneDay,
        }
    }

    /// Short label shown in the chart title (e.g., `"1D"`, `"1W"`).
    pub fn label(self) -> &'static str {
        match self {
            Self::OneDay => "1D",
            Self::OneWeek => "1W",
            Self::OneMonth => "1M",
            Self::Ytd => "YTD",
        }
    }

    /// `(period, timeframe)` query parameters for the Alpaca portfolio history API.
    pub fn api_params(self) -> (&'static str, &'static str) {
        match self {
            Self::OneDay => ("1D", "1Min"),
            Self::OneWeek => ("1W", "1H"),
            Self::OneMonth => ("1M", "1D"),
            Self::Ytd => ("YTD", "1D"),
        }
    }

    /// `[start_label, end_label]` for the chart x-axis.
    pub fn x_labels(self) -> [&'static str; 2] {
        match self {
            Self::OneDay => ["09:30", "16:00"],
            Self::OneWeek => ["Mon", "Fri"],
            Self::OneMonth => ["Day 1", "Day 30"],
            Self::Ytd => ["Jan", "Today"],
        }
    }
}

/// Sort direction used by [`SortState`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SortDir {
    /// Ascending order (A → Z, lowest → highest).
    #[default]
    Asc,
    /// Descending order (Z → A, highest → lowest).
    Desc,
}

/// Sortable column in the Positions table.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PositionSortCol {
    /// No explicit sort; positions are shown in API-returned order.
    #[default]
    None,
    Symbol,
    Qty,
    AvgCost,
    MarketValue,
    UnrealizedPl,
    Pct,
}

impl PositionSortCol {
    /// Advance to the next column in the cycle, wrapping back to `None`.
    pub fn cycle(self) -> Self {
        match self {
            Self::None => Self::Symbol,
            Self::Symbol => Self::Qty,
            Self::Qty => Self::AvgCost,
            Self::AvgCost => Self::MarketValue,
            Self::MarketValue => Self::UnrealizedPl,
            Self::UnrealizedPl => Self::Pct,
            Self::Pct => Self::None,
        }
    }
}

/// Sortable column in the Orders table.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum OrderSortCol {
    /// No explicit sort; orders are shown in API-returned order.
    #[default]
    None,
    Symbol,
    Side,
    Type,
    Status,
    Submitted,
}

impl OrderSortCol {
    /// Advance to the next column in the cycle, wrapping back to `None`.
    pub fn cycle(self) -> Self {
        match self {
            Self::None => Self::Symbol,
            Self::Symbol => Self::Side,
            Self::Side => Self::Type,
            Self::Type => Self::Status,
            Self::Status => Self::Submitted,
            Self::Submitted => Self::None,
        }
    }
}

/// Sort state for a table: which column is active and what direction.
#[derive(Debug, Clone, Default)]
pub struct SortState<C: Default + Copy + PartialEq> {
    /// The currently active sort column.
    pub col: C,
    /// Sort direction.
    pub dir: SortDir,
}

impl<C: Default + Copy + PartialEq> SortState<C> {
    /// Toggle between ascending and descending.
    pub fn toggle_dir(&mut self) {
        self.dir = match self.dir {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        };
    }
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
    /// Full area of the equity chart block on the Account tab.
    /// Used to hit-test mouse clicks and map column → data-point index.
    pub equity_chart_area: Rect,
    /// Bounding box of the currently rendered modal popup.
    /// A left-click outside this area dismisses the modal.
    pub modal_popup_area: Option<Rect>,
}

pub struct App {
    pub config: AlpacaConfig,
    pub prefs: AppPrefs,
    /// The currently active colour theme. Changed at runtime via the `T` key.
    pub current_theme: Theme,
    pub refresh_notify: Arc<Notify>,
    pub command_tx: mpsc::Sender<Command>,
    pub symbol_tx: watch::Sender<Vec<String>>,

    pub account: Option<AccountInfo>,
    pub positions: Vec<Position>,
    pub orders: Vec<Order>,
    pub quotes: HashMap<String, Quote>,
    pub watchlist: Option<Watchlist>,
    /// Set to `true` when the paper trading endpoint signals that watchlists
    /// are unsupported. Drives a persistent informational message in the UI.
    pub watchlist_unavailable: bool,
    pub snapshots: HashMap<String, Snapshot>,
    pub clock: Option<MarketClock>,
    pub equity_history: Vec<u64>,
    /// Active date range displayed in the equity chart.
    ///
    /// Determines both the x-axis labels and the API parameters used when
    /// re-fetching history after the user cycles the range with `p`.
    pub equity_range: EquityRange,
    /// Intraday 1-minute close prices in cents, keyed by ticker symbol.
    pub intraday_bars: HashMap<String, Vec<u64>>,

    /// In-memory price alerts keyed by ticker symbol.
    ///
    /// Each entry holds optional upper/lower thresholds.  When a streaming
    /// quote crosses a threshold the status bar flashes and the terminal
    /// bell (`\x07`) is emitted.  Alerts persist for the lifetime of the
    /// session; they are not written to disk.
    pub price_alerts: HashMap<String, PriceAlert>,

    pub active_tab: Tab,
    pub watchlist_state: TableState,
    pub positions_state: TableState,
    pub orders_state: TableState,
    pub orders_subtab: OrdersSubTab,

    pub modal: Option<Modal>,
    pub search_query: String,
    pub searching: bool,

    pub status_queue: VecDeque<StatusMessage>,
    pub should_quit: bool,
    /// Set to `true` by the `Event::Resize` handler to request an immediate
    /// redraw before the next tick. Cleared by the main loop after drawing.
    pub needs_redraw: bool,

    /// Number of in-flight REST requests.
    ///
    /// Incremented by `Event::FetchStarted`, decremented by `Event::FetchComplete`.
    /// Non-zero while any fetch is in-flight; used to show the loading spinner.
    pub pending_requests: u8,
    /// Wall-clock time of the most recent complete data refresh, updated when
    /// `pending_requests` drops to zero.
    pub last_updated: Option<DateTime<Local>>,
    /// Frame index advanced on every `Event::Tick` while `pending_requests > 0`.
    /// Used to cycle through spinner frames without storing wall-clock timers.
    pub spinner_tick: u8,

    /// `true` while the market-data WebSocket is connected and authenticated.
    pub market_stream_ok: bool,
    /// `true` while the account WebSocket is connected and authenticated.
    pub account_stream_ok: bool,
    /// `true` while the market-data stream is in a reconnect back-off window.
    pub market_stream_reconnecting: bool,
    /// `true` while the account stream is in a reconnect back-off window.
    pub account_stream_reconnecting: bool,
    /// 1-based reconnect attempt counter for the market-data stream.
    ///
    /// Reset to `0` when the stream connects successfully. Non-zero when
    /// `!market_stream_ok`; used to distinguish "initial loading" from
    /// "permanently offline after exhausting max attempts".
    pub market_reconnect_attempt: u32,
    /// 1-based reconnect attempt counter for the account stream.
    ///
    /// See `market_reconnect_attempt` for semantics.
    pub account_reconnect_attempt: u32,

    /// Interactive element positions from the last rendered frame.
    pub hit_areas: HitAreas,

    /// Tracks the last left-click's (row, time) for double-click detection.
    ///
    /// A second click on the same terminal row within 400 ms is treated as a
    /// double-click and opens the detail modal (equivalent to pressing `Enter`).
    pub last_click: Option<(u16, Instant)>,

    /// Timestamp of the first `g` keypress for `gg` (jump-to-top) detection.
    ///
    /// Set on the first `g`; cleared when a second `g` arrives within 500 ms
    /// (firing jump-to-top) or when any other key clears the pending state.
    pub pending_g_at: Option<Instant>,

    /// Timestamp of the last equity-history point pushed from streaming quotes.
    ///
    /// Used to throttle how often `push_equity_from_quotes` appends a new
    /// sample so the chart isn't flooded on every incoming quote.
    pub last_equity_stream_push: Option<Instant>,

    /// Tracks when each symbol's intraday bars were last fetched.
    ///
    /// Keyed by ticker symbol; updated whenever `Event::IntradayBarsReceived`
    /// arrives. The `Tick` handler uses this to schedule periodic re-fetches
    /// while a symbol-detail modal is open.
    pub intraday_fetched_at: HashMap<String, Instant>,

    /// Index into [`App::equity_history`] that the crosshair cursor is pointing at.
    ///
    /// `None` means no crosshair is shown. Set by `←`/`→` keys while the
    /// Account tab is active; cleared by `Esc`.
    pub equity_chart_cursor: Option<usize>,

    /// Index into the intraday bars of the currently open [`Modal::SymbolDetail`]
    /// that the crosshair is pointing at.
    ///
    /// `None` means no crosshair is shown. Activated by `←`/`→` while
    /// `Modal::SymbolDetail` is open; cleared by `Esc` (first `Esc` clears
    /// the crosshair, second `Esc` closes the modal).
    pub symbol_detail_crosshair: Option<usize>,

    /// Active sort column and direction for the Positions table.
    pub positions_sort: SortState<PositionSortCol>,

    /// Active sort column and direction for the Orders table.
    pub orders_sort: SortState<OrderSortCol>,

    /// Symbol prefix filter applied to the Orders table.
    ///
    /// Empty string means no filter is active. When non-empty, only orders
    /// whose symbol contains this string (case-insensitive) are shown.
    pub orders_symbol_filter: String,

    /// Whether the Orders table is currently in filter-input mode.
    ///
    /// While `true`, printable key-presses are appended to `orders_symbol_filter`
    /// instead of being handled as navigation or action shortcuts.
    pub orders_filter_active: bool,
}

impl App {
    pub fn new(
        config: AlpacaConfig,
        prefs: AppPrefs,
        refresh_notify: Arc<Notify>,
        command_tx: mpsc::Sender<Command>,
        symbol_tx: watch::Sender<Vec<String>>,
    ) -> Self {
        let current_theme = Theme::from_str(&prefs.ui.theme);
        Self {
            config,
            prefs,
            current_theme,
            refresh_notify,
            command_tx,
            symbol_tx,
            account: None,
            positions: Vec::new(),
            orders: Vec::new(),
            quotes: HashMap::new(),
            watchlist: None,
            watchlist_unavailable: false,
            snapshots: HashMap::new(),
            clock: None,
            equity_history: Vec::new(),
            equity_range: EquityRange::OneDay,
            intraday_bars: HashMap::new(),
            active_tab: Tab::Account,
            watchlist_state: TableState::default(),
            positions_state: TableState::default(),
            orders_state: TableState::default(),
            orders_subtab: OrdersSubTab::Open,
            modal: None,
            search_query: String::new(),
            searching: false,
            status_queue: VecDeque::new(),
            should_quit: false,
            needs_redraw: false,
            pending_requests: 0,
            last_updated: None,
            spinner_tick: 0,
            market_stream_ok: false,
            account_stream_ok: false,
            market_stream_reconnecting: false,
            account_stream_reconnecting: false,
            market_reconnect_attempt: 0,
            account_reconnect_attempt: 0,
            hit_areas: HitAreas::default(),
            last_click: None,
            pending_g_at: None,
            last_equity_stream_push: None,
            intraday_fetched_at: HashMap::new(),
            equity_chart_cursor: None,
            symbol_detail_crosshair: None,
            positions_sort: SortState::default(),
            orders_sort: SortState::default(),
            orders_symbol_filter: String::new(),
            orders_filter_active: false,
            price_alerts: HashMap::new(),
        }
    }

    pub fn filtered_orders(&self) -> Vec<&Order> {
        let filter = self.orders_symbol_filter.to_uppercase();
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
            .filter(|o| filter.is_empty() || o.symbol.to_uppercase().contains(&filter))
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

    /// Returns the ticker symbol of the selected row in the Orders table.
    pub fn selected_order_symbol(&self) -> Option<String> {
        let orders = self.filtered_orders();
        let i = self.orders_state.selected()?;
        orders.get(i).map(|o| o.symbol.clone())
    }

    /// Returns the ticker symbol of the focused row in whichever table tab is active.
    ///
    /// Returns `None` when no row is selected or the active tab has no symbols
    /// (e.g. the Account tab).
    pub fn focused_symbol(&self) -> Option<String> {
        match self.active_tab {
            Tab::Watchlist => self.selected_watchlist_symbol(),
            Tab::Positions => self.selected_position_symbol(),
            Tab::Orders => self.selected_order_symbol(),
            Tab::Account => None,
        }
    }

    pub fn push_equity(&mut self) {
        if self.equity_range != EquityRange::OneDay {
            return;
        }
        if let Some(account) = &self.account {
            if let Ok(v) = account.equity.parse::<f64>() {
                self.equity_history.push((v * 100.0) as u64);
                if self.equity_history.len() > 120 {
                    self.equity_history.remove(0);
                }
            }
        }
    }

    /// Appends an estimated equity data point computed from live streaming quotes.
    ///
    /// Called on every `MarketQuote` event so the equity chart updates
    /// between REST polls without any extra API calls.
    ///
    /// The estimate is: `account.cash + Σ(qty × mid_price)` for each open
    /// position, where `mid_price` is the ask or bid from the latest streaming
    /// quote for that symbol, falling back to the position's `current_price`
    /// when no live quote is available yet.
    ///
    /// Calls are throttled to at most once per [`EQUITY_STREAM_INTERVAL`] to
    /// avoid flooding `equity_history` when quotes arrive in rapid succession.
    /// Skips silently when there are no open positions (nothing to compute).
    pub fn push_equity_from_quotes(&mut self) {
        // Only meaningful for intraday; other ranges are static snapshots.
        if self.equity_range != EquityRange::OneDay {
            return;
        }
        // Throttle: skip if we pushed a streaming sample too recently.
        if let Some(last) = self.last_equity_stream_push {
            if last.elapsed() < EQUITY_STREAM_INTERVAL {
                return;
            }
        }

        // No positions → no meaningful estimate to push.
        if self.positions.is_empty() {
            return;
        }

        let position_value: f64 = self
            .positions
            .iter()
            .filter_map(|p| {
                let qty = p.qty.parse::<f64>().ok()?;
                // Prefer live quote (ask then bid), fall back to last REST price.
                let price = self
                    .quotes
                    .get(&p.symbol)
                    .and_then(|q| q.ap.or(q.bp))
                    .or_else(|| p.current_price.parse::<f64>().ok())?;
                Some(qty * price)
            })
            .sum();

        let cash: f64 = self
            .account
            .as_ref()
            .and_then(|a| a.cash.parse::<f64>().ok())
            .unwrap_or(0.0);

        let equity = cash + position_value;
        if equity > 0.0 {
            self.equity_history.push((equity * 100.0) as u64);
            if self.equity_history.len() > 120 {
                self.equity_history.remove(0);
            }
            self.last_equity_stream_push = Some(Instant::now());
        }
    }

    /// Enqueues a status message.
    ///
    /// If the queue is already at [`STATUS_QUEUE_CAP`], the oldest entry is
    /// dropped from the front to make room. Persistent (no-TTL) messages that
    /// are already at the front are not displaced — transient messages are
    /// appended behind them so the persistent message stays visible.
    pub fn push_status(&mut self, msg: StatusMessage) {
        if self.status_queue.len() >= STATUS_QUEUE_CAP {
            self.status_queue.pop_front();
        }
        self.status_queue.push_back(msg);
    }

    /// Returns the text of the current (front) status message, or `""` if the
    /// queue is empty or the front message has no text.
    pub fn current_status_text(&self) -> &str {
        self.status_queue
            .front()
            .map(|m| m.text.as_str())
            .unwrap_or("")
    }

    /// Sets a transient status message using the TTL from user preferences.
    pub fn push_transient_status(&mut self, text: impl Into<String>) {
        self.push_status(StatusMessage::with_ttl(text, self.prefs.status_ttl()));
    }

    /// Sets a fill-notification status message using the fill TTL from user preferences.
    pub fn push_fill_notification(&mut self, text: impl Into<String>) {
        self.push_status(StatusMessage::with_ttl(text, self.prefs.fill_ttl()));
    }

    /// Increments the in-flight request counter (called when a fetch begins).
    pub fn request_started(&mut self) {
        self.pending_requests = self.pending_requests.saturating_add(1);
    }

    /// Decrements the in-flight request counter (called when a fetch completes).
    ///
    /// When the counter reaches zero, [`last_updated`](App::last_updated) is
    /// set to the current local time.
    pub fn request_finished(&mut self) {
        self.pending_requests = self.pending_requests.saturating_sub(1);
        if self.pending_requests == 0 {
            self.last_updated = Some(Local::now());
        }
    }

    /// Returns the current spinner frame character for the active-fetch indicator.
    ///
    /// Cycles through the ten braille-dot frames on each tick.
    pub fn spinner_frame(&self) -> char {
        const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        FRAMES[(self.spinner_tick as usize) % FRAMES.len()]
    }

    /// Advances the spinner by one frame (called on each `Event::Tick` while busy).
    pub fn tick_spinner(&mut self) {
        self.spinner_tick = self.spinner_tick.wrapping_add(1);
    }

    /// Advances to the next theme in the cycle (Default → Dark → High-contrast → Default).
    ///
    /// Updates `prefs.ui.theme` and persists it to disk silently. If the config
    /// file is unavailable the change is kept in memory only.
    pub fn cycle_theme(&mut self) {
        self.current_theme = self.current_theme.cycle();
        self.prefs.ui.theme = self.current_theme.as_str().to_string();
        if let Some(path) = AppPrefs::default_path() {
            if let Err(e) = self.prefs.write_to(&path) {
                tracing::warn!(error = %e, "could not persist theme to config");
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
        assert_eq!(OrderField::Price.next(), OrderField::StopPrice);
        assert_eq!(OrderField::StopPrice.next(), OrderField::TrailAmount);
        assert_eq!(OrderField::TrailAmount.next(), OrderField::TrailMode);
        assert_eq!(OrderField::TrailMode.next(), OrderField::ExtendedHours);
        assert_eq!(OrderField::ExtendedHours.next(), OrderField::TimeInForce);
        assert_eq!(OrderField::TimeInForce.next(), OrderField::Bracket);
        assert_eq!(OrderField::Bracket.next(), OrderField::TakeProfit);
        assert_eq!(OrderField::TakeProfit.next(), OrderField::StopLoss);
        assert_eq!(OrderField::StopLoss.next(), OrderField::StopLossLimit);
        assert_eq!(OrderField::StopLossLimit.next(), OrderField::Submit);
        assert_eq!(OrderField::Submit.next(), OrderField::Symbol);
    }

    #[test]
    fn order_field_prev_full_cycle() {
        assert_eq!(OrderField::Symbol.prev(), OrderField::Submit);
        assert_eq!(OrderField::Submit.prev(), OrderField::StopLossLimit);
        assert_eq!(OrderField::StopLossLimit.prev(), OrderField::StopLoss);
        assert_eq!(OrderField::StopLoss.prev(), OrderField::TakeProfit);
        assert_eq!(OrderField::TakeProfit.prev(), OrderField::Bracket);
        assert_eq!(OrderField::Bracket.prev(), OrderField::TimeInForce);
        assert_eq!(OrderField::TimeInForce.prev(), OrderField::ExtendedHours);
        assert_eq!(OrderField::ExtendedHours.prev(), OrderField::TrailMode);
        assert_eq!(OrderField::TrailMode.prev(), OrderField::TrailAmount);
        assert_eq!(OrderField::TrailAmount.prev(), OrderField::StopPrice);
        assert_eq!(OrderField::StopPrice.prev(), OrderField::Price);
        assert_eq!(OrderField::Price.prev(), OrderField::Qty);
        assert_eq!(OrderField::Qty.prev(), OrderField::OrderType);
        assert_eq!(OrderField::OrderType.prev(), OrderField::Side);
        assert_eq!(OrderField::Side.prev(), OrderField::Symbol);
    }

    // ── FullOrderType ─────────────────────────────────────────────────────────

    #[test]
    fn full_order_type_as_str() {
        assert_eq!(FullOrderType::Market.as_str(), "market");
        assert_eq!(FullOrderType::Limit.as_str(), "limit");
        assert_eq!(FullOrderType::Stop.as_str(), "stop");
        assert_eq!(FullOrderType::StopLimit.as_str(), "stop_limit");
        assert_eq!(FullOrderType::TrailingStop.as_str(), "trailing_stop");
    }

    #[test]
    fn full_order_type_cycle_next_full_cycle() {
        assert_eq!(FullOrderType::Market.cycle_next(), FullOrderType::Limit);
        assert_eq!(FullOrderType::Limit.cycle_next(), FullOrderType::Stop);
        assert_eq!(FullOrderType::Stop.cycle_next(), FullOrderType::StopLimit);
        assert_eq!(
            FullOrderType::StopLimit.cycle_next(),
            FullOrderType::TrailingStop
        );
        assert_eq!(
            FullOrderType::TrailingStop.cycle_next(),
            FullOrderType::Market
        );
    }

    #[test]
    fn full_order_type_cycle_prev_full_cycle() {
        assert_eq!(
            FullOrderType::Market.cycle_prev(),
            FullOrderType::TrailingStop
        );
        assert_eq!(FullOrderType::Limit.cycle_prev(), FullOrderType::Market);
        assert_eq!(FullOrderType::Stop.cycle_prev(), FullOrderType::Limit);
        assert_eq!(FullOrderType::StopLimit.cycle_prev(), FullOrderType::Stop);
        assert_eq!(
            FullOrderType::TrailingStop.cycle_prev(),
            FullOrderType::StopLimit
        );
    }

    #[test]
    fn full_order_type_default_is_limit() {
        assert_eq!(FullOrderType::default(), FullOrderType::Limit);
    }

    // ── TrailType ─────────────────────────────────────────────────────────────

    #[test]
    fn trail_type_toggle() {
        assert_eq!(TrailType::Price.toggle(), TrailType::Percent);
        assert_eq!(TrailType::Percent.toggle(), TrailType::Price);
    }

    #[test]
    fn trail_type_default_is_price() {
        assert_eq!(TrailType::default(), TrailType::Price);
    }

    // ── OrderField::is_visible_for ────────────────────────────────────────────

    #[test]
    fn order_field_visibility_market() {
        let ot = FullOrderType::Market;
        assert!(OrderField::Symbol.is_visible_for(&ot, false));
        assert!(OrderField::Side.is_visible_for(&ot, false));
        assert!(OrderField::OrderType.is_visible_for(&ot, false));
        assert!(OrderField::Qty.is_visible_for(&ot, false));
        assert!(!OrderField::Price.is_visible_for(&ot, false));
        assert!(!OrderField::StopPrice.is_visible_for(&ot, false));
        assert!(!OrderField::TrailAmount.is_visible_for(&ot, false));
        assert!(!OrderField::TrailMode.is_visible_for(&ot, false));
        assert!(!OrderField::ExtendedHours.is_visible_for(&ot, false));
        assert!(OrderField::TimeInForce.is_visible_for(&ot, false));
        assert!(OrderField::Submit.is_visible_for(&ot, false));
        assert!(OrderField::Bracket.is_visible_for(&ot, false));
    }

    #[test]
    fn order_field_visibility_limit() {
        let ot = FullOrderType::Limit;
        assert!(OrderField::Price.is_visible_for(&ot, false));
        assert!(!OrderField::StopPrice.is_visible_for(&ot, false));
        assert!(!OrderField::TrailAmount.is_visible_for(&ot, false));
        assert!(!OrderField::TrailMode.is_visible_for(&ot, false));
        assert!(OrderField::ExtendedHours.is_visible_for(&ot, false));
        assert!(OrderField::Bracket.is_visible_for(&ot, false));
    }

    #[test]
    fn order_field_visibility_stop() {
        let ot = FullOrderType::Stop;
        assert!(!OrderField::Price.is_visible_for(&ot, false));
        assert!(OrderField::StopPrice.is_visible_for(&ot, false));
        assert!(!OrderField::TrailAmount.is_visible_for(&ot, false));
        assert!(!OrderField::TrailMode.is_visible_for(&ot, false));
        assert!(!OrderField::ExtendedHours.is_visible_for(&ot, false));
        assert!(!OrderField::Bracket.is_visible_for(&ot, false));
    }

    #[test]
    fn order_field_visibility_stop_limit() {
        let ot = FullOrderType::StopLimit;
        assert!(OrderField::Price.is_visible_for(&ot, false));
        assert!(OrderField::StopPrice.is_visible_for(&ot, false));
        assert!(!OrderField::TrailAmount.is_visible_for(&ot, false));
        assert!(!OrderField::ExtendedHours.is_visible_for(&ot, false));
        assert!(!OrderField::Bracket.is_visible_for(&ot, false));
    }

    #[test]
    fn order_field_visibility_trailing_stop() {
        let ot = FullOrderType::TrailingStop;
        assert!(!OrderField::Price.is_visible_for(&ot, false));
        assert!(!OrderField::StopPrice.is_visible_for(&ot, false));
        assert!(OrderField::TrailAmount.is_visible_for(&ot, false));
        assert!(OrderField::TrailMode.is_visible_for(&ot, false));
        assert!(!OrderField::ExtendedHours.is_visible_for(&ot, false));
        assert!(!OrderField::Bracket.is_visible_for(&ot, false));
    }

    #[test]
    fn order_field_visibility_bracket_fields() {
        let market = FullOrderType::Market;
        let limit = FullOrderType::Limit;
        let stop = FullOrderType::Stop;
        assert!(OrderField::Bracket.is_visible_for(&market, false));
        assert!(OrderField::Bracket.is_visible_for(&limit, false));
        assert!(!OrderField::Bracket.is_visible_for(&stop, false));
        // Sub-fields hidden when bracket=false, visible when bracket=true
        assert!(!OrderField::TakeProfit.is_visible_for(&market, false));
        assert!(OrderField::TakeProfit.is_visible_for(&market, true));
        assert!(!OrderField::StopLoss.is_visible_for(&limit, false));
        assert!(OrderField::StopLoss.is_visible_for(&limit, true));
        assert!(!OrderField::StopLossLimit.is_visible_for(&stop, true));
    }

    // ── OrderEntryState::next_field / prev_field ──────────────────────────────

    #[test]
    fn next_field_skips_invisible_fields_for_market() {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::Market;
        s.focused_field = OrderField::Qty;
        // Price, StopPrice, TrailAmount, TrailMode, ExtendedHours invisible → skip to TIF
        assert_eq!(s.next_field(), OrderField::TimeInForce);
    }

    #[test]
    fn next_field_market_tif_to_bracket() {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::Market;
        s.focused_field = OrderField::TimeInForce;
        // Bracket is visible for Market
        assert_eq!(s.next_field(), OrderField::Bracket);
    }

    #[test]
    fn next_field_market_bracket_to_submit_when_disabled() {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::Market;
        s.bracket = false;
        s.focused_field = OrderField::Bracket;
        // TakeProfit/StopLoss/StopLossLimit invisible when bracket=false → Submit
        assert_eq!(s.next_field(), OrderField::Submit);
    }

    #[test]
    fn next_field_market_bracket_to_take_profit_when_enabled() {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::Market;
        s.bracket = true;
        s.focused_field = OrderField::Bracket;
        assert_eq!(s.next_field(), OrderField::TakeProfit);
    }

    #[test]
    fn prev_field_skips_invisible_fields_for_market() {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::Market;
        s.focused_field = OrderField::TimeInForce;
        // ExtendedHours, TrailMode, TrailAmount, StopPrice, Price are invisible → skip to Qty
        assert_eq!(s.prev_field(), OrderField::Qty);
    }

    #[test]
    fn next_field_stop_limit_shows_price_then_stop_price() {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::StopLimit;
        s.focused_field = OrderField::Qty;
        assert_eq!(s.next_field(), OrderField::Price);
        s.focused_field = OrderField::Price;
        assert_eq!(s.next_field(), OrderField::StopPrice);
        s.focused_field = OrderField::StopPrice;
        // TrailAmount, TrailMode, ExtendedHours, Bracket (Stop not bracket-eligible) invisible → TIF
        assert_eq!(s.next_field(), OrderField::TimeInForce);
    }

    #[test]
    fn next_field_trailing_stop_shows_trail_fields() {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::TrailingStop;
        s.focused_field = OrderField::Qty;
        // Price and StopPrice invisible → TrailAmount
        assert_eq!(s.next_field(), OrderField::TrailAmount);
        s.focused_field = OrderField::TrailAmount;
        assert_eq!(s.next_field(), OrderField::TrailMode);
        s.focused_field = OrderField::TrailMode;
        // ExtendedHours and Bracket invisible for TrailingStop → TIF
        assert_eq!(s.next_field(), OrderField::TimeInForce);
    }

    #[test]
    fn next_field_limit_shows_extended_hours() {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::Limit;
        s.focused_field = OrderField::Price;
        // StopPrice, TrailAmount, TrailMode invisible → ExtendedHours
        assert_eq!(s.next_field(), OrderField::ExtendedHours);
        s.focused_field = OrderField::ExtendedHours;
        assert_eq!(s.next_field(), OrderField::TimeInForce);
    }

    #[test]
    fn next_field_wraps_from_submit_to_symbol() {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::Market;
        s.focused_field = OrderField::Submit;
        assert_eq!(s.next_field(), OrderField::Symbol);
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

    #[test]
    fn filtered_orders_symbol_filter_narrows_results() {
        let mut app = make_test_app();
        app.orders = vec![
            make_order("1", "new"), // symbol = AAPL
            make_order("2", "new"), // symbol = AAPL
        ];
        // Override second order symbol via a custom Order
        app.orders[1].symbol = "TSLA".into();
        app.orders_subtab = OrdersSubTab::Open;
        app.orders_symbol_filter = "AAPL".to_string();
        let result = app.filtered_orders();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].symbol, "AAPL");
    }

    #[test]
    fn filtered_orders_symbol_filter_is_case_insensitive() {
        let mut app = make_test_app();
        app.orders = vec![make_order("1", "new")]; // symbol = AAPL
        app.orders_subtab = OrdersSubTab::Open;
        app.orders_symbol_filter = "aapl".to_string();
        assert_eq!(app.filtered_orders().len(), 1);
    }

    #[test]
    fn filtered_orders_symbol_filter_empty_shows_all() {
        let mut app = make_test_app();
        app.orders = vec![make_order("1", "new"), make_order("2", "new")];
        app.orders[1].symbol = "TSLA".into();
        app.orders_subtab = OrdersSubTab::Open;
        app.orders_symbol_filter = String::new();
        assert_eq!(app.filtered_orders().len(), 2);
    }

    #[test]
    fn filtered_orders_symbol_filter_prefix_match() {
        let mut app = make_test_app();
        app.orders = vec![make_order("1", "new"), make_order("2", "new")];
        app.orders[1].symbol = "AMZN".into();
        app.orders_subtab = OrdersSubTab::Open;
        app.orders_symbol_filter = "AA".to_string();
        // "AAPL" contains "AA", "AMZN" does not
        let result = app.filtered_orders();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].symbol, "AAPL");
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

    // ── push_equity_from_quotes ───────────────────────────────────────────────

    fn make_position_with_price(
        symbol: &str,
        qty: &str,
        current_price: &str,
    ) -> crate::types::Position {
        crate::types::Position {
            symbol: symbol.into(),
            qty: qty.into(),
            avg_entry_price: current_price.into(),
            current_price: current_price.into(),
            market_value: "0".into(),
            unrealized_pl: "0".into(),
            unrealized_plpc: "0".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }
    }

    #[test]
    fn push_equity_from_quotes_no_positions_is_noop() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            cash: "10000.00".into(),
            ..Default::default()
        });
        app.push_equity_from_quotes();
        assert!(app.equity_history.is_empty());
    }

    #[test]
    fn push_equity_from_quotes_uses_live_quote_ask_price() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            cash: "0.00".into(),
            ..Default::default()
        });
        app.positions = vec![make_position_with_price("AAPL", "10", "150.00")];
        app.quotes.insert(
            "AAPL".into(),
            crate::types::Quote {
                symbol: "AAPL".into(),
                ap: Some(200.00),
                bp: None,
                ..Default::default()
            },
        );
        app.push_equity_from_quotes();
        // 10 shares × $200.00 = $2000.00 → 200000 cents
        assert_eq!(app.equity_history, vec![200_000]);
    }

    #[test]
    fn push_equity_from_quotes_falls_back_to_bid_when_no_ask() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            cash: "0.00".into(),
            ..Default::default()
        });
        app.positions = vec![make_position_with_price("TSLA", "5", "300.00")];
        app.quotes.insert(
            "TSLA".into(),
            crate::types::Quote {
                symbol: "TSLA".into(),
                ap: None,
                bp: Some(250.00),
                ..Default::default()
            },
        );
        app.push_equity_from_quotes();
        // 5 × $250.00 = $1250.00 → 125000 cents
        assert_eq!(app.equity_history, vec![125_000]);
    }

    #[test]
    fn push_equity_from_quotes_falls_back_to_current_price_when_no_quote() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            cash: "0.00".into(),
            ..Default::default()
        });
        app.positions = vec![make_position_with_price("NVDA", "2", "400.00")];
        // No quote for NVDA
        app.push_equity_from_quotes();
        // 2 × $400.00 = $800.00 → 80000 cents
        assert_eq!(app.equity_history, vec![80_000]);
    }

    #[test]
    fn push_equity_from_quotes_includes_cash() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            cash: "500.00".into(),
            ..Default::default()
        });
        app.positions = vec![make_position_with_price("AAPL", "1", "100.00")];
        app.push_equity_from_quotes();
        // $500 cash + 1 × $100.00 = $600.00 → 60000 cents
        assert_eq!(app.equity_history, vec![60_000]);
    }

    #[test]
    fn push_equity_from_quotes_throttles_rapid_calls() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            cash: "0.00".into(),
            ..Default::default()
        });
        app.positions = vec![make_position_with_price("AAPL", "1", "100.00")];

        app.push_equity_from_quotes();
        assert_eq!(app.equity_history.len(), 1);

        // Immediately call again — should be suppressed by throttle
        app.push_equity_from_quotes();
        assert_eq!(
            app.equity_history.len(),
            1,
            "second immediate call should be throttled"
        );
    }

    #[test]
    fn push_equity_from_quotes_caps_at_120_entries() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            cash: "0.00".into(),
            ..Default::default()
        });
        app.positions = vec![make_position_with_price("AAPL", "1", "100.00")];

        // Bypass throttle for this test by pre-filling equity_history
        app.equity_history = vec![1u64; 120];
        // Reset throttle stamp so one more push is allowed
        app.last_equity_stream_push = None;
        app.push_equity_from_quotes();
        assert_eq!(app.equity_history.len(), 120, "should stay at 120 cap");
    }

    // ── focused_symbol ────────────────────────────────────────────────────────

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

    // ── selected_order_symbol ─────────────────────────────────────────────────

    #[test]
    fn selected_order_symbol_returns_symbol_of_selected_order() {
        let mut app = make_test_app();
        app.orders = vec![make_order("id-1", "new"), make_order("id-2", "filled")];
        app.orders_state.select(Some(0));
        // Open sub-tab — id-1 is in "new" status, so it's shown
        assert_eq!(app.selected_order_symbol(), Some("AAPL".into()));
    }

    #[test]
    fn selected_order_symbol_none_when_no_selection() {
        let mut app = make_test_app();
        app.orders = vec![make_order("id-1", "new")];
        assert_eq!(app.selected_order_symbol(), None);
    }

    // ── focused_symbol ────────────────────────────────────────────────────────

    #[test]
    fn focused_symbol_watchlist_tab_returns_selected_symbol() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = Some(make_watchlist(&["AAPL", "TSLA"]));
        app.watchlist_state.select(Some(0));
        assert_eq!(app.focused_symbol(), Some("AAPL".into()));
    }

    #[test]
    fn focused_symbol_positions_tab_returns_selected_symbol() {
        let mut app = make_test_app();
        app.active_tab = Tab::Positions;
        app.positions = vec![crate::types::Position {
            symbol: "MSFT".into(),
            qty: "5".into(),
            avg_entry_price: "300".into(),
            current_price: "310".into(),
            market_value: "1550".into(),
            unrealized_pl: "50".into(),
            unrealized_plpc: "0.032".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }];
        app.positions_state.select(Some(0));
        assert_eq!(app.focused_symbol(), Some("MSFT".into()));
    }

    #[test]
    fn focused_symbol_orders_tab_returns_selected_symbol() {
        let mut app = make_test_app();
        app.active_tab = Tab::Orders;
        app.orders = vec![make_order("id-1", "new")];
        app.orders_state.select(Some(0));
        assert_eq!(app.focused_symbol(), Some("AAPL".into()));
    }

    #[test]
    fn focused_symbol_account_tab_returns_none() {
        let app = make_test_app();
        // active_tab defaults to Account
        assert_eq!(app.focused_symbol(), None);
    }

    #[test]
    fn focused_symbol_returns_none_when_nothing_selected() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        // no selection
        assert_eq!(app.focused_symbol(), None);
    }

    // ── EquityRange ───────────────────────────────────────────────────────────

    #[test]
    fn equity_range_default_is_one_day() {
        let app = make_test_app();
        assert_eq!(app.equity_range, EquityRange::OneDay);
    }

    #[test]
    fn equity_range_cycle_one_day_to_week() {
        assert_eq!(EquityRange::OneDay.cycle(), EquityRange::OneWeek);
    }

    #[test]
    fn equity_range_cycle_week_to_month() {
        assert_eq!(EquityRange::OneWeek.cycle(), EquityRange::OneMonth);
    }

    #[test]
    fn equity_range_cycle_month_to_ytd() {
        assert_eq!(EquityRange::OneMonth.cycle(), EquityRange::Ytd);
    }

    #[test]
    fn equity_range_cycle_ytd_wraps_to_one_day() {
        assert_eq!(EquityRange::Ytd.cycle(), EquityRange::OneDay);
    }

    #[test]
    fn equity_range_label_values() {
        assert_eq!(EquityRange::OneDay.label(), "1D");
        assert_eq!(EquityRange::OneWeek.label(), "1W");
        assert_eq!(EquityRange::OneMonth.label(), "1M");
        assert_eq!(EquityRange::Ytd.label(), "YTD");
    }

    #[test]
    fn equity_range_api_params_one_day() {
        assert_eq!(EquityRange::OneDay.api_params(), ("1D", "1Min"));
    }

    #[test]
    fn equity_range_api_params_one_week() {
        assert_eq!(EquityRange::OneWeek.api_params(), ("1W", "1H"));
    }

    #[test]
    fn equity_range_api_params_one_month() {
        assert_eq!(EquityRange::OneMonth.api_params(), ("1M", "1D"));
    }

    #[test]
    fn equity_range_api_params_ytd() {
        assert_eq!(EquityRange::Ytd.api_params(), ("YTD", "1D"));
    }

    #[test]
    fn equity_range_x_labels_one_day() {
        assert_eq!(EquityRange::OneDay.x_labels(), ["09:30", "16:00"]);
    }

    #[test]
    fn equity_range_x_labels_one_week() {
        assert_eq!(EquityRange::OneWeek.x_labels(), ["Mon", "Fri"]);
    }

    #[test]
    fn equity_range_x_labels_one_month() {
        assert_eq!(EquityRange::OneMonth.x_labels(), ["Day 1", "Day 30"]);
    }

    #[test]
    fn equity_range_x_labels_ytd() {
        assert_eq!(EquityRange::Ytd.x_labels(), ["Jan", "Today"]);
    }

    #[test]
    fn push_equity_is_noop_when_range_is_not_one_day() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            equity: "1000.00".into(),
            ..Default::default()
        });
        app.equity_range = EquityRange::OneWeek;
        app.push_equity();
        assert!(
            app.equity_history.is_empty(),
            "push_equity must not append when range != OneDay"
        );
    }

    #[test]
    fn push_equity_from_quotes_is_noop_when_range_is_not_one_day() {
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            cash: "0.00".into(),
            ..Default::default()
        });
        app.positions = vec![make_position_with_price("AAPL", "10", "150.00")];
        app.equity_range = EquityRange::OneMonth;
        app.push_equity_from_quotes();
        assert!(
            app.equity_history.is_empty(),
            "push_equity_from_quotes must not append when range != OneDay"
        );
    }

    // ── SortState / PositionSortCol / OrderSortCol ───────────────────────────

    #[test]
    fn position_sort_col_cycles_through_all_variants() {
        let mut col = PositionSortCol::None;
        col = col.cycle();
        assert_eq!(col, PositionSortCol::Symbol);
        col = col.cycle();
        assert_eq!(col, PositionSortCol::Qty);
        col = col.cycle();
        assert_eq!(col, PositionSortCol::AvgCost);
        col = col.cycle();
        assert_eq!(col, PositionSortCol::MarketValue);
        col = col.cycle();
        assert_eq!(col, PositionSortCol::UnrealizedPl);
        col = col.cycle();
        assert_eq!(col, PositionSortCol::Pct);
        col = col.cycle();
        assert_eq!(col, PositionSortCol::None, "should wrap back to None");
    }

    #[test]
    fn order_sort_col_cycles_through_all_variants() {
        let mut col = OrderSortCol::None;
        col = col.cycle();
        assert_eq!(col, OrderSortCol::Symbol);
        col = col.cycle();
        assert_eq!(col, OrderSortCol::Side);
        col = col.cycle();
        assert_eq!(col, OrderSortCol::Type);
        col = col.cycle();
        assert_eq!(col, OrderSortCol::Status);
        col = col.cycle();
        assert_eq!(col, OrderSortCol::Submitted);
        col = col.cycle();
        assert_eq!(col, OrderSortCol::None, "should wrap back to None");
    }

    #[test]
    fn sort_state_toggle_dir_flips_asc_to_desc() {
        let mut state: SortState<PositionSortCol> = SortState::default();
        assert_eq!(state.dir, SortDir::Asc);
        state.toggle_dir();
        assert_eq!(state.dir, SortDir::Desc);
    }

    #[test]
    fn sort_state_toggle_dir_flips_desc_to_asc() {
        let mut state: SortState<PositionSortCol> = SortState {
            col: PositionSortCol::Symbol,
            dir: SortDir::Desc,
        };
        state.toggle_dir();
        assert_eq!(state.dir, SortDir::Asc);
    }

    #[test]
    fn sort_state_default_is_no_sort_ascending() {
        let state: SortState<PositionSortCol> = SortState::default();
        assert_eq!(state.col, PositionSortCol::None);
        assert_eq!(state.dir, SortDir::Asc);
    }

    // ── StatusMessage constructors ────────────────────────────────────────────

    #[test]
    fn status_message_with_ttl_has_text_and_expiry() {
        let msg = StatusMessage::with_ttl("hello", std::time::Duration::from_secs(5));
        assert_eq!(msg.text, "hello");
        assert!(msg.expires_at.is_some(), "with_ttl must set expires_at");
    }

    #[test]
    fn status_message_persistent_has_text_and_no_expiry() {
        let msg = StatusMessage::persistent("world");
        assert_eq!(msg.text, "world");
        assert!(
            msg.expires_at.is_none(),
            "persistent must have no expires_at"
        );
    }

    #[test]
    fn status_message_default_is_persistent_empty() {
        let msg = StatusMessage::default();
        assert_eq!(msg.text, "");
        assert!(msg.expires_at.is_none());
    }

    // ── StatusMessage PartialEq impls ─────────────────────────────────────────

    #[test]
    fn status_message_eq_str() {
        let msg = StatusMessage::persistent("hello");
        assert!(
            msg == *"hello",
            "StatusMessage should equal &str with same text"
        );
        assert!(msg != *"other");
    }

    #[test]
    fn status_message_eq_str_ref() {
        let msg = StatusMessage::persistent("hello");
        let s: &str = "hello";
        assert!(msg == s);
        assert!(msg != "world");
    }

    #[test]
    fn status_message_eq_string() {
        let msg = StatusMessage::persistent("hello");
        let s = String::from("hello");
        assert!(msg == s);
        assert!(msg != *"other");
    }

    #[test]
    fn status_message_eq_status_message() {
        let a = StatusMessage::persistent("hello");
        let b = StatusMessage::with_ttl("hello", std::time::Duration::from_secs(1));
        // PartialEq for StatusMessage compares text only, not expires_at.
        assert!(
            a == b,
            "two StatusMessages with same text should be equal regardless of TTL"
        );
        let c = StatusMessage::persistent("other");
        assert!(a != c);
    }
}
