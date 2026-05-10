use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, Modal, Tab};
use crate::events::Event;
use crate::input::{
    handle_modal_key, handle_mouse, handle_orders_key, handle_positions_key, handle_search_key,
    handle_watchlist_key,
};

pub fn update(app: &mut App, event: Event) {
    match event {
        Event::Input(key) => handle_key(app, key),
        Event::Mouse(m) => handle_mouse(app, m),
        Event::Resize(_, _) => {}

        Event::AccountUpdated(a) => {
            app.account = Some(a);
            app.push_equity();
        }
        Event::PositionsUpdated(p) => {
            app.positions = p;
            if app.positions_state.selected().is_none() && !app.positions.is_empty() {
                app.positions_state.select(Some(0));
            }
        }
        Event::OrdersUpdated(o) => {
            app.orders = o;
            if app.orders_state.selected().is_none() && !app.orders.is_empty() {
                app.orders_state.select(Some(0));
            }
        }
        Event::ClockUpdated(c) => app.clock = Some(c),
        Event::WatchlistUpdated(w) => {
            // Push new symbol list to the market stream for resubscription
            let symbols: Vec<String> = w.assets.iter().map(|a| a.symbol.clone()).collect();
            let _ = app.symbol_tx.send(symbols);
            if app.watchlist_state.selected().is_none() && !w.assets.is_empty() {
                app.watchlist_state.select(Some(0));
            }
            app.watchlist = Some(w);
        }
        Event::MarketQuote(q) => {
            app.quotes.insert(q.symbol.clone(), q);
        }
        Event::TradeUpdate(o) => {
            if let Some(existing) = app.orders.iter_mut().find(|x| x.id == o.id) {
                *existing = o;
            } else {
                app.orders.insert(0, o);
            }
        }
        Event::StatusMsg(msg) => app.status_msg = msg,
        Event::Tick => {}
        Event::Quit => app.should_quit = true,
    }
}

fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) {
    // Modal takes full priority
    if app.modal.is_some() {
        handle_modal_key(app, key);
        return;
    }

    // Search mode intercepts printable keys
    if app.searching {
        handle_search_key(app, key);
        return;
    }

    // Global shortcuts
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true
        }
        KeyCode::Char('?') => app.modal = Some(Modal::Help),
        // '1'/'2'/'3' switch panels globally, but yield to the Orders panel so those
        // keys can switch sub-tabs (Open / Filled / Cancelled) when Orders is active.
        KeyCode::Char('1') if app.active_tab != Tab::Orders => app.active_tab = Tab::Account,
        KeyCode::Char('2') if app.active_tab != Tab::Orders => app.active_tab = Tab::Watchlist,
        KeyCode::Char('3') if app.active_tab != Tab::Orders => app.active_tab = Tab::Positions,
        KeyCode::Char('4') => app.active_tab = Tab::Orders,
        KeyCode::Tab => app.active_tab = app.active_tab.next(),
        KeyCode::BackTab => app.active_tab = app.active_tab.prev(),
        KeyCode::Char('r') => {
            app.status_msg = "Refreshing…".into();
            app.refresh_notify.notify_one();
        }
        _ => handle_panel_key(app, key),
    }
}

fn handle_panel_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match app.active_tab.clone() {
        Tab::Account => {}
        Tab::Watchlist => handle_watchlist_key(app, key),
        Tab::Positions => handle_positions_key(app, key),
        Tab::Orders => handle_orders_key(app, key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_helpers::*;
    use crate::app::{Modal, OrdersSubTab, Tab};
    use crate::commands::Command;
    use crate::events::Event;
    use crate::types::{AccountInfo, MarketClock, Order, Quote};
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::layout::Rect;

    fn key(code: KeyCode) -> Event {
        Event::Input(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ctrl(code: KeyCode) -> Event {
        Event::Input(KeyEvent::new(code, KeyModifiers::CONTROL))
    }

    fn mouse_click(col: u16, row: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn mouse_move(col: u16, row: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn rect(x: u16, y: u16, w: u16, h: u16) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    // ── Data events ───────────────────────────────────────────────────────────

    #[test]
    fn account_updated_sets_account_and_pushes_equity() {
        let mut app = make_test_app();
        let acc = AccountInfo {
            equity: "500".into(),
            status: "ACTIVE".into(),
            ..Default::default()
        };
        update(&mut app, Event::AccountUpdated(acc));
        assert!(app.account.is_some());
        assert_eq!(app.equity_history.len(), 1);
    }

    #[test]
    fn positions_updated_empty_does_not_select() {
        let mut app = make_test_app();
        update(&mut app, Event::PositionsUpdated(vec![]));
        assert_eq!(app.positions_state.selected(), None);
    }

    #[test]
    fn positions_updated_non_empty_auto_selects_zero() {
        let mut app = make_test_app();
        let pos = vec![crate::types::Position {
            symbol: "AAPL".into(),
            qty: "10".into(),
            avg_entry_price: "100".into(),
            current_price: "110".into(),
            market_value: "1100".into(),
            unrealized_pl: "100".into(),
            unrealized_plpc: "0.1".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }];
        update(&mut app, Event::PositionsUpdated(pos));
        assert_eq!(app.positions_state.selected(), Some(0));
    }

    #[test]
    fn orders_updated_non_empty_auto_selects_zero() {
        let mut app = make_test_app();
        let orders = vec![make_order("o1", "accepted")];
        update(&mut app, Event::OrdersUpdated(orders));
        assert_eq!(app.orders_state.selected(), Some(0));
    }

    #[test]
    fn watchlist_updated_auto_selects_zero() {
        let mut app = make_test_app();
        let wl = make_watchlist(&["AAPL", "TSLA"]);
        update(&mut app, Event::WatchlistUpdated(wl));
        assert!(app.watchlist.is_some());
        assert_eq!(app.watchlist_state.selected(), Some(0));
    }

    #[test]
    fn trade_update_existing_replaces_in_place() {
        let mut app = make_test_app();
        app.orders = vec![make_order("o1", "accepted")];
        let updated = Order {
            id: "o1".into(),
            status: "filled".into(),
            ..make_order("o1", "filled")
        };
        update(&mut app, Event::TradeUpdate(updated));
        assert_eq!(app.orders.len(), 1);
        assert_eq!(app.orders[0].status, "filled");
    }

    #[test]
    fn trade_update_new_id_prepends() {
        let mut app = make_test_app();
        app.orders = vec![make_order("o1", "accepted")];
        update(&mut app, Event::TradeUpdate(make_order("o2", "accepted")));
        assert_eq!(app.orders.len(), 2);
        assert_eq!(app.orders[0].id, "o2");
    }

    #[test]
    fn market_quote_inserted() {
        let mut app = make_test_app();
        let q = Quote {
            symbol: "AAPL".into(),
            ap: Some(185.0),
            bp: Some(184.9),
            ..Default::default()
        };
        update(&mut app, Event::MarketQuote(q));
        assert!(app.quotes.contains_key("AAPL"));
        assert_eq!(app.quotes["AAPL"].ap, Some(185.0));
    }

    #[test]
    fn clock_updated() {
        let mut app = make_test_app();
        let clock = MarketClock {
            is_open: true,
            ..Default::default()
        };
        update(&mut app, Event::ClockUpdated(clock));
        assert!(app.clock.as_ref().unwrap().is_open);
    }

    #[test]
    fn status_msg_updated() {
        let mut app = make_test_app();
        update(&mut app, Event::StatusMsg("hello".into()));
        assert_eq!(app.status_msg, "hello");
    }

    #[test]
    fn quit_event_sets_flag() {
        let mut app = make_test_app();
        update(&mut app, Event::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn tick_is_noop() {
        let mut app = make_test_app();
        let before_status = app.status_msg.clone();
        update(&mut app, Event::Tick);
        assert!(!app.should_quit);
        assert_eq!(app.status_msg, before_status);
    }

    // ── Global key events ─────────────────────────────────────────────────────

    #[test]
    fn key_q_quits() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn key_ctrl_c_quits() {
        let mut app = make_test_app();
        update(&mut app, ctrl(KeyCode::Char('c')));
        assert!(app.should_quit);
    }

    #[test]
    fn key_question_mark_opens_help() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Char('?')));
        assert!(matches!(app.modal, Some(Modal::Help)));
    }

    #[test]
    fn key_1_switches_to_account() {
        let mut app = make_test_app();
        app.active_tab = Tab::Positions; // not Orders, so '1' switches tab
        update(&mut app, key(KeyCode::Char('1')));
        assert_eq!(app.active_tab, Tab::Account);
    }

    #[test]
    fn key_4_switches_to_orders() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Char('4')));
        assert_eq!(app.active_tab, Tab::Orders);
    }

    #[test]
    fn key_tab_cycles_forward() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Tab));
        assert_eq!(app.active_tab, Tab::Watchlist);
    }

    #[test]
    fn key_backtab_cycles_backward() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::BackTab));
        assert_eq!(app.active_tab, Tab::Orders);
    }

    #[test]
    fn key_esc_closes_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Help);
        update(&mut app, key(KeyCode::Esc));
        assert!(app.modal.is_none());
    }

    #[test]
    fn key_r_sets_refreshing_status() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Char('r')));
        assert_eq!(app.status_msg, "Refreshing…");
    }

    // ── Watchlist panel keys ──────────────────────────────────────────────────

    fn watchlist_app() -> App {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = Some(make_watchlist(&["AAPL", "TSLA", "NVDA"]));
        app.watchlist_state.select(Some(0));
        app
    }

    #[test]
    fn watchlist_j_moves_down() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.watchlist_state.selected(), Some(1));
    }

    #[test]
    fn watchlist_j_clamps_at_end() {
        let mut app = watchlist_app();
        app.watchlist_state.select(Some(2)); // last row
        update(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.watchlist_state.selected(), Some(2));
    }

    #[test]
    fn watchlist_k_moves_up() {
        let mut app = watchlist_app();
        app.watchlist_state.select(Some(2));
        update(&mut app, key(KeyCode::Char('k')));
        assert_eq!(app.watchlist_state.selected(), Some(1));
    }

    #[test]
    fn watchlist_k_clamps_at_zero() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('k')));
        assert_eq!(app.watchlist_state.selected(), Some(0));
    }

    #[test]
    fn watchlist_g_jumps_to_top() {
        let mut app = watchlist_app();
        app.watchlist_state.select(Some(2));
        update(&mut app, key(KeyCode::Char('g')));
        assert_eq!(app.watchlist_state.selected(), Some(0));
    }

    #[test]
    #[allow(non_snake_case)]
    fn watchlist_G_jumps_to_bottom() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('G')));
        assert_eq!(app.watchlist_state.selected(), Some(2));
    }

    #[test]
    fn watchlist_enter_opens_symbol_detail() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Enter));
        assert!(matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "AAPL"));
    }

    #[test]
    fn watchlist_o_opens_order_entry_with_symbol() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('o')));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "AAPL"));
    }

    #[test]
    fn watchlist_a_opens_add_symbol() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('a')));
        assert!(matches!(&app.modal, Some(Modal::AddSymbol { .. })));
    }

    #[test]
    fn watchlist_d_opens_confirm() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('d')));
        assert!(matches!(&app.modal, Some(Modal::Confirm { .. })));
    }

    #[test]
    fn watchlist_slash_starts_search() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('/')));
        assert!(app.searching);
    }

    // ── Orders panel keys ─────────────────────────────────────────────────────

    fn orders_app() -> App {
        let mut app = make_test_app();
        app.active_tab = Tab::Orders;
        app.orders = vec![make_order("o1", "accepted"), make_order("o2", "accepted")];
        app.orders_state.select(Some(0));
        app
    }

    #[test]
    fn orders_key_1_switches_to_open_subtab() {
        let mut app = orders_app();
        app.orders_subtab = OrdersSubTab::Filled;
        update(&mut app, key(KeyCode::Char('1')));
        assert_eq!(app.orders_subtab, OrdersSubTab::Open);
        assert_eq!(app.active_tab, Tab::Orders); // tab must NOT change
    }

    #[test]
    fn orders_key_2_switches_to_filled_subtab() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Char('2')));
        assert_eq!(app.orders_subtab, OrdersSubTab::Filled);
        assert_eq!(app.active_tab, Tab::Orders);
    }

    #[test]
    fn orders_key_3_switches_to_cancelled_subtab() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Char('3')));
        assert_eq!(app.orders_subtab, OrdersSubTab::Cancelled);
        assert_eq!(app.active_tab, Tab::Orders);
    }

    #[test]
    fn key_1_from_other_panels_still_switches_tab() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        update(&mut app, key(KeyCode::Char('1')));
        assert_eq!(app.active_tab, Tab::Account);
    }

    #[test]
    fn orders_c_opens_confirm_for_selected() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Char('c')));
        assert!(matches!(&app.modal, Some(Modal::Confirm { .. })));
    }

    #[test]
    fn orders_o_opens_blank_order_entry() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Char('o')));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol.is_empty()));
    }

    // ── Modal keys ────────────────────────────────────────────────────────────

    #[test]
    fn modal_tab_advances_focused_field() {
        use crate::app::{OrderEntryState, OrderField};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new(String::new());
        state.focused_field = OrderField::Qty;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Tab));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.focused_field == OrderField::Price)
        );
    }

    #[test]
    fn modal_char_appends_to_symbol_field() {
        use crate::app::{OrderEntryState, OrderField};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new(String::new());
        state.focused_field = OrderField::Symbol;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Char('A')));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "A"));
    }

    #[test]
    fn modal_digit_appends_to_qty_field() {
        use crate::app::{OrderEntryState, OrderField};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new(String::new());
        state.focused_field = OrderField::Qty;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Char('5')));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.qty_input == "5"));
    }

    #[test]
    fn modal_non_digit_ignored_in_qty_field() {
        use crate::app::{OrderEntryState, OrderField};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new(String::new());
        state.focused_field = OrderField::Qty;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Char('x')));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.qty_input.is_empty()));
    }

    #[test]
    fn modal_backspace_removes_last_char_from_symbol() {
        use crate::app::{OrderEntryState, OrderField};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new("AB".into());
        state.focused_field = OrderField::Symbol;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Backspace));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "A"));
    }

    // ── Search keys ───────────────────────────────────────────────────────────

    #[test]
    fn search_char_appends_to_query() {
        let mut app = watchlist_app();
        app.searching = true;
        update(&mut app, key(KeyCode::Char('A')));
        assert_eq!(app.search_query, "A");
    }

    #[test]
    fn search_esc_exits_search_mode() {
        let mut app = watchlist_app();
        app.searching = true;
        update(&mut app, key(KeyCode::Esc));
        assert!(!app.searching);
    }

    #[test]
    fn search_enter_exits_search_mode() {
        let mut app = watchlist_app();
        app.searching = true;
        update(&mut app, key(KeyCode::Enter));
        assert!(!app.searching);
    }

    // ── Phase 2: command sends ────────────────────────────────────────────────

    fn app_with_rx() -> (App, tokio::sync::mpsc::Receiver<Command>) {
        use crate::config::{AlpacaConfig, AlpacaEnv};
        let (command_tx, command_rx) = tokio::sync::mpsc::channel(16);
        let (symbol_tx, _) = tokio::sync::watch::channel(vec![]);
        let app = App::new(
            AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: AlpacaEnv::Paper,
            },
            std::sync::Arc::new(tokio::sync::Notify::new()),
            command_tx,
            symbol_tx,
        );
        (app, command_rx)
    }

    #[test]
    fn order_entry_submit_sends_submit_order_command() {
        use crate::app::{OrderEntryState, OrderField};
        use crate::types::AccountInfo;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Submit;
        state.qty_input = "10".into();
        state.price_input = "185.00".into();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_none(), "modal should close after submit");
        assert_eq!(app.status_msg, "Submitting order…");
        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::SubmitOrder { symbol, .. } if symbol == "AAPL"),
            "expected SubmitOrder for AAPL"
        );
    }

    #[test]
    fn order_entry_submit_market_order_omits_price() {
        use crate::app::{OrderEntryState, OrderField};
        use crate::types::AccountInfo;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let mut state = OrderEntryState::new("TSLA".into());
        state.focused_field = OrderField::Submit;
        state.market_order = true;
        state.qty_input = "5".into();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::SubmitOrder { order_type, price: None, .. }
                if order_type == "market"),
            "market order should have no price"
        );
    }

    #[test]
    fn confirm_cancel_order_sends_cancel_command() {
        use crate::app::ConfirmAction;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.active_tab = Tab::Orders;
        app.modal = Some(Modal::Confirm {
            message: "Cancel?".into(),
            action: ConfirmAction::CancelOrder("order-xyz".into()),
            confirmed: false,
        });

        update(&mut app, key(KeyCode::Char('y')));

        assert!(app.modal.is_none());
        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::CancelOrder(id) if id == "order-xyz"),
            "expected CancelOrder command"
        );
    }

    #[test]
    fn confirm_remove_watchlist_sends_remove_command() {
        use crate::app::ConfirmAction;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::Confirm {
            message: "Remove?".into(),
            action: ConfirmAction::RemoveFromWatchlist {
                watchlist_id: "wl-id".into(),
                symbol: "TLRY".into(),
            },
            confirmed: false,
        });

        update(&mut app, key(KeyCode::Char('y')));

        assert!(app.modal.is_none());
        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::RemoveFromWatchlist { symbol, .. } if symbol == "TLRY"),
            "expected RemoveFromWatchlist command"
        );
    }

    #[test]
    fn add_symbol_enter_sends_add_command() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::AddSymbol {
            input: "NVDA".into(),
            watchlist_id: "wl-id".into(),
        });

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_none());
        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::AddToWatchlist { symbol, .. } if symbol == "NVDA"),
            "expected AddToWatchlist command"
        );
    }

    #[test]
    fn add_symbol_empty_input_sends_no_command() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::AddSymbol {
            input: String::new(),
            watchlist_id: "wl-id".into(),
        });

        update(&mut app, key(KeyCode::Enter));

        assert!(cmd_rx.try_recv().is_err(), "no command for empty input");
    }

    #[test]
    fn watchlist_updated_pushes_symbols_to_symbol_tx() {
        use crate::config::{AlpacaConfig, AlpacaEnv};
        use tokio::sync::watch;
        let (command_tx, _) = tokio::sync::mpsc::channel(1);
        let (symbol_tx, symbol_rx) = watch::channel(vec![]);
        let mut app = App::new(
            AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: AlpacaEnv::Paper,
            },
            std::sync::Arc::new(tokio::sync::Notify::new()),
            command_tx,
            symbol_tx,
        );

        let wl = make_watchlist(&["AAPL", "TSLA", "NVDA"]);
        update(&mut app, Event::WatchlistUpdated(wl));

        assert!(
            symbol_rx.has_changed().unwrap_or(false),
            "symbol_tx should have been updated"
        );
        let symbols = symbol_rx.borrow().clone();
        assert_eq!(symbols, vec!["AAPL", "TSLA", "NVDA"]);
    }

    // ── send_command error path: channel full / closed (regression for #7) ────

    fn app_with_capacity(cap: usize) -> (App, tokio::sync::mpsc::Receiver<Command>) {
        use crate::config::{AlpacaConfig, AlpacaEnv};
        let (command_tx, command_rx) = tokio::sync::mpsc::channel(cap);
        let (symbol_tx, _) = tokio::sync::watch::channel(vec![]);
        let app = App::new(
            AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: AlpacaEnv::Paper,
            },
            std::sync::Arc::new(tokio::sync::Notify::new()),
            command_tx,
            symbol_tx,
        );
        (app, command_rx)
    }

    #[test]
    fn channel_full_sets_busy_status_msg() {
        // Channel capacity 1, pre-fill it so the next try_send hits Full.
        let (mut app, _rx) = app_with_capacity(1);
        // Fill the channel
        let _ = app
            .command_tx
            .try_send(Command::CancelOrder("dummy".into()));

        // Now trigger another command via update() — should hit TrySendError::Full
        use crate::app::{OrderEntryState, OrderField};
        use crate::types::AccountInfo;
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Submit;
        state.qty_input = "1".into();
        state.price_input = "100.00".into();
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Enter));

        assert_eq!(
            app.status_msg, "System busy — please retry",
            "full channel should show busy message"
        );
    }

    #[test]
    fn mouse_click_tab_bar_switches_tab() {
        let (mut app, _rx) = app_with_capacity(4);
        // Tab bar at row 2, full width 80.
        // Actual ratatui Tabs layout (` label ` + `|` divider):
        //   Tab 0 "1:Account"   (len 9): cols  0..10  (width 11)
        //   divider `|`:               col  11
        //   Tab 1 "2:Watchlist" (len 11): cols 12..24 (width 13)
        //   divider `|`:               col  25
        //   Tab 2 "3:Positions" (len 11): cols 26..38 (width 13)
        //   divider `|`:               col  39
        //   Tab 3 "4:Orders"    (len  8): cols 40..49 (width 10)
        app.hit_areas.tab_bar = rect(0, 2, 80, 1);
        assert_eq!(app.active_tab, Tab::Account);

        // Click inside "2:Watchlist" (col 18 is within 12..24)
        update(&mut app, mouse_click(18, 2));
        assert_eq!(app.active_tab, Tab::Watchlist);

        // Click inside "3:Positions" (col 32 is within 26..38)
        update(&mut app, mouse_click(32, 2));
        assert_eq!(app.active_tab, Tab::Positions);

        // Click inside "4:Orders" (col 45 is within 40..49)
        update(&mut app, mouse_click(45, 2));
        assert_eq!(app.active_tab, Tab::Orders);

        // Click back on "1:Account" (col 5 is within 0..10)
        update(&mut app, mouse_click(5, 2));
        assert_eq!(app.active_tab, Tab::Account);
    }

    #[test]
    fn mouse_non_left_click_ignored() {
        let (mut app, _rx) = app_with_capacity(4);
        app.hit_areas.tab_bar = rect(0, 2, 80, 1);
        app.active_tab = Tab::Watchlist;
        // MouseMove event — must not switch tab
        update(&mut app, mouse_move(25, 2));
        assert_eq!(app.active_tab, Tab::Watchlist);
    }

    #[test]
    fn mouse_click_orders_subtab() {
        let (mut app, _rx) = app_with_capacity(4);
        app.active_tab = Tab::Orders;
        // Simulate per-subtab rects as rendered from actual label widths (0 orders each):
        //   "1:Open (0)"      → len=10 → width=12, x=0
        //   "2:Filled (0)"    → len=12 → width=14, x=13 (12 + 1 divider)
        //   "3:Cancelled (0)" → len=15 → width=17, x=28 (13 + 14 + 1 divider)
        app.hit_areas.orders_subtab_rects = vec![
            rect(0, 5, 12, 1),  // Open
            rect(13, 5, 14, 1), // Filled
            rect(28, 5, 17, 1), // Cancelled
        ];
        assert_eq!(app.orders_subtab, OrdersSubTab::Open);

        // Click within Filled subtab rect
        update(&mut app, mouse_click(13, 5));
        assert_eq!(app.orders_subtab, OrdersSubTab::Filled);

        // Click within Cancelled subtab rect
        update(&mut app, mouse_click(28, 5));
        assert_eq!(app.orders_subtab, OrdersSubTab::Cancelled);

        // Click back within Open subtab rect
        update(&mut app, mouse_click(0, 5));
        assert_eq!(app.orders_subtab, OrdersSubTab::Open);
    }

    #[test]
    fn mouse_click_list_row_selects_item() {
        use crate::types::{Asset, Watchlist};

        let (mut app, _rx) = app_with_capacity(4);
        let make_asset = |sym: &str| Asset {
            id: sym.to_string(),
            symbol: sym.to_string(),
            name: sym.to_string(),
            exchange: "NASDAQ".to_string(),
            asset_class: "us_equity".to_string(),
            tradable: true,
            shortable: false,
            fractionable: false,
            easy_to_borrow: false,
        };
        app.watchlist = Some(Watchlist {
            id: "wl1".to_string(),
            name: "Test".to_string(),
            assets: vec![make_asset("AAPL"), make_asset("GOOG")],
        });
        app.watchlist_state.select(Some(0));
        app.active_tab = Tab::Watchlist;

        // List data starts at row 10 (border + header already accounted for)
        app.hit_areas.list_data_start_y = 10;

        // Click on row 11 → data_row 1 → idx 1 → selects second asset
        update(&mut app, mouse_click(5, 11));
        assert_eq!(app.watchlist_state.selected(), Some(1));
    }

    #[test]
    fn channel_closed_sets_stopped_status_msg() {
        // Drop the receiver to close the channel.
        let (mut app, rx) = app_with_capacity(8);
        drop(rx);

        use crate::app::{OrderEntryState, OrderField};
        use crate::types::AccountInfo;
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Submit;
        state.qty_input = "1".into();
        state.price_input = "100.00".into();
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Enter));

        assert_eq!(
            app.status_msg, "Command handler stopped — restart app",
            "closed channel should show stopped message"
        );
    }

    // ── Validation gate tests ─────────────────────────────────────────────────

    fn order_entry_submit_state(symbol: &str) -> crate::app::OrderEntryState {
        use crate::app::{OrderEntryState, OrderField};
        let mut s = OrderEntryState::new(symbol.into());
        s.focused_field = OrderField::Submit;
        s.market_order = false;
        s.qty_input = "10".into();
        s.price_input = "100.00".into();
        s
    }

    #[test]
    fn validation_empty_symbol_keeps_modal_open() {
        let (mut app, _rx) = app_with_capacity(4);
        let mut state = order_entry_submit_state("");
        state.symbol.clear();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(
            app.modal.is_some(),
            "modal must stay open on validation failure"
        );
        assert!(
            !app.status_msg.is_empty(),
            "status_msg must contain error text"
        );
    }

    #[test]
    fn validation_zero_qty_keeps_modal_open() {
        let (mut app, _rx) = app_with_capacity(4);
        let mut state = order_entry_submit_state("AAPL");
        state.qty_input = "0".into();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_some());
        assert!(!app.status_msg.is_empty());
    }

    #[test]
    fn validation_non_numeric_price_on_limit_keeps_modal_open() {
        let (mut app, _rx) = app_with_capacity(4);
        let mut state = order_entry_submit_state("AAPL");
        state.price_input = "bad".into();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_some());
        assert!(!app.status_msg.is_empty());
    }

    #[test]
    fn validation_exceeds_buying_power_keeps_modal_open() {
        use crate::types::AccountInfo;
        let (mut app, _rx) = app_with_capacity(4);
        app.account = Some(AccountInfo {
            buying_power: "500".into(), // 10 × 100 = 1000 > 500
            ..Default::default()
        });
        let state = order_entry_submit_state("AAPL");
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_some());
        assert!(!app.status_msg.is_empty());
    }

    #[test]
    fn validation_pass_sends_command_and_closes_modal() {
        use crate::types::AccountInfo;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let state = order_entry_submit_state("AAPL");
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_none(), "modal should close on valid submit");
        cmd_rx.try_recv().expect("command should be sent");
    }
}
