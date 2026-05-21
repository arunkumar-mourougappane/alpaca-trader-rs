use crossterm::event::KeyCode;

use super::send_command;
use crate::app::{
    App, ConfirmAction, Modal, OrderEntryState, OrderField, OrderSide, StatusMessage,
};
use crate::commands::Command;

pub(crate) fn handle_modal_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if key.code == KeyCode::Esc {
        // First Esc clears an active crosshair; second Esc closes the modal.
        if matches!(app.modal, Some(Modal::SymbolDetail(_)))
            && app.symbol_detail_crosshair.is_some()
        {
            app.symbol_detail_crosshair = None;
            return;
        }
        app.modal = None;
        app.symbol_detail_crosshair = None;
        return;
    }

    // Clone to avoid borrow issues
    let modal = match app.modal.take() {
        Some(m) => m,
        None => return,
    };

    let new_modal = match modal {
        // Esc is already handled above; any other key also closes the help overlay.
        Modal::Help => None,

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
                            app.push_status(StatusMessage::persistent(err));
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
                app.modal = Some(Modal::OrderEntry(
                    OrderEntryState::new(symbol.clone()).with_side(OrderSide::Sell),
                ));
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
            KeyCode::Left => {
                let len = app
                    .intraday_bars
                    .get(symbol.as_str())
                    .map(|b| b.len())
                    .unwrap_or(0);
                if len > 0 {
                    app.symbol_detail_crosshair = Some(match app.symbol_detail_crosshair {
                        Some(i) => i.saturating_sub(1),
                        None => len - 1,
                    });
                }
                Some(Modal::SymbolDetail(symbol))
            }
            KeyCode::Right => {
                let len = app
                    .intraday_bars
                    .get(symbol.as_str())
                    .map(|b| b.len())
                    .unwrap_or(0);
                if len > 0 {
                    app.symbol_detail_crosshair = Some(match app.symbol_detail_crosshair {
                        Some(i) => (i + 1).min(len - 1),
                        None => 0,
                    });
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

        // Dedicated watchlist-removal confirmation modal.
        // `y` or `Enter` confirms; `n` or `Esc` cancels.
        Modal::ConfirmRemoveWatchlist {
            symbol,
            watchlist_id,
        } => match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                send_command(
                    app,
                    Command::RemoveFromWatchlist {
                        watchlist_id: watchlist_id.clone(),
                        symbol: symbol.clone(),
                    },
                    format!("Removing {}…", symbol),
                );
                app.modal = None;
                return;
            }
            KeyCode::Char('n') => {
                // 'n' cancels; Esc is handled at the top of the function
                None
            }
            _ => Some(Modal::ConfirmRemoveWatchlist {
                symbol,
                watchlist_id,
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

        Modal::GlobalSearch { mut query } => match key.code {
            KeyCode::Char(c) => {
                query.push(c.to_ascii_uppercase());
                Some(Modal::GlobalSearch { query })
            }
            KeyCode::Backspace => {
                query.pop();
                Some(Modal::GlobalSearch { query })
            }
            KeyCode::Enter => {
                if query.is_empty() {
                    None
                } else {
                    let _ = app
                        .command_tx
                        .try_send(Command::FetchIntradayBars(query.clone()));
                    Some(Modal::SymbolDetail(query))
                }
            }
            _ => Some(Modal::GlobalSearch { query }),
        },

        Modal::PositionDetail { symbol } => match key.code {
            KeyCode::Char('o') => {
                app.modal = Some(Modal::OrderEntry(OrderEntryState::new(symbol)));
                return;
            }
            _ => Some(Modal::PositionDetail { symbol }),
        },
    };

    app.modal = new_modal;
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::app::test_helpers::make_test_app;
    use crate::app::Modal;

    fn press(app: &mut crate::app::App, code: KeyCode) {
        let event = KeyEvent::new(code, KeyModifiers::NONE);
        super::handle_modal_key(app, event);
    }

    // ── Help modal ────────────────────────────────────────────────────────────

    #[test]
    fn esc_closes_help_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Help);
        press(&mut app, KeyCode::Esc);
        assert!(app.modal.is_none(), "Esc should close the Help modal");
    }

    #[test]
    fn any_key_closes_help_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Help);
        press(&mut app, KeyCode::Enter);
        assert!(
            app.modal.is_none(),
            "any key should close Help; got: {:?}",
            app.modal
        );
    }

    #[test]
    fn question_mark_closes_help_modal_when_open() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Help);
        press(&mut app, KeyCode::Char('?'));
        assert!(
            app.modal.is_none(),
            "? should dismiss Help when already open; got: {:?}",
            app.modal
        );
    }

    // ── PositionDetail modal ───────────────────────────────────────────────────

    #[test]
    fn esc_closes_position_detail_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::PositionDetail {
            symbol: "AAPL".into(),
        });
        press(&mut app, KeyCode::Esc);
        assert!(app.modal.is_none());
    }

    #[test]
    fn o_key_in_position_detail_opens_order_entry() {
        let mut app = make_test_app();
        app.modal = Some(Modal::PositionDetail {
            symbol: "AAPL".into(),
        });
        press(&mut app, KeyCode::Char('o'));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "AAPL"),
            "expected OrderEntry modal for AAPL, got: {:?}",
            app.modal
        );
    }

    #[test]
    fn unhandled_key_keeps_position_detail_modal_open() {
        let mut app = make_test_app();
        app.modal = Some(Modal::PositionDetail {
            symbol: "TSLA".into(),
        });
        press(&mut app, KeyCode::Char('z'));
        assert!(
            matches!(&app.modal, Some(Modal::PositionDetail { symbol }) if symbol == "TSLA"),
            "expected PositionDetail to stay open, got: {:?}",
            app.modal
        );
    }

    // ── SymbolDetail crosshair ────────────────────────────────────────────────

    fn setup_symbol_detail_with_bars(bars: Vec<u64>) -> crate::app::App {
        let mut app = make_test_app();
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        app.intraday_bars.insert("AAPL".into(), bars);
        app
    }

    #[test]
    fn left_key_initialises_crosshair_at_last_bar() {
        let mut app = setup_symbol_detail_with_bars(vec![100, 200, 300]);
        press(&mut app, KeyCode::Left);
        assert_eq!(app.symbol_detail_crosshair, Some(2));
    }

    #[test]
    fn right_key_initialises_crosshair_at_first_bar() {
        let mut app = setup_symbol_detail_with_bars(vec![100, 200, 300]);
        press(&mut app, KeyCode::Right);
        assert_eq!(app.symbol_detail_crosshair, Some(0));
    }

    #[test]
    fn left_key_moves_crosshair_left() {
        let mut app = setup_symbol_detail_with_bars(vec![100, 200, 300]);
        app.symbol_detail_crosshair = Some(2);
        press(&mut app, KeyCode::Left);
        assert_eq!(app.symbol_detail_crosshair, Some(1));
    }

    #[test]
    fn right_key_moves_crosshair_right() {
        let mut app = setup_symbol_detail_with_bars(vec![100, 200, 300]);
        app.symbol_detail_crosshair = Some(0);
        press(&mut app, KeyCode::Right);
        assert_eq!(app.symbol_detail_crosshair, Some(1));
    }

    #[test]
    fn left_key_clamps_at_zero() {
        let mut app = setup_symbol_detail_with_bars(vec![100, 200, 300]);
        app.symbol_detail_crosshair = Some(0);
        press(&mut app, KeyCode::Left);
        assert_eq!(app.symbol_detail_crosshair, Some(0));
    }

    #[test]
    fn right_key_clamps_at_last_bar() {
        let mut app = setup_symbol_detail_with_bars(vec![100, 200, 300]);
        app.symbol_detail_crosshair = Some(2);
        press(&mut app, KeyCode::Right);
        assert_eq!(app.symbol_detail_crosshair, Some(2));
    }

    #[test]
    fn left_key_with_no_bars_does_nothing() {
        let mut app = setup_symbol_detail_with_bars(vec![]);
        press(&mut app, KeyCode::Left);
        assert!(app.symbol_detail_crosshair.is_none());
    }

    #[test]
    fn right_key_with_no_bars_does_nothing() {
        let mut app = setup_symbol_detail_with_bars(vec![]);
        press(&mut app, KeyCode::Right);
        assert!(app.symbol_detail_crosshair.is_none());
    }

    #[test]
    fn esc_clears_active_crosshair_without_closing_modal() {
        let mut app = setup_symbol_detail_with_bars(vec![100, 200, 300]);
        app.symbol_detail_crosshair = Some(1);
        press(&mut app, KeyCode::Esc);
        assert!(
            app.symbol_detail_crosshair.is_none(),
            "first Esc should clear crosshair"
        );
        assert!(
            matches!(app.modal, Some(Modal::SymbolDetail(_))),
            "modal should still be open after first Esc; got: {:?}",
            app.modal
        );
    }

    #[test]
    fn esc_closes_symbol_detail_when_no_crosshair_active() {
        let mut app = setup_symbol_detail_with_bars(vec![100, 200, 300]);
        assert!(app.symbol_detail_crosshair.is_none());
        press(&mut app, KeyCode::Esc);
        assert!(
            app.modal.is_none(),
            "Esc with no crosshair should close modal; got: {:?}",
            app.modal
        );
    }

    #[test]
    fn unhandled_key_keeps_symbol_detail_open() {
        let mut app = setup_symbol_detail_with_bars(vec![100, 200]);
        press(&mut app, KeyCode::Char('z'));
        assert!(
            matches!(app.modal, Some(Modal::SymbolDetail(_))),
            "unhandled key should keep SymbolDetail open; got: {:?}",
            app.modal
        );
    }
}
