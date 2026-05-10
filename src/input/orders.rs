use crossterm::event::KeyCode;

use crate::app::{App, ConfirmAction, Modal, OrderEntryState, OrdersSubTab};

pub(crate) fn handle_orders_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let orders = app.filtered_orders();
    let len = orders.len();

    match key.code {
        KeyCode::Char('j') | KeyCode::Down if len > 0 => {
            let i = app.orders_state.selected().unwrap_or(0);
            app.orders_state.select(Some((i + 1).min(len - 1)));
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let i = app.orders_state.selected().unwrap_or(0);
            app.orders_state.select(Some(i.saturating_sub(1)));
        }
        KeyCode::Char('g') => app.orders_state.select(Some(0)),
        KeyCode::Char('G') if len > 0 => {
            app.orders_state.select(Some(len - 1));
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
