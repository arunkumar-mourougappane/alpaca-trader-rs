use crossterm::event::KeyCode;

use crate::app::{App, ConfirmAction, Modal, OrderEntryState};

pub(crate) fn handle_watchlist_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let len = app.watchlist.as_ref().map(|w| w.assets.len()).unwrap_or(0);

    match key.code {
        KeyCode::Char('j') | KeyCode::Down if len > 0 => {
            let i = app.watchlist_state.selected().unwrap_or(0);
            app.watchlist_state.select(Some((i + 1).min(len - 1)));
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let i = app.watchlist_state.selected().unwrap_or(0);
            app.watchlist_state.select(Some(i.saturating_sub(1)));
        }
        KeyCode::Char('g') => app.watchlist_state.select(Some(0)),
        KeyCode::Char('G') if len > 0 => {
            app.watchlist_state.select(Some(len - 1));
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
