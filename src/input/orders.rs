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
        _ => {}
    }
}
