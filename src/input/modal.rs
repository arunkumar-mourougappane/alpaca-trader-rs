use crossterm::event::KeyCode;

use super::send_command;
use crate::app::{App, ConfirmAction, Modal, OrderField, StatusMessage};
use crate::commands::Command;

pub(crate) fn handle_modal_key(app: &mut App, key: crossterm::event::KeyEvent) {
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
                    OrderField::Qty if c.is_ascii_digit() || c == '.' => {
                        state.qty_input.push(c);
                    }
                    OrderField::Price if c.is_ascii_digit() || c == '.' => {
                        state.price_input.push(c);
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
                        let buying_power = app
                            .account
                            .as_ref()
                            .and_then(|a| a.buying_power.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        if let Some(err) = crate::input::validate(&state, buying_power) {
                            app.status_msg = StatusMessage::persistent(err);
                            app.modal = Some(Modal::OrderEntry(state));
                            return;
                        }
                        send_command(
                            app,
                            Command::SubmitOrder {
                                symbol: state.symbol.clone(),
                                side: if state.side_buy { "buy" } else { "sell" }.into(),
                                order_type: if state.market_order {
                                    "market"
                                } else {
                                    "limit"
                                }
                                .into(),
                                qty: if state.qty_input.is_empty() {
                                    None
                                } else {
                                    Some(state.qty_input.clone())
                                },
                                price: if state.market_order || state.price_input.is_empty() {
                                    None
                                } else {
                                    Some(state.price_input.clone())
                                },
                                time_in_force: "day".into(),
                            },
                            "Submitting order…",
                        );
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
        } => match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Char('y') | KeyCode::Char('n') => {
                confirmed = matches!(key.code, KeyCode::Char('y') | KeyCode::Left);
                if confirmed {
                    match &action {
                        ConfirmAction::CancelOrder(id) => {
                            send_command(
                                app,
                                Command::CancelOrder(id.clone()),
                                format!("Cancelling {}…", &id[..id.len().min(8)]),
                            );
                        }
                        ConfirmAction::RemoveFromWatchlist {
                            watchlist_id,
                            symbol,
                        } => {
                            send_command(
                                app,
                                Command::RemoveFromWatchlist {
                                    watchlist_id: watchlist_id.clone(),
                                    symbol: symbol.clone(),
                                },
                                format!("Removing {}…", symbol),
                            );
                        }
                    }
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
        },

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
                    send_command(
                        app,
                        Command::AddToWatchlist {
                            watchlist_id: watchlist_id.clone(),
                            symbol: input.clone(),
                        },
                        format!("Adding {}…", input),
                    );
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
