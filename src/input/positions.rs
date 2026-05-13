use crossterm::event::KeyCode;

use crate::app::{App, Modal, OrderEntryState};

pub(crate) fn handle_positions_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let len = app.positions.len();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down if len > 0 => {
            let i = app.positions_state.selected().unwrap_or(0);
            app.positions_state.select(Some((i + 1).min(len - 1)));
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let i = app.positions_state.selected().unwrap_or(0);
            app.positions_state.select(Some(i.saturating_sub(1)));
        }
        KeyCode::Char('g') => app.positions_state.select(Some(0)),
        KeyCode::Char('G') if len > 0 => {
            app.positions_state.select(Some(len - 1));
        }
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
            let mut state = OrderEntryState::new(symbol);
            state.side_buy = false;
            app.modal = Some(Modal::OrderEntry(state));
        }
        _ => {}
    }
}
