use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, ConfirmAction, Modal, OrderEntryState, OrderField, OrdersSubTab, Tab};
use crate::events::Event;

pub fn update(app: &mut App, event: Event) {
    match event {
        Event::Input(key) => handle_key(app, key),
        Event::Mouse(_) => {}
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

fn handle_watchlist_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let len = app.watchlist.as_ref().map(|w| w.assets.len()).unwrap_or(0);

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                let i = app.watchlist_state.selected().unwrap_or(0);
                app.watchlist_state.select(Some((i + 1).min(len - 1)));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let i = app.watchlist_state.selected().unwrap_or(0);
            app.watchlist_state.select(Some(i.saturating_sub(1)));
        }
        KeyCode::Char('g') => app.watchlist_state.select(Some(0)),
        KeyCode::Char('G') => {
            if len > 0 {
                app.watchlist_state.select(Some(len - 1));
            }
        }
        KeyCode::Enter => {
            if let Some(symbol) = app.selected_watchlist_symbol() {
                app.modal = Some(Modal::SymbolDetail(symbol));
            }
        }
        KeyCode::Char('o') => {
            let symbol = app.selected_watchlist_symbol().unwrap_or_default();
            app.modal = Some(Modal::OrderEntry(OrderEntryState::new(symbol)));
        }
        KeyCode::Char('a') => {
            if let Some(wl) = &app.watchlist {
                let id = wl.id.clone();
                app.modal = Some(Modal::AddSymbol {
                    input: String::new(),
                    watchlist_id: id,
                });
            }
        }
        KeyCode::Char('d') => {
            if let (Some(symbol), Some(wl)) =
                (app.selected_watchlist_symbol(), app.watchlist.as_ref())
            {
                let wl_id = wl.id.clone();
                app.modal = Some(Modal::Confirm {
                    message: format!("Remove {} from watchlist?", symbol),
                    action: ConfirmAction::RemoveFromWatchlist {
                        watchlist_id: wl_id,
                        symbol,
                    },
                    confirmed: false,
                });
            }
        }
        KeyCode::Char('/') => {
            app.searching = true;
            app.search_query.clear();
        }
        _ => {}
    }
}

fn handle_positions_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let len = app.positions.len();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                let i = app.positions_state.selected().unwrap_or(0);
                app.positions_state.select(Some((i + 1).min(len - 1)));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let i = app.positions_state.selected().unwrap_or(0);
            app.positions_state.select(Some(i.saturating_sub(1)));
        }
        KeyCode::Char('g') => app.positions_state.select(Some(0)),
        KeyCode::Char('G') => {
            if len > 0 {
                app.positions_state.select(Some(len - 1));
            }
        }
        KeyCode::Enter => {
            if let Some(symbol) = app.selected_position_symbol() {
                app.modal = Some(Modal::SymbolDetail(symbol));
            }
        }
        KeyCode::Char('o') => {
            let symbol = app.selected_position_symbol().unwrap_or_default();
            let mut state = OrderEntryState::new(symbol);
            state.side_buy = false;
            app.modal = Some(Modal::OrderEntry(state));
        }
        _ => {}
    }
}

fn handle_orders_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let orders = app.filtered_orders();
    let len = orders.len();

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                let i = app.orders_state.selected().unwrap_or(0);
                app.orders_state.select(Some((i + 1).min(len - 1)));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let i = app.orders_state.selected().unwrap_or(0);
            app.orders_state.select(Some(i.saturating_sub(1)));
        }
        KeyCode::Char('g') => app.orders_state.select(Some(0)),
        KeyCode::Char('G') => {
            if len > 0 {
                app.orders_state.select(Some(len - 1));
            }
        }
        KeyCode::Char('1') => {
            app.orders_subtab = OrdersSubTab::Open;
            app.orders_state.select(Some(0));
        }
        KeyCode::Char('2') => {
            app.orders_subtab = OrdersSubTab::Filled;
            app.orders_state.select(Some(0));
        }
        KeyCode::Char('3') => {
            app.orders_subtab = OrdersSubTab::Cancelled;
            app.orders_state.select(Some(0));
        }
        KeyCode::Char('o') => {
            app.modal = Some(Modal::OrderEntry(OrderEntryState::new(String::new())));
        }
        KeyCode::Char('c') => {
            if let Some(id) = app.selected_order_id() {
                app.modal = Some(Modal::Confirm {
                    message: format!("Cancel order {}?", &id[..id.len().min(8)]),
                    action: ConfirmAction::CancelOrder(id),
                    confirmed: false,
                });
            }
        }
        _ => {}
    }
}

fn handle_modal_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if key.code == KeyCode::Esc {
        app.modal = None;
        return;
    }

    // Clone to avoid borrow issues
    let modal = match app.modal.take() {
        Some(m) => m,
        None => return,
    };

    let new_modal = match modal {
        Modal::Help => {
            if key.code != KeyCode::Esc {
                None
            } else {
                Some(Modal::Help)
            }
        }

        Modal::OrderEntry(mut state) => {
            match key.code {
                KeyCode::Tab => state.focused_field = state.focused_field.next(),
                KeyCode::BackTab => state.focused_field = state.focused_field.prev(),
                KeyCode::Left | KeyCode::Right => match state.focused_field {
                    OrderField::Side => state.side_buy = !state.side_buy,
                    OrderField::OrderType => state.market_order = !state.market_order,
                    _ => {}
                },
                KeyCode::Char(c) => match state.focused_field {
                    OrderField::Symbol => state.symbol.push(c),
                    OrderField::Qty => {
                        if c.is_ascii_digit() || c == '.' {
                            state.qty_input.push(c);
                        }
                    }
                    OrderField::Price => {
                        if c.is_ascii_digit() || c == '.' {
                            state.price_input.push(c);
                        }
                    }
                    OrderField::Side => {
                        if c == 'b' || c == 'B' {
                            state.side_buy = true;
                        } else if c == 's' || c == 'S' {
                            state.side_buy = false;
                        }
                    }
                    _ => {}
                },
                KeyCode::Backspace => match state.focused_field {
                    OrderField::Symbol => {
                        state.symbol.pop();
                    }
                    OrderField::Qty => {
                        state.qty_input.pop();
                    }
                    OrderField::Price => {
                        state.price_input.pop();
                    }
                    _ => {}
                },
                KeyCode::Enter => {
                    if state.focused_field == OrderField::Submit {
                        // Order submission handled in main loop via command channel (Phase 2)
                        app.status_msg = "Order submission coming in Phase 2".into();
                        app.modal = None;
                        return;
                    } else {
                        state.focused_field = state.focused_field.next();
                    }
                }
                _ => {}
            }
            Some(Modal::OrderEntry(state))
        }

        Modal::SymbolDetail(_) => None,

        Modal::Confirm {
            message,
            action,
            mut confirmed,
        } => {
            match key.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Char('y') | KeyCode::Char('n') => {
                    confirmed = matches!(key.code, KeyCode::Char('y') | KeyCode::Left);
                    if confirmed {
                        // Trigger action via notify + status (Phase 2: send command via channel)
                        match &action {
                            ConfirmAction::CancelOrder(id) => {
                                app.status_msg = format!("Cancelling order {}…", &id[..8]);
                            }
                            ConfirmAction::RemoveFromWatchlist { symbol, .. } => {
                                app.status_msg = format!("Removing {}…", symbol);
                            }
                        }
                        app.refresh_notify.notify_one();
                        app.modal = None;
                        return;
                    }
                    None
                }
                KeyCode::Enter => {
                    if confirmed {
                        app.modal = None;
                        return;
                    }
                    Some(Modal::Confirm {
                        message,
                        action,
                        confirmed,
                    })
                }
                _ => Some(Modal::Confirm {
                    message,
                    action,
                    confirmed,
                }),
            }
        }

        Modal::AddSymbol {
            mut input,
            watchlist_id,
        } => match key.code {
            KeyCode::Char(c) => {
                input.push(c.to_ascii_uppercase());
                Some(Modal::AddSymbol {
                    input,
                    watchlist_id,
                })
            }
            KeyCode::Backspace => {
                input.pop();
                Some(Modal::AddSymbol {
                    input,
                    watchlist_id,
                })
            }
            KeyCode::Enter => {
                if !input.is_empty() {
                    app.status_msg = format!("Adding {}…", input);
                    app.refresh_notify.notify_one();
                }
                None
            }
            _ => Some(Modal::AddSymbol {
                input,
                watchlist_id,
            }),
        },
    };

    app.modal = new_modal;
}

fn handle_search_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.searching = false;
        }
        KeyCode::Backspace => {
            app.search_query.pop();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.watchlist_state.select(Some(0));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_helpers::*;
    use crate::app::{Modal, OrdersSubTab, Tab};
    use crate::config::{AlpacaConfig, AlpacaEnv};
    use crate::events::Event;
    use crate::types::{AccountInfo, MarketClock, Order, Quote, Watchlist};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::sync::Arc;

    fn key(code: KeyCode) -> Event {
        Event::Input(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ctrl(code: KeyCode) -> Event {
        Event::Input(KeyEvent::new(code, KeyModifiers::CONTROL))
    }

    // ── Data events ───────────────────────────────────────────────────────────

    #[test]
    fn account_updated_sets_account_and_pushes_equity() {
        let mut app = make_test_app();
        let acc = AccountInfo { equity: "500".into(), status: "ACTIVE".into(), ..Default::default() };
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
            symbol: "AAPL".into(), qty: "10".into(),
            avg_entry_price: "100".into(), current_price: "110".into(),
            market_value: "1100".into(), unrealized_pl: "100".into(),
            unrealized_plpc: "0.1".into(), side: "long".into(),
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
        let updated = Order { id: "o1".into(), status: "filled".into(), ..make_order("o1", "filled") };
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
        let q = Quote { symbol: "AAPL".into(), ap: Some(185.0), bp: Some(184.9), ..Default::default() };
        update(&mut app, Event::MarketQuote(q));
        assert!(app.quotes.contains_key("AAPL"));
        assert_eq!(app.quotes["AAPL"].ap, Some(185.0));
    }

    #[test]
    fn clock_updated() {
        let mut app = make_test_app();
        let clock = MarketClock { is_open: true, ..Default::default() };
        update(&mut app, Event::ClockUpdated(clock));
        assert_eq!(app.clock.as_ref().unwrap().is_open, true);
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
        app.orders = vec![
            make_order("o1", "accepted"),
            make_order("o2", "accepted"),
        ];
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
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.focused_field == OrderField::Price));
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
}
