use crossterm::event::KeyCode;

use super::send_command;
use crate::app::{
    App, ConfirmAction, Modal, OrderEntryState, OrderField, OrderSide, StatusMessage,
};
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

        Modal::About => None,

        Modal::OrderEntry(mut state) => {
            match key.code {
                KeyCode::Tab => state.focused_field = state.focused_field.next(),
                KeyCode::BackTab => state.focused_field = state.focused_field.prev(),
                KeyCode::Left | KeyCode::Right => match state.focused_field {
                    OrderField::Side => {
                        state.side = if key.code == KeyCode::Left {
                            state.side.cycle_prev()
                        } else {
                            state.side.cycle_next()
                        };
                    }
                    OrderField::OrderType => state.market_order = !state.market_order,
                    OrderField::TimeInForce => state.gtc_order = !state.gtc_order,
                    _ => {}
                },
                KeyCode::Up | KeyCode::Down => match state.focused_field {
                    OrderField::Side => {
                        state.side = if key.code == KeyCode::Up {
                            state.side.cycle_prev()
                        } else {
                            state.side.cycle_next()
                        };
                    }
                    OrderField::OrderType => state.market_order = !state.market_order,
                    OrderField::TimeInForce => state.gtc_order = !state.gtc_order,
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
                            state.side = OrderSide::Buy;
                        } else if c == 's' || c == 'S' {
                            state.side = OrderSide::Sell;
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
                        let market_open = app.clock.as_ref().map(|c| c.is_open).unwrap_or(true);
                        if let Some(err) = crate::input::validate(&state, buying_power, market_open)
                        {
                            app.status_msg = StatusMessage::persistent(err);
                            app.modal = Some(Modal::OrderEntry(state));
                            return;
                        }
                        send_command(
                            app,
                            Command::SubmitOrder {
                                symbol: state.symbol.clone(),
                                side: state.side.as_str().into(),
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
                                time_in_force: if state.gtc_order { "gtc" } else { "day" }.into(),
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

        Modal::SymbolDetail(symbol) => match key.code {
            KeyCode::Char('o') => {
                app.modal = Some(Modal::OrderEntry(OrderEntryState::new(symbol.clone())));
                return;
            }
            KeyCode::Char('s') => {
                let mut state = OrderEntryState::new(symbol.clone());
                state.side = OrderSide::Sell;
                app.modal = Some(Modal::OrderEntry(state));
                return;
            }
            KeyCode::Char('w') => {
                let in_watchlist = app
                    .watchlist
                    .as_ref()
                    .map(|w| w.assets.iter().any(|a| a.symbol == symbol))
                    .unwrap_or(false);
                let wl_info = app
                    .watchlist
                    .as_ref()
                    .map(|wl| (wl.id.clone(), in_watchlist));
                if let Some((wl_id, remove)) = wl_info {
                    let (cmd, msg) = if remove {
                        (
                            Command::RemoveFromWatchlist {
                                watchlist_id: wl_id,
                                symbol: symbol.clone(),
                            },
                            format!("Removing {}…", symbol),
                        )
                    } else {
                        (
                            Command::AddToWatchlist {
                                watchlist_id: wl_id,
                                symbol: symbol.clone(),
                            },
                            format!("Adding {}…", symbol),
                        )
                    };
                    send_command(app, cmd, msg);
                }
                Some(Modal::SymbolDetail(symbol))
            }
            _ => Some(Modal::SymbolDetail(symbol)),
        },

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
