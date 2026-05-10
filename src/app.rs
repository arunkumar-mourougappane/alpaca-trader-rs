use std::collections::HashMap;
use std::sync::Arc;

use ratatui::widgets::TableState;
use tokio::sync::Notify;

use crate::config::AlpacaConfig;
use crate::types::{AccountInfo, MarketClock, Order, Position, Quote, Watchlist};

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
    Submit,
}

impl OrderField {
    pub fn next(&self) -> Self {
        match self {
            OrderField::Symbol => OrderField::Side,
            OrderField::Side => OrderField::OrderType,
            OrderField::OrderType => OrderField::Qty,
            OrderField::Qty => OrderField::Price,
            OrderField::Price => OrderField::Submit,
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
            OrderField::Submit => OrderField::Price,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderEntryState {
    pub symbol: String,
    pub side_buy: bool,     // true = BUY, false = SELL
    pub market_order: bool, // true = MARKET, false = LIMIT
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

pub struct App {
    pub config: AlpacaConfig,
    pub refresh_notify: Arc<Notify>,

    pub account: Option<AccountInfo>,
    pub positions: Vec<Position>,
    pub orders: Vec<Order>,
    pub quotes: HashMap<String, Quote>,
    pub watchlist: Option<Watchlist>,
    pub clock: Option<MarketClock>,
    pub equity_history: Vec<u64>,

    pub active_tab: Tab,
    pub watchlist_state: TableState,
    pub positions_state: TableState,
    pub orders_state: TableState,
    pub orders_subtab: OrdersSubTab,

    pub modal: Option<Modal>,
    pub search_query: String,
    pub searching: bool,

    pub status_msg: String,
    pub should_quit: bool,
}

impl App {
    pub fn new(config: AlpacaConfig, refresh_notify: Arc<Notify>) -> Self {
        Self {
            config,
            refresh_notify,
            account: None,
            positions: Vec::new(),
            orders: Vec::new(),
            quotes: HashMap::new(),
            watchlist: None,
            clock: None,
            equity_history: Vec::new(),
            active_tab: Tab::Account,
            watchlist_state: TableState::default(),
            positions_state: TableState::default(),
            orders_state: TableState::default(),
            orders_subtab: OrdersSubTab::Open,
            modal: None,
            search_query: String::new(),
            searching: false,
            status_msg: String::from("Loading…"),
            should_quit: false,
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
