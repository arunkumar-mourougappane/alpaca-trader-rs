use crossterm::event::KeyCode;

use crate::app::{AlertField, App, Modal, OrderEntryState};

pub(crate) fn handle_watchlist_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let len = app.watchlist.as_ref().map(|w| w.assets.len()).unwrap_or(0);

    super::handle_nav_key(
        key.code,
        len,
        &mut app.watchlist_state,
        &mut app.pending_g_at,
    );

    match key.code {
        KeyCode::Enter => {
            if let Some(symbol) = app.selected_watchlist_symbol() {
                let _ = app
                    .command_tx
                    .try_send(crate::commands::Command::FetchIntradayBars(symbol.clone()));
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
        // 'A' (uppercase) — open the price-alert dialog for the selected symbol.
        // Pre-fills existing thresholds so the user can edit or clear them.
        KeyCode::Char('A') => {
            if let Some(symbol) = app.selected_watchlist_symbol() {
                let existing = app.price_alerts.get(&symbol);
                let above_input = existing
                    .and_then(|a| a.above)
                    .map(|v| format!("{:.2}", v))
                    .unwrap_or_default();
                let below_input = existing
                    .and_then(|a| a.below)
                    .map(|v| format!("{:.2}", v))
                    .unwrap_or_default();
                app.modal = Some(Modal::SetAlert {
                    symbol,
                    above_input,
                    below_input,
                    focused: AlertField::Above,
                });
            }
        }
        KeyCode::Char('d') => {
            if let (Some(symbol), Some(wl)) =
                (app.selected_watchlist_symbol(), app.watchlist.as_ref())
            {
                let watchlist_id = wl.id.clone();
                if app.prefs.safety.confirm_watchlist_remove {
                    app.modal = Some(Modal::ConfirmRemoveWatchlist {
                        symbol,
                        watchlist_id,
                    });
                } else {
                    let _ =
                        app.command_tx
                            .try_send(crate::commands::Command::RemoveFromWatchlist {
                                watchlist_id,
                                symbol,
                            });
                }
            }
        }
        KeyCode::Char('/') => {
            app.searching = true;
            app.search_query.clear();
        }
        _ => {}
    }
}

