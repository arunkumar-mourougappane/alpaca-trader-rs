use crossterm::event::KeyCode;

use crate::app::{App, Modal, OrderEntryState, OrderSide};

pub(crate) fn handle_positions_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let len = app.positions.len();

    super::handle_nav_key(
        key.code,
        len,
        &mut app.positions_state,
        &mut app.pending_g_at,
    );

    match key.code {
        KeyCode::Enter => {
            if let Some(symbol) = app.selected_position_symbol() {
                let _ = app
                    .command_tx
                    .try_send(crate::commands::Command::FetchIntradayBars(symbol.clone()));
                app.modal = Some(Modal::SymbolDetail(symbol));
            }
        }
        KeyCode::Char('o') => {
            let symbol = app.selected_position_symbol().unwrap_or_default();
            app.modal = Some(Modal::OrderEntry(
                OrderEntryState::new(symbol).with_side(OrderSide::Sell),
            ));
        }
        KeyCode::Char('s') => {
            let symbol = app.selected_position_symbol().unwrap_or_default();
            app.modal = Some(Modal::OrderEntry(
                OrderEntryState::new(symbol).with_side(OrderSide::SellShort),
            ));
        }
        _ => {}
    }
}
