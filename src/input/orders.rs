use crossterm::event::KeyCode;

use crate::app::{App, ConfirmAction, Modal, OrderEntryState, OrdersSubTab};

pub(crate) fn handle_orders_key(app: &mut App, key: crossterm::event::KeyEvent) {
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
}
