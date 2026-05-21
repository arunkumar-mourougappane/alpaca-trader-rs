use crossterm::event::KeyCode;

use crate::app::{App, ConfirmAction, Modal, OrderEntryState, OrdersSubTab};

pub(crate) fn handle_orders_key(app: &mut App, key: crossterm::event::KeyEvent) {
    // While the filter input is active, intercept all keys for text editing.
    if app.orders_filter_active {
        match key.code {
            KeyCode::Esc => {
                app.orders_filter_active = false;
                app.orders_symbol_filter.clear();
                app.orders_state.select(Some(0));
            }
            KeyCode::Enter => {
                app.orders_filter_active = false;
                app.orders_state.select(Some(0));
            }
            KeyCode::Backspace => {
                app.orders_symbol_filter.pop();
            }
            KeyCode::Char(c) => {
                app.orders_symbol_filter.push(c.to_ascii_uppercase());
            }
            _ => {}
        }
        return;
    }

    let orders = app.filtered_orders();
    let len = orders.len();

    super::handle_nav_key(key.code, len, &mut app.orders_state, &mut app.pending_g_at);

    match key.code {
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
        KeyCode::Char('f') => {
            app.orders_filter_active = true;
            app.orders_symbol_filter.clear();
        }
        KeyCode::Char('F') => {
            // Clear any active symbol filter.
            app.orders_symbol_filter.clear();
            app.orders_filter_active = false;
            app.orders_state.select(Some(0));
        }
        KeyCode::Char('s') => {
            app.orders_sort.col = app.orders_sort.col.cycle();
        }
        KeyCode::Char('S') => {
            app.orders_sort.toggle_dir();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::app::test_helpers::{make_order, make_test_app};
    use crate::app::{OrderSortCol, SortDir};

    fn press(app: &mut crate::app::App, code: KeyCode) {
        let event = KeyEvent::new(code, KeyModifiers::NONE);
        super::handle_orders_key(app, event);
    }

    #[test]
    fn s_key_cycles_orders_sort_column() {
        let mut app = make_test_app();
        app.orders.push(make_order("order-1", "new"));
        assert_eq!(app.orders_sort.col, OrderSortCol::None);
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.orders_sort.col, OrderSortCol::Symbol);
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.orders_sort.col, OrderSortCol::Side);
    }

    #[test]
    fn shift_s_toggles_orders_sort_direction() {
        let mut app = make_test_app();
        assert_eq!(app.orders_sort.dir, SortDir::Asc);
        let event = crossterm::event::KeyEvent::new(
            KeyCode::Char('S'),
            crossterm::event::KeyModifiers::SHIFT,
        );
        super::handle_orders_key(&mut app, event);
        assert_eq!(app.orders_sort.dir, SortDir::Desc);
    }

    #[test]
    fn s_key_wraps_back_to_none_after_full_cycle() {
        let mut app = make_test_app();
        // cycle through all 5 sortable columns + back to None
        for _ in 0..6 {
            press(&mut app, KeyCode::Char('s'));
        }
        assert_eq!(app.orders_sort.col, OrderSortCol::None);
    }

    #[test]
    fn subtab_1_2_3_keys_switch_orders_subtabs() {
        let mut app = make_test_app();
        press(&mut app, KeyCode::Char('2'));
        assert_eq!(app.orders_subtab, crate::app::OrdersSubTab::Filled);
        press(&mut app, KeyCode::Char('3'));
        assert_eq!(app.orders_subtab, crate::app::OrdersSubTab::Cancelled);
        press(&mut app, KeyCode::Char('1'));
        assert_eq!(app.orders_subtab, crate::app::OrdersSubTab::Open);
    }

    #[test]
    fn f_key_activates_filter_mode() {
        let mut app = make_test_app();
        assert!(!app.orders_filter_active);
        press(&mut app, KeyCode::Char('f'));
        assert!(app.orders_filter_active);
        assert!(app.orders_symbol_filter.is_empty());
    }

    #[test]
    fn chars_typed_in_filter_mode_build_filter_string() {
        let mut app = make_test_app();
        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Char('p'));
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.orders_symbol_filter, "AAPL");
    }

    #[test]
    fn backspace_removes_last_char_in_filter_mode() {
        let mut app = make_test_app();
        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Char('b'));
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.orders_symbol_filter, "A");
    }

    #[test]
    fn enter_confirms_filter_and_exits_input_mode() {
        let mut app = make_test_app();
        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Enter);
        assert!(!app.orders_filter_active);
        assert_eq!(app.orders_symbol_filter, "A");
    }

    #[test]
    fn esc_clears_filter_and_exits_input_mode() {
        let mut app = make_test_app();
        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Esc);
        assert!(!app.orders_filter_active);
        assert!(app.orders_symbol_filter.is_empty());
    }

    #[test]
    fn shift_f_clears_active_filter() {
        let mut app = make_test_app();
        app.orders_symbol_filter = "AAPL".to_string();
        let event = KeyEvent::new(KeyCode::Char('F'), KeyModifiers::SHIFT);
        super::handle_orders_key(&mut app, event);
        assert!(app.orders_symbol_filter.is_empty());
        assert!(!app.orders_filter_active);
    }

    #[test]
    fn filter_mode_does_not_consume_normal_keys_when_inactive() {
        // 'o' should open order modal when NOT in filter mode
        let mut app = make_test_app();
        assert!(!app.orders_filter_active);
        press(&mut app, KeyCode::Char('o'));
        assert!(app.modal.is_some());
    }

    #[test]
    fn filter_mode_blocks_normal_key_handling() {
        // When filter is active, 'o' should be appended to the filter string, not open modal
        let mut app = make_test_app();
        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Char('o'));
        assert_eq!(app.orders_symbol_filter, "O");
        assert!(app.modal.is_none());
    }
}
