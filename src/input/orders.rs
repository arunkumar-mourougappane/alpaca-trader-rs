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

    #[test]
    fn unhandled_key_in_filter_mode_is_ignored() {
        // Arrow keys and other non-text keys should be silently ignored in filter mode
        let mut app = make_test_app();
        press(&mut app, KeyCode::Char('f'));
        app.orders_symbol_filter = "AB".to_string();
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Home);
        // filter string unchanged, mode still active
        assert_eq!(app.orders_symbol_filter, "AB");
        assert!(app.orders_filter_active);
    }

    #[test]
    fn c_key_with_selected_order_opens_confirm_modal() {
        let mut app = make_test_app();
        app.orders.push(make_order("order-abc-123", "new"));
        app.orders_state.select(Some(0));
        press(&mut app, KeyCode::Char('c'));
        match &app.modal {
            Some(crate::app::Modal::Confirm { action, .. }) => {
                assert!(
                    matches!(action, crate::app::ConfirmAction::CancelOrder(_)),
                    "expected CancelOrder action"
                );
            }
            other => panic!("expected Confirm modal, got: {:?}", other),
        }
    }

    #[test]
    fn c_key_with_no_selection_does_nothing() {
        let mut app = make_test_app();
        // orders_state starts with no selection and no orders
        press(&mut app, KeyCode::Char('c'));
        assert!(app.modal.is_none());
    }

    #[test]
    fn j_key_moves_selection_down() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders.push(make_order("o2", "new"));
        app.orders_state.select(Some(0));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.orders_state.selected(), Some(1));
    }

    #[test]
    fn k_key_moves_selection_up() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders.push(make_order("o2", "new"));
        app.orders_state.select(Some(1));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.orders_state.selected(), Some(0));
    }

    #[test]
    fn capital_g_jumps_to_last_row() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders.push(make_order("o2", "new"));
        app.orders.push(make_order("o3", "new"));
        app.orders_state.select(Some(0));
        press(&mut app, KeyCode::Char('G'));
        assert_eq!(app.orders_state.selected(), Some(2));
    }

    #[test]
    fn shift_f_resets_selection_to_zero() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders.push(make_order("o2", "new"));
        app.orders_state.select(Some(1));
        app.orders_symbol_filter = "AA".to_string();
        let event = KeyEvent::new(KeyCode::Char('F'), KeyModifiers::SHIFT);
        super::handle_orders_key(&mut app, event);
        assert!(app.orders_symbol_filter.is_empty());
        assert_eq!(app.orders_state.selected(), Some(0));
    }

    #[test]
    fn f_key_clears_previous_filter_before_activating() {
        let mut app = make_test_app();
        app.orders_symbol_filter = "OLD".to_string();
        press(&mut app, KeyCode::Char('f'));
        assert!(app.orders_filter_active);
        assert!(app.orders_symbol_filter.is_empty());
    }
}
