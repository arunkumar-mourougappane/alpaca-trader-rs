use std::time::Instant;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, Modal, StatusMessage, Tab};
use crate::events::{Event, StreamKind};
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
        Event::StatusMsg(msg) => app.status_msg = StatusMessage::persistent(msg),
        Event::StreamConnected(kind) => match kind {
            StreamKind::Market => app.market_stream_ok = true,
            StreamKind::Account => app.account_stream_ok = true,
        },
        Event::StreamDisconnected(kind) => match kind {
            StreamKind::Market => app.market_stream_ok = false,
            StreamKind::Account => app.account_stream_ok = false,
        },
        Event::PortfolioHistoryLoaded(data) => {
            // Convert dollar values → cents (u64) to match equity_history format.
            // Keep all samples; ongoing push_equity() calls will append new ones.
            app.equity_history = data.into_iter().map(|v| (v * 100.0) as u64).collect();
        }
        Event::SnapshotsUpdated(snapshots) => {
            app.snapshots = snapshots;
        }
        Event::IntradayBarsReceived { symbol, bars } => {
            app.intraday_bars.insert(symbol, bars);
        }
        Event::Tick => {
            if let Some(exp) = app.status_msg.expires_at {
                if exp <= Instant::now() {
                    app.status_msg.clear();
                }
            }
        }
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
            app.status_msg = StatusMessage::transient("Refreshing…");
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
    use crate::events::{Event, StreamKind};
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
    fn stream_connected_market_sets_flag() {
        let mut app = make_test_app();
        assert!(!app.market_stream_ok);
        update(&mut app, Event::StreamConnected(StreamKind::Market));
        assert!(app.market_stream_ok);
        assert!(
            !app.account_stream_ok,
            "account flag must remain unaffected"
        );
    }

    #[test]
    fn stream_connected_account_sets_flag() {
        let mut app = make_test_app();
        update(&mut app, Event::StreamConnected(StreamKind::Account));
        assert!(app.account_stream_ok);
        assert!(!app.market_stream_ok, "market flag must remain unaffected");
    }

    #[test]
    fn stream_disconnected_market_clears_flag() {
        let mut app = make_test_app();
        app.market_stream_ok = true;
        app.account_stream_ok = true;
        update(&mut app, Event::StreamDisconnected(StreamKind::Market));
        assert!(!app.market_stream_ok);
        assert!(app.account_stream_ok, "account flag must remain unaffected");
    }

    #[test]
    fn stream_disconnected_account_clears_flag() {
        let mut app = make_test_app();
        app.market_stream_ok = true;
        app.account_stream_ok = true;
        update(&mut app, Event::StreamDisconnected(StreamKind::Account));
        assert!(!app.account_stream_ok);
        assert!(app.market_stream_ok, "market flag must remain unaffected");
    }

    #[test]
    fn stream_connected_then_disconnected_roundtrip() {
        let mut app = make_test_app();
        update(&mut app, Event::StreamConnected(StreamKind::Market));
        update(&mut app, Event::StreamConnected(StreamKind::Account));
        assert!(app.market_stream_ok && app.account_stream_ok);
        update(&mut app, Event::StreamDisconnected(StreamKind::Market));
        assert!(!app.market_stream_ok);
        assert!(app.account_stream_ok);
    }

    #[test]
    fn quit_event_sets_flag() {
        let mut app = make_test_app();
        update(&mut app, Event::Quit);
        assert!(app.should_quit);
    }

    // ── PortfolioHistoryLoaded ────────────────────────────────────────────────

    #[test]
    fn portfolio_history_loaded_replaces_equity_history() {
        let mut app = make_test_app();
        let data = vec![1000.0_f64, 1001.5, 1002.0];
        update(&mut app, Event::PortfolioHistoryLoaded(data));
        assert_eq!(app.equity_history, vec![100000, 100150, 100200]);
    }

    #[test]
    fn portfolio_history_loaded_overwrites_existing_history() {
        let mut app = make_test_app();
        app.equity_history = vec![99999];
        let data = vec![500.0_f64, 600.0];
        update(&mut app, Event::PortfolioHistoryLoaded(data));
        assert_eq!(app.equity_history, vec![50000, 60000]);
    }

    #[test]
    fn portfolio_history_loaded_empty_vec_clears_history() {
        let mut app = make_test_app();
        app.equity_history = vec![12345];
        update(&mut app, Event::PortfolioHistoryLoaded(vec![]));
        assert!(app.equity_history.is_empty());
    }

    #[test]
    fn portfolio_history_loaded_preserves_all_samples() {
        let mut app = make_test_app();
        let data: Vec<f64> = (0..390).map(|i| 100.0 + i as f64 * 0.1).collect();
        update(&mut app, Event::PortfolioHistoryLoaded(data));
        assert_eq!(
            app.equity_history.len(),
            390,
            "all intraday samples should be kept"
        );
    }

    #[test]
    fn tick_is_noop() {
        let mut app = make_test_app();
        let before_status = app.status_msg.clone();
        update(&mut app, Event::Tick);
        assert!(!app.should_quit);
        assert_eq!(app.status_msg, before_status);
    }

    #[test]
    fn tick_clears_expired_transient_status_msg() {
        use std::time::{Duration, Instant};
        let mut app = make_test_app();
        // Set an already-expired transient message.
        app.status_msg = crate::app::StatusMessage {
            text: "Order submitted".into(),
            expires_at: Some(Instant::now() - Duration::from_secs(1)),
        };
        update(&mut app, Event::Tick);
        assert!(
            app.status_msg.is_empty(),
            "expired transient message should be cleared"
        );
    }

    #[test]
    fn tick_does_not_clear_unexpired_transient_status_msg() {
        use std::time::{Duration, Instant};
        let mut app = make_test_app();
        // Set a transient message that expires in the far future.
        app.status_msg = crate::app::StatusMessage {
            text: "Refreshing…".into(),
            expires_at: Some(Instant::now() + Duration::from_secs(60)),
        };
        update(&mut app, Event::Tick);
        assert_eq!(
            app.status_msg, "Refreshing…",
            "non-expired message must not be cleared"
        );
    }

    #[test]
    fn tick_does_not_clear_persistent_status_msg() {
        let mut app = make_test_app();
        app.status_msg = crate::app::StatusMessage::persistent("Loading…");
        update(&mut app, Event::Tick);
        assert_eq!(
            app.status_msg, "Loading…",
            "persistent message must survive tick"
        );
    }

    #[test]
    fn status_msg_transient_has_expiry() {
        let msg = crate::app::StatusMessage::transient("Submitting order…");
        assert!(!msg.text.is_empty());
        assert!(
            msg.expires_at.is_some(),
            "transient message must have an expiry"
        );
    }

    #[test]
    fn status_msg_persistent_has_no_expiry() {
        let msg = crate::app::StatusMessage::persistent("Error: unauthorized");
        assert!(
            msg.expires_at.is_none(),
            "persistent message must have no expiry"
        );
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

    // ── IntradayBarsReceived ──────────────────────────────────────────────────

    #[test]
    fn intraday_bars_received_stores_bars() {
        let (mut app, _rx) = app_with_rx();
        update(
            &mut app,
            Event::IntradayBarsReceived {
                symbol: "AAPL".into(),
                bars: vec![14200, 14215, 14198],
            },
        );
        assert_eq!(
            app.intraday_bars.get("AAPL"),
            Some(&vec![14200u64, 14215, 14198])
        );
    }

    #[test]
    fn intraday_bars_received_overwrites_existing_bars() {
        let (mut app, _rx) = app_with_rx();
        app.intraday_bars.insert("AAPL".into(), vec![100, 200]);
        update(
            &mut app,
            Event::IntradayBarsReceived {
                symbol: "AAPL".into(),
                bars: vec![300, 400, 500],
            },
        );
        assert_eq!(app.intraday_bars.get("AAPL"), Some(&vec![300u64, 400, 500]));
    }

    // ── SymbolDetail modal key handling ───────────────────────────────────────

    #[test]
    fn symbol_detail_o_opens_order_entry() {
        let (mut app, _rx) = app_with_rx();
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        update(&mut app, key(KeyCode::Char('o')));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "AAPL" && s.side == crate::app::OrderSide::Buy),
            "o key should open buy order entry for symbol"
        );
    }

    #[test]
    fn symbol_detail_s_opens_sell_order_entry() {
        let (mut app, _rx) = app_with_rx();
        app.modal = Some(Modal::SymbolDetail("NVDA".into()));
        update(&mut app, key(KeyCode::Char('s')));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "NVDA" && s.side == crate::app::OrderSide::Sell),
            "s key should open sell order entry for symbol"
        );
    }

    #[test]
    fn symbol_detail_w_sends_add_watchlist_command() {
        use crate::types::Watchlist;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.watchlist = Some(Watchlist {
            id: "wl-1".into(),
            name: "Primary".into(),
            assets: vec![],
        });
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        update(&mut app, key(KeyCode::Char('w')));
        // Modal stays open
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "AAPL"),
            "w key should keep symbol detail modal open"
        );
        // Command dispatched
        let cmd = cmd_rx.try_recv().expect("AddToWatchlist command expected");
        assert!(
            matches!(cmd, Command::AddToWatchlist { watchlist_id, symbol }
                if watchlist_id == "wl-1" && symbol == "AAPL")
        );
    }

    #[test]
    fn symbol_detail_w_sends_remove_watchlist_command_when_in_watchlist() {
        use crate::types::{Asset, Watchlist};
        let (mut app, mut cmd_rx) = app_with_rx();
        let asset = Asset {
            id: "asset-1".into(),
            symbol: "AAPL".into(),
            name: "Apple Inc.".into(),
            exchange: "NASDAQ".into(),
            asset_class: "us_equity".into(),
            tradable: true,
            shortable: true,
            fractionable: true,
            easy_to_borrow: true,
        };
        app.watchlist = Some(Watchlist {
            id: "wl-1".into(),
            name: "Primary".into(),
            assets: vec![asset],
        });
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        update(&mut app, key(KeyCode::Char('w')));
        let cmd = cmd_rx
            .try_recv()
            .expect("RemoveFromWatchlist command expected");
        assert!(
            matches!(cmd, Command::RemoveFromWatchlist { watchlist_id, symbol }
                if watchlist_id == "wl-1" && symbol == "AAPL")
        );
    }

    #[test]
    fn symbol_detail_other_key_keeps_modal_open() {
        let (mut app, _rx) = app_with_rx();
        app.modal = Some(Modal::SymbolDetail("TSLA".into()));
        update(&mut app, key(KeyCode::Char('j')));
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "TSLA"),
            "unknown key should keep symbol detail modal open"
        );
    }

    #[test]
    fn symbol_detail_esc_closes_modal() {
        let (mut app, _rx) = app_with_rx();
        app.modal = Some(Modal::SymbolDetail("TSLA".into()));
        update(&mut app, key(KeyCode::Esc));
        assert!(app.modal.is_none(), "Esc should close the modal");
    }

    #[test]
    fn symbol_detail_w_with_no_watchlist_keeps_modal_open() {
        // When there is no watchlist, pressing 'w' should keep the modal open
        // without dispatching any command (covers the wl_info == None branch).
        let (mut app, mut cmd_rx) = app_with_rx();
        app.watchlist = None;
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        update(&mut app, key(KeyCode::Char('w')));
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "AAPL"),
            "modal should remain open when no watchlist is set"
        );
        assert!(
            cmd_rx.try_recv().is_err(),
            "no command should be dispatched without a watchlist"
        );
    }

    // ── Positions panel key handling ─────────────────────────────────────────

    fn positions_app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
        let (mut app, rx) = app_with_rx();
        app.active_tab = Tab::Positions;
        app.positions = vec![crate::types::Position {
            symbol: "AAPL".into(),
            qty: "10".into(),
            avg_entry_price: "150.00".into(),
            current_price: "155.00".into(),
            market_value: "1550.00".into(),
            unrealized_pl: "50.00".into(),
            unrealized_plpc: "0.033".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }];
        app.positions_state.select(Some(0));
        (app, rx)
    }

    #[test]
    fn positions_enter_opens_symbol_detail_and_dispatches_fetch() {
        let (mut app, mut cmd_rx) = positions_app();
        update(&mut app, key(KeyCode::Enter));
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "AAPL"),
            "Enter on a position should open SymbolDetail for that symbol"
        );
        let cmd = cmd_rx
            .try_recv()
            .expect("FetchIntradayBars should be dispatched");
        assert!(
            matches!(cmd, Command::FetchIntradayBars(s) if s == "AAPL"),
            "expected FetchIntradayBars for AAPL"
        );
    }

    #[test]
    fn positions_enter_with_no_selection_does_nothing() {
        let (mut app, _rx) = app_with_rx();
        app.active_tab = Tab::Positions;
        app.positions_state.select(None);
        update(&mut app, key(KeyCode::Enter));
        assert!(
            app.modal.is_none(),
            "Enter with no selection should not open a modal"
        );
    }

    #[test]
    fn positions_o_opens_sell_order_entry() {
        let (mut app, _rx) = positions_app();
        update(&mut app, key(KeyCode::Char('o')));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "AAPL" && s.side == crate::app::OrderSide::Sell),
            "o key in positions should open SELL order entry for selected symbol"
        );
    }

    #[test]
    fn positions_s_opens_sell_short_order_entry() {
        let (mut app, _rx) = positions_app();
        update(&mut app, key(KeyCode::Char('s')));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "AAPL" && s.side == crate::app::OrderSide::SellShort),
            "s key in positions should open SELL SHORT order entry for selected symbol"
        );
    }

    #[test]
    fn order_side_cycle_next_wraps() {
        use crate::app::OrderSide;
        assert_eq!(OrderSide::Buy.cycle_next(), OrderSide::Sell);
        assert_eq!(OrderSide::Sell.cycle_next(), OrderSide::SellShort);
        assert_eq!(OrderSide::SellShort.cycle_next(), OrderSide::Buy);
    }

    #[test]
    fn order_side_cycle_prev_wraps() {
        use crate::app::OrderSide;
        assert_eq!(OrderSide::Buy.cycle_prev(), OrderSide::SellShort);
        assert_eq!(OrderSide::Sell.cycle_prev(), OrderSide::Buy);
        assert_eq!(OrderSide::SellShort.cycle_prev(), OrderSide::Sell);
    }

    #[test]
    fn order_side_as_str() {
        use crate::app::OrderSide;
        assert_eq!(OrderSide::Buy.as_str(), "buy");
        assert_eq!(OrderSide::Sell.as_str(), "sell");
        assert_eq!(OrderSide::SellShort.as_str(), "sell_short");
    }

    #[test]
    fn modal_side_right_arrow_cycles_forward() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Side;
        state.side = OrderSide::Buy;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Right));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.side == OrderSide::Sell),
            "right arrow should cycle Buy → Sell"
        );
    }

    #[test]
    fn modal_side_left_arrow_cycles_backward() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Side;
        state.side = OrderSide::Sell;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Left));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.side == OrderSide::Buy),
            "left arrow should cycle Sell → Buy"
        );
    }

    #[test]
    fn modal_side_down_arrow_cycles_forward() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Side;
        state.side = OrderSide::Buy;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Down));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.side == OrderSide::Sell),
            "down arrow should cycle Buy → Sell"
        );
    }

    #[test]
    fn modal_side_up_arrow_cycles_backward() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Side;
        state.side = OrderSide::Sell;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Up));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.side == OrderSide::Buy),
            "up arrow should cycle Sell → Buy"
        );
    }

    #[test]
    fn modal_order_type_down_arrow_toggles() {
        use crate::app::{OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::OrderType;
        state.market_order = false;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Down));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.market_order),
            "down arrow on OrderType should toggle to market"
        );
    }

    #[test]
    fn modal_order_type_up_arrow_toggles() {
        use crate::app::{OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::OrderType;
        state.market_order = true;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Up));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if !s.market_order),
            "up arrow on OrderType should toggle to limit"
        );
    }

    #[test]
    fn modal_tif_down_arrow_toggles() {
        use crate::app::{OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::TimeInForce;
        state.gtc_order = false;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Down));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.gtc_order),
            "down arrow on TIF should toggle to GTC"
        );
    }

    #[test]
    fn modal_tif_up_arrow_toggles() {
        use crate::app::{OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::TimeInForce;
        state.gtc_order = true;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Up));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if !s.gtc_order),
            "up arrow on TIF should toggle to DAY"
        );
    }

    #[test]
    fn modal_up_down_on_text_field_is_noop() {
        use crate::app::{OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Qty;
        state.qty_input = "10".into();
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Down));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.qty_input == "10"),
            "down arrow on text field should not modify input"
        );
        update(&mut app, key(KeyCode::Up));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.qty_input == "10"),
            "up arrow on text field should not modify input"
        );
    }

    #[test]
    fn order_entry_submit_sell_short_sends_correct_side() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        use crate::types::AccountInfo;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let mut state = OrderEntryState::new("AAPL".into());
        state.side = OrderSide::SellShort;
        state.focused_field = OrderField::Submit;
        state.qty_input = "5".into();
        state.price_input = "180.00".into();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_none(), "modal should close after submit");
        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::SubmitOrder { side, .. } if side == "sell_short"),
            "expected sell_short side in SubmitOrder"
        );
    }
}
