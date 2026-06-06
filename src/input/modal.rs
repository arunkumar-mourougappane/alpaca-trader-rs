use crossterm::event::KeyCode;

use super::send_command;
use crate::app::{
    AlertField, App, ConfirmAction, FullOrderType, Modal, OrderEntryState, OrderField, OrderSide,
    StatusMessage, TrailType,
};
use crate::commands::Command;
use crate::types::PriceAlert;

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
                KeyCode::Tab => state.focused_field = state.next_field(),
                KeyCode::BackTab => state.focused_field = state.prev_field(),
                KeyCode::Left | KeyCode::Right => match state.focused_field {
                    OrderField::Side => {
                        state.side = if key.code == KeyCode::Left {
                            state.side.cycle_prev()
                        } else {
                            state.side.cycle_next()
                        };
                    }
                    OrderField::OrderType => {
                        state.order_type = if key.code == KeyCode::Left {
                            state.order_type.cycle_prev()
                        } else {
                            state.order_type.cycle_next()
                        };
                        // Reset focus if current field is now hidden.
                        if !state.focused_field.is_visible_for(&state.order_type) {
                            state.focused_field = OrderField::Qty;
                        }
                    }
                    OrderField::TrailMode => {
                        state.trail_type = state.trail_type.toggle();
                    }
                    OrderField::ExtendedHours => {
                        state.extended_hours = !state.extended_hours;
                    }
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
                    OrderField::OrderType => {
                        state.order_type = if key.code == KeyCode::Up {
                            state.order_type.cycle_prev()
                        } else {
                            state.order_type.cycle_next()
                        };
                        if !state.focused_field.is_visible_for(&state.order_type) {
                            state.focused_field = OrderField::Qty;
                        }
                    }
                    OrderField::TrailMode => {
                        state.trail_type = state.trail_type.toggle();
                    }
                    OrderField::ExtendedHours => {
                        state.extended_hours = !state.extended_hours;
                    }
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
                    OrderField::StopPrice if c.is_ascii_digit() || c == '.' => {
                        state.stop_price_input.push(c);
                    }
                    OrderField::TrailAmount if c.is_ascii_digit() || c == '.' => {
                        state.trail_input.push(c);
                    }
                    OrderField::Side => {
                        if c == 'b' || c == 'B' {
                            state.side = OrderSide::Buy;
                        } else if c == 's' || c == 'S' {
                            state.side = OrderSide::Sell;
                        }
                    }
                    OrderField::ExtendedHours if c == ' ' => {
                        state.extended_hours = !state.extended_hours;
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
                    OrderField::StopPrice => {
                        state.stop_price_input.pop();
                    }
                    OrderField::TrailAmount => {
                        state.trail_input.pop();
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
                        let extended_hours_ok = app
                            .clock
                            .as_ref()
                            .map(|c| {
                                use crate::types::MarketState;
                                matches!(
                                    c.market_state(),
                                    MarketState::PreMarket | MarketState::AfterHours
                                )
                            })
                            .unwrap_or(false);
                        if let Some(err) = crate::input::validate(
                            &state,
                            buying_power,
                            market_open,
                            extended_hours_ok,
                        ) {
                            app.push_status(StatusMessage::persistent(err));
                            app.modal = Some(Modal::OrderEntry(state));
                            return;
                        }
                        // Build request fields based on order type (only legal fields).
                        let (limit_price, stop_price, trail_price, trail_percent) =
                            match &state.order_type {
                                FullOrderType::Market => (None, None, None, None),
                                FullOrderType::Limit => {
                                    let p = if state.price_input.is_empty() {
                                        None
                                    } else {
                                        Some(state.price_input.clone())
                                    };
                                    (p, None, None, None)
                                }
                                FullOrderType::Stop => {
                                    let sp = if state.stop_price_input.is_empty() {
                                        None
                                    } else {
                                        Some(state.stop_price_input.clone())
                                    };
                                    (None, sp, None, None)
                                }
                                FullOrderType::StopLimit => {
                                    let p = if state.price_input.is_empty() {
                                        None
                                    } else {
                                        Some(state.price_input.clone())
                                    };
                                    let sp = if state.stop_price_input.is_empty() {
                                        None
                                    } else {
                                        Some(state.stop_price_input.clone())
                                    };
                                    (p, sp, None, None)
                                }
                                FullOrderType::TrailingStop => {
                                    let amount = if state.trail_input.is_empty() {
                                        None
                                    } else {
                                        Some(state.trail_input.clone())
                                    };
                                    match state.trail_type {
                                        TrailType::Price => (None, None, amount, None),
                                        TrailType::Percent => (None, None, None, amount),
                                    }
                                }
                            };
                        send_command(
                            app,
                            Command::SubmitOrder {
                                symbol: state.symbol.clone(),
                                side: state.side.as_str().into(),
                                order_type: state.order_type.as_str().into(),
                                qty: if state.qty_input.is_empty() {
                                    None
                                } else {
                                    Some(state.qty_input.clone())
                                },
                                limit_price,
                                stop_price,
                                trail_price,
                                trail_percent,
                                time_in_force: if state.gtc_order { "gtc" } else { "day" }.into(),
                                extended_hours: state.extended_hours
                                    && state.order_type == FullOrderType::Limit,
                            },
                            "Submitting order…",
                        );
                        app.modal = None;
                        return;
                    } else {
                        state.focused_field = state.next_field();
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

        Modal::SetAlert {
            symbol,
            mut above_input,
            mut below_input,
            mut focused,
        } => match key.code {
            // Tab / Shift-Tab toggle between Above and Below inputs.
            KeyCode::Tab | KeyCode::BackTab => {
                focused = focused.toggle();
                Some(Modal::SetAlert {
                    symbol,
                    above_input,
                    below_input,
                    focused,
                })
            }
            // Up / Down also switch focus.
            KeyCode::Up => {
                focused = AlertField::Above;
                Some(Modal::SetAlert {
                    symbol,
                    above_input,
                    below_input,
                    focused,
                })
            }
            KeyCode::Down => {
                focused = AlertField::Below;
                Some(Modal::SetAlert {
                    symbol,
                    above_input,
                    below_input,
                    focused,
                })
            }
            // Digits and decimal point are accepted in the active field.
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                match focused {
                    AlertField::Above => above_input.push(c),
                    AlertField::Below => below_input.push(c),
                }
                Some(Modal::SetAlert {
                    symbol,
                    above_input,
                    below_input,
                    focused,
                })
            }
            KeyCode::Backspace => {
                match focused {
                    AlertField::Above => {
                        above_input.pop();
                    }
                    AlertField::Below => {
                        below_input.pop();
                    }
                }
                Some(Modal::SetAlert {
                    symbol,
                    above_input,
                    below_input,
                    focused,
                })
            }
            // Enter saves the alert (at least one threshold must be set).
            KeyCode::Enter => {
                let above = above_input.parse::<f64>().ok();
                let below = below_input.parse::<f64>().ok();
                if above.is_none() && below.is_none() {
                    // Both fields empty — remove any existing alert for this symbol.
                    app.price_alerts.remove(&symbol);
                    app.push_transient_status(format!("Alert cleared for {symbol}"));
                } else {
                    let alert = PriceAlert {
                        above,
                        below,
                        above_triggered: false,
                        below_triggered: false,
                    };
                    app.price_alerts.insert(symbol.clone(), alert);
                    let mut parts: Vec<String> = Vec::new();
                    if let Some(a) = above {
                        parts.push(format!("above ${:.2}", a));
                    }
                    if let Some(b) = below {
                        parts.push(format!("below ${:.2}", b));
                    }
                    app.push_transient_status(format!(
                        "🔔 Alert set for {symbol}: {}",
                        parts.join(", ")
                    ));
                }
                None // close the modal
            }
            _ => Some(Modal::SetAlert {
                symbol,
                above_input,
                below_input,
                focused,
            }),
        },
    };

    app.modal = new_modal;
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::app::test_helpers::{make_test_app, make_watchlist};
    use crate::app::{ConfirmAction, Modal};

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

    // ── OrderEntry modal ──────────────────────────────────────────────────────

    use crate::app::{FullOrderType, OrderEntryState, OrderField, TrailType};

    fn make_order_entry(field: OrderField) -> crate::app::App {
        let mut app = make_test_app();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = field;
        app.modal = Some(Modal::OrderEntry(state));
        app
    }

    fn order_entry_state(app: &crate::app::App) -> &OrderEntryState {
        match app.modal.as_ref().unwrap() {
            Modal::OrderEntry(s) => s,
            _ => panic!("expected OrderEntry modal"),
        }
    }

    #[test]
    fn esc_closes_order_entry_modal() {
        let mut app = make_order_entry(OrderField::Symbol);
        press(&mut app, KeyCode::Esc);
        assert!(app.modal.is_none(), "Esc should close OrderEntry");
    }

    #[test]
    fn tab_advances_focus_from_symbol() {
        let mut app = make_order_entry(OrderField::Symbol);
        press(&mut app, KeyCode::Tab);
        let s = order_entry_state(&app);
        assert_ne!(s.focused_field, OrderField::Symbol, "Tab should move focus");
    }

    #[test]
    fn backtab_retreats_focus() {
        let mut app = make_order_entry(OrderField::Qty);
        press(&mut app, KeyCode::BackTab);
        let s = order_entry_state(&app);
        // Should have moved backwards from Qty
        assert_ne!(s.focused_field, OrderField::Qty, "BackTab should retreat");
    }

    // Symbol field: char input
    #[test]
    fn char_appends_to_symbol() {
        let mut app = make_order_entry(OrderField::Symbol);
        press(&mut app, KeyCode::Char('a'));
        let s = order_entry_state(&app);
        assert!(
            s.symbol.ends_with('a'),
            "char should append as-is to symbol (got: {})",
            s.symbol
        );
    }

    #[test]
    fn backspace_removes_from_symbol() {
        let mut app = make_order_entry(OrderField::Symbol);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.symbol = "APPL".into();
        }
        press(&mut app, KeyCode::Backspace);
        assert_eq!(order_entry_state(&app).symbol, "APP");
    }

    // Qty field: char input
    #[test]
    fn char_appends_to_qty() {
        let mut app = make_order_entry(OrderField::Qty);
        press(&mut app, KeyCode::Char('5'));
        assert!(order_entry_state(&app).qty_input.ends_with('5'));
    }

    #[test]
    fn backspace_removes_from_qty() {
        let mut app = make_order_entry(OrderField::Qty);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.qty_input = "10".into();
        }
        press(&mut app, KeyCode::Backspace);
        assert_eq!(order_entry_state(&app).qty_input, "1");
    }

    // Price field: char input
    #[test]
    fn char_appends_to_price() {
        let mut app = make_order_entry(OrderField::Price);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.order_type = FullOrderType::Limit;
        }
        press(&mut app, KeyCode::Char('3'));
        assert!(order_entry_state(&app).price_input.ends_with('3'));
    }

    #[test]
    fn backspace_removes_from_price() {
        let mut app = make_order_entry(OrderField::Price);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.order_type = FullOrderType::Limit;
            s.price_input = "100".into();
        }
        press(&mut app, KeyCode::Backspace);
        assert_eq!(order_entry_state(&app).price_input, "10");
    }

    // StopPrice field
    #[test]
    fn char_appends_to_stop_price() {
        let mut app = make_order_entry(OrderField::StopPrice);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.order_type = FullOrderType::Stop;
        }
        press(&mut app, KeyCode::Char('9'));
        assert!(order_entry_state(&app).stop_price_input.ends_with('9'));
    }

    #[test]
    fn backspace_removes_from_stop_price() {
        let mut app = make_order_entry(OrderField::StopPrice);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.order_type = FullOrderType::Stop;
            s.stop_price_input = "200".into();
        }
        press(&mut app, KeyCode::Backspace);
        assert_eq!(order_entry_state(&app).stop_price_input, "20");
    }

    // TrailAmount field
    #[test]
    fn char_appends_to_trail_amount() {
        let mut app = make_order_entry(OrderField::TrailAmount);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.order_type = FullOrderType::TrailingStop;
        }
        press(&mut app, KeyCode::Char('2'));
        assert!(order_entry_state(&app).trail_input.ends_with('2'));
    }

    #[test]
    fn backspace_removes_from_trail_amount() {
        let mut app = make_order_entry(OrderField::TrailAmount);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.order_type = FullOrderType::TrailingStop;
            s.trail_input = "5.5".into();
        }
        press(&mut app, KeyCode::Backspace);
        assert_eq!(order_entry_state(&app).trail_input, "5.");
    }

    // OrderType cycling via Left/Right
    #[test]
    fn right_cycles_order_type_forward() {
        let mut app = make_order_entry(OrderField::OrderType);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.order_type = FullOrderType::Market;
        }
        press(&mut app, KeyCode::Right);
        assert_eq!(order_entry_state(&app).order_type, FullOrderType::Limit);
    }

    #[test]
    fn left_cycles_order_type_backward() {
        let mut app = make_order_entry(OrderField::OrderType);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.order_type = FullOrderType::Limit;
        }
        press(&mut app, KeyCode::Left);
        assert_eq!(order_entry_state(&app).order_type, FullOrderType::Market);
    }

    #[test]
    fn down_cycles_order_type_forward_on_order_type_field() {
        let mut app = make_order_entry(OrderField::OrderType);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.order_type = FullOrderType::Stop;
        }
        press(&mut app, KeyCode::Down);
        assert_eq!(order_entry_state(&app).order_type, FullOrderType::StopLimit);
    }

    #[test]
    fn up_cycles_order_type_backward_on_order_type_field() {
        let mut app = make_order_entry(OrderField::OrderType);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.order_type = FullOrderType::Stop;
        }
        press(&mut app, KeyCode::Up);
        assert_eq!(order_entry_state(&app).order_type, FullOrderType::Limit);
    }

    // TrailMode toggle
    #[test]
    fn left_right_toggle_trail_mode() {
        let mut app = make_order_entry(OrderField::TrailMode);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.trail_type = TrailType::Price;
        }
        press(&mut app, KeyCode::Right);
        assert_eq!(order_entry_state(&app).trail_type, TrailType::Percent);
        press(&mut app, KeyCode::Left);
        assert_eq!(order_entry_state(&app).trail_type, TrailType::Price);
    }

    // ExtendedHours toggle
    #[test]
    fn space_toggles_extended_hours() {
        let mut app = make_order_entry(OrderField::ExtendedHours);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.extended_hours = false;
        }
        press(&mut app, KeyCode::Char(' '));
        assert!(
            order_entry_state(&app).extended_hours,
            "space should set extended_hours"
        );
        press(&mut app, KeyCode::Char(' '));
        assert!(
            !order_entry_state(&app).extended_hours,
            "second space clears extended_hours"
        );
    }

    #[test]
    fn enter_on_extended_hours_advances_focus() {
        let mut app = make_order_entry(OrderField::ExtendedHours);
        press(&mut app, KeyCode::Enter);
        // Enter on non-Submit field advances focus, not toggles
        let s = order_entry_state(&app);
        assert_ne!(
            s.focused_field,
            OrderField::ExtendedHours,
            "Enter should advance focus"
        );
        assert!(app.modal.is_some());
    }

    // Side cycling
    #[test]
    fn left_right_cycle_side() {
        let mut app = make_order_entry(OrderField::Side);
        press(&mut app, KeyCode::Right);
        let s = order_entry_state(&app);
        // Should have cycled from default (Buy)
        assert_ne!(
            s.side,
            crate::app::OrderSide::Buy,
            "Right should cycle side away from Buy"
        );
    }

    // TimeInForce cycling
    #[test]
    fn left_toggles_tif_to_day() {
        let mut app = make_order_entry(OrderField::TimeInForce);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.gtc_order = true;
        }
        press(&mut app, KeyCode::Left);
        assert!(!order_entry_state(&app).gtc_order);
    }

    #[test]
    fn right_toggles_tif_to_gtc() {
        let mut app = make_order_entry(OrderField::TimeInForce);
        press(&mut app, KeyCode::Right);
        assert!(order_entry_state(&app).gtc_order);
    }

    // Tab skips hidden fields
    #[test]
    fn tab_skips_price_for_market_order() {
        let mut app = make_order_entry(OrderField::Qty);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.order_type = FullOrderType::Market;
        }
        press(&mut app, KeyCode::Tab);
        // Should skip Price, StopPrice, TrailAmount, TrailMode, ExtendedHours → TimeInForce
        assert_eq!(
            order_entry_state(&app).focused_field,
            OrderField::TimeInForce
        );
    }

    // Enter advances through fields (does not submit until Submit is focused)
    #[test]
    fn enter_on_non_submit_field_advances_focus() {
        let mut app = make_order_entry(OrderField::Symbol);
        press(&mut app, KeyCode::Enter);
        let s = order_entry_state(&app);
        assert_ne!(
            s.focused_field,
            OrderField::Symbol,
            "Enter on non-Submit field should advance"
        );
        assert!(app.modal.is_some(), "modal should still be open");
    }

    // Unhandled key on generic field is a no-op
    #[test]
    fn unhandled_key_on_order_entry_is_noop() {
        let mut app = make_order_entry(OrderField::Qty);
        press(&mut app, KeyCode::F(1));
        // Modal should still be open
        assert!(app.modal.is_some(), "F1 should not close order entry");
    }

    // Non-digit char on Qty is a no-op
    #[test]
    fn non_digit_char_on_qty_is_noop() {
        let mut app = make_order_entry(OrderField::Qty);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.qty_input = "10".into();
        }
        press(&mut app, KeyCode::Char('x'));
        assert_eq!(
            order_entry_state(&app).qty_input,
            "10",
            "non-digit char should not change qty input"
        );
    }

    // b/B/s/S shortcut on Side field
    #[test]
    fn b_key_sets_side_to_buy_on_side_field() {
        let mut app = make_order_entry(OrderField::Side);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.side = crate::app::OrderSide::Sell;
        }
        press(&mut app, KeyCode::Char('b'));
        assert_eq!(order_entry_state(&app).side, crate::app::OrderSide::Buy);
    }

    #[test]
    fn upper_b_key_sets_side_to_buy_on_side_field() {
        let mut app = make_order_entry(OrderField::Side);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.side = crate::app::OrderSide::Sell;
        }
        press(&mut app, KeyCode::Char('B'));
        assert_eq!(order_entry_state(&app).side, crate::app::OrderSide::Buy);
    }

    #[test]
    fn s_key_sets_side_to_sell_on_side_field() {
        let mut app = make_order_entry(OrderField::Side);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.side = crate::app::OrderSide::Buy;
        }
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(order_entry_state(&app).side, crate::app::OrderSide::Sell);
    }

    #[test]
    fn upper_s_key_sets_side_to_sell_on_side_field() {
        let mut app = make_order_entry(OrderField::Side);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.side = crate::app::OrderSide::Buy;
        }
        press(&mut app, KeyCode::Char('S'));
        assert_eq!(order_entry_state(&app).side, crate::app::OrderSide::Sell);
    }

    // Up/Down on Side cycles side
    #[test]
    fn up_key_cycles_side_backward() {
        let mut app = make_order_entry(OrderField::Side);
        // default side is Buy; Up should cycle backward (away from Buy)
        press(&mut app, KeyCode::Up);
        assert_ne!(
            order_entry_state(&app).side,
            crate::app::OrderSide::Buy,
            "Up should cycle side backward from Buy"
        );
    }

    #[test]
    fn down_key_cycles_side_forward() {
        let mut app = make_order_entry(OrderField::Side);
        press(&mut app, KeyCode::Down);
        assert_ne!(
            order_entry_state(&app).side,
            crate::app::OrderSide::Buy,
            "Down should cycle side forward from Buy"
        );
    }

    // Up/Down on TrailMode toggles trail_type
    #[test]
    fn up_down_toggle_trail_mode_via_up_down() {
        let mut app = make_order_entry(OrderField::TrailMode);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.trail_type = TrailType::Price;
        }
        press(&mut app, KeyCode::Down);
        assert_eq!(order_entry_state(&app).trail_type, TrailType::Percent);
        press(&mut app, KeyCode::Up);
        assert_eq!(order_entry_state(&app).trail_type, TrailType::Price);
    }

    // Up/Down on ExtendedHours toggles extended_hours
    #[test]
    fn up_down_toggle_extended_hours_via_up_down() {
        let mut app = make_order_entry(OrderField::ExtendedHours);
        {
            let Modal::OrderEntry(ref mut s) = app.modal.as_mut().unwrap() else {
                panic!()
            };
            s.extended_hours = false;
        }
        press(&mut app, KeyCode::Down);
        assert!(
            order_entry_state(&app).extended_hours,
            "Down should toggle extended_hours on"
        );
        press(&mut app, KeyCode::Up);
        assert!(
            !order_entry_state(&app).extended_hours,
            "Up should toggle extended_hours off"
        );
    }

    // Up/Down on TimeInForce toggles gtc_order
    #[test]
    fn up_down_toggle_tif_via_up_down() {
        let mut app = make_order_entry(OrderField::TimeInForce);
        press(&mut app, KeyCode::Down);
        assert!(
            order_entry_state(&app).gtc_order,
            "Down should toggle gtc_order on"
        );
        press(&mut app, KeyCode::Up);
        assert!(
            !order_entry_state(&app).gtc_order,
            "Up should toggle gtc_order off"
        );
    }

    // ── About modal ───────────────────────────────────────────────────────────

    #[test]
    fn any_key_closes_about_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::About);
        press(&mut app, KeyCode::Enter);
        assert!(app.modal.is_none(), "any key should close About modal");
    }

    // ── Confirm modal ─────────────────────────────────────────────────────────

    #[test]
    fn y_key_confirms_and_closes_confirm_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Confirm {
            message: "Cancel order?".into(),
            action: ConfirmAction::CancelOrder("ord-1".into()),
            confirmed: false,
        });
        press(&mut app, KeyCode::Char('y'));
        assert!(app.modal.is_none(), "y should close the Confirm modal");
    }

    #[test]
    fn left_key_confirms_and_closes_confirm_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Confirm {
            message: "Cancel?".into(),
            action: ConfirmAction::CancelOrder("ord-2".into()),
            confirmed: false,
        });
        press(&mut app, KeyCode::Left);
        assert!(app.modal.is_none(), "Left should close the Confirm modal");
    }

    #[test]
    fn n_key_closes_confirm_modal_unconfirmed() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Confirm {
            message: "Cancel?".into(),
            action: ConfirmAction::CancelOrder("ord-3".into()),
            confirmed: false,
        });
        press(&mut app, KeyCode::Char('n'));
        assert!(
            app.modal.is_none(),
            "n should close the Confirm modal (unconfirmed)"
        );
    }

    #[test]
    fn right_key_closes_confirm_modal_unconfirmed() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Confirm {
            message: "Cancel?".into(),
            action: ConfirmAction::CancelOrder("ord-4".into()),
            confirmed: false,
        });
        press(&mut app, KeyCode::Right);
        assert!(
            app.modal.is_none(),
            "Right should close the Confirm modal (unconfirmed)"
        );
    }

    #[test]
    fn enter_on_confirm_modal_when_not_confirmed_keeps_modal_open() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Confirm {
            message: "Cancel?".into(),
            action: ConfirmAction::CancelOrder("ord-5".into()),
            confirmed: false,
        });
        press(&mut app, KeyCode::Enter);
        assert!(
            matches!(app.modal, Some(Modal::Confirm { .. })),
            "Enter with confirmed=false should keep Confirm modal open"
        );
    }

    #[test]
    fn enter_on_confirm_modal_when_confirmed_closes_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Confirm {
            message: "Cancel?".into(),
            action: ConfirmAction::CancelOrder("ord-6".into()),
            confirmed: true,
        });
        press(&mut app, KeyCode::Enter);
        assert!(
            app.modal.is_none(),
            "Enter with confirmed=true should close Confirm modal"
        );
    }

    #[test]
    fn unhandled_key_keeps_confirm_modal_open() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Confirm {
            message: "Cancel?".into(),
            action: ConfirmAction::CancelOrder("ord-7".into()),
            confirmed: false,
        });
        press(&mut app, KeyCode::F(5));
        assert!(
            matches!(app.modal, Some(Modal::Confirm { .. })),
            "unhandled key should keep Confirm modal open"
        );
    }

    // ── ConfirmRemoveWatchlist modal ──────────────────────────────────────────

    #[test]
    fn y_key_confirms_and_closes_confirm_remove_watchlist_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::ConfirmRemoveWatchlist {
            symbol: "AAPL".into(),
            watchlist_id: "wl-1".into(),
        });
        press(&mut app, KeyCode::Char('y'));
        assert!(app.modal.is_none(), "y should close ConfirmRemoveWatchlist");
    }

    #[test]
    fn enter_key_confirms_and_closes_confirm_remove_watchlist_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::ConfirmRemoveWatchlist {
            symbol: "TSLA".into(),
            watchlist_id: "wl-1".into(),
        });
        press(&mut app, KeyCode::Enter);
        assert!(
            app.modal.is_none(),
            "Enter should close ConfirmRemoveWatchlist"
        );
    }

    #[test]
    fn n_key_cancels_confirm_remove_watchlist_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::ConfirmRemoveWatchlist {
            symbol: "NVDA".into(),
            watchlist_id: "wl-2".into(),
        });
        press(&mut app, KeyCode::Char('n'));
        assert!(
            app.modal.is_none(),
            "n should close ConfirmRemoveWatchlist without confirming"
        );
    }

    #[test]
    fn unhandled_key_keeps_confirm_remove_watchlist_modal_open() {
        let mut app = make_test_app();
        app.modal = Some(Modal::ConfirmRemoveWatchlist {
            symbol: "MSFT".into(),
            watchlist_id: "wl-3".into(),
        });
        press(&mut app, KeyCode::Char('z'));
        assert!(
            matches!(app.modal, Some(Modal::ConfirmRemoveWatchlist { .. })),
            "unhandled key should keep ConfirmRemoveWatchlist open"
        );
    }

    // ── AddSymbol modal ───────────────────────────────────────────────────────

    #[test]
    fn char_appends_uppercase_to_add_symbol_input() {
        let mut app = make_test_app();
        app.modal = Some(Modal::AddSymbol {
            input: "AAP".into(),
            watchlist_id: "wl-1".into(),
        });
        press(&mut app, KeyCode::Char('l'));
        match &app.modal {
            Some(Modal::AddSymbol { input, .. }) => {
                assert_eq!(input, "AAPL", "char should be uppercased and appended")
            }
            _ => panic!("expected AddSymbol modal"),
        }
    }

    #[test]
    fn backspace_removes_from_add_symbol_input() {
        let mut app = make_test_app();
        app.modal = Some(Modal::AddSymbol {
            input: "AAPL".into(),
            watchlist_id: "wl-1".into(),
        });
        press(&mut app, KeyCode::Backspace);
        match &app.modal {
            Some(Modal::AddSymbol { input, .. }) => assert_eq!(input, "AAP"),
            _ => panic!("expected AddSymbol modal"),
        }
    }

    #[test]
    fn enter_with_non_empty_input_closes_add_symbol_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::AddSymbol {
            input: "AAPL".into(),
            watchlist_id: "wl-1".into(),
        });
        press(&mut app, KeyCode::Enter);
        assert!(
            app.modal.is_none(),
            "Enter with non-empty input should close AddSymbol"
        );
    }

    #[test]
    fn enter_with_empty_input_closes_add_symbol_modal_without_command() {
        let mut app = make_test_app();
        app.modal = Some(Modal::AddSymbol {
            input: "".into(),
            watchlist_id: "wl-1".into(),
        });
        press(&mut app, KeyCode::Enter);
        assert!(
            app.modal.is_none(),
            "Enter with empty input should close AddSymbol (no command sent)"
        );
    }

    #[test]
    fn unhandled_key_keeps_add_symbol_modal_open() {
        let mut app = make_test_app();
        app.modal = Some(Modal::AddSymbol {
            input: "TS".into(),
            watchlist_id: "wl-1".into(),
        });
        press(&mut app, KeyCode::F(1));
        assert!(
            matches!(app.modal, Some(Modal::AddSymbol { .. })),
            "unhandled key should keep AddSymbol open"
        );
    }

    // ── GlobalSearch modal ────────────────────────────────────────────────────

    #[test]
    fn char_appends_uppercase_to_global_search_query() {
        let mut app = make_test_app();
        app.modal = Some(Modal::GlobalSearch {
            query: "AAP".into(),
        });
        press(&mut app, KeyCode::Char('l'));
        match &app.modal {
            Some(Modal::GlobalSearch { query }) => {
                assert_eq!(
                    query, "AAPL",
                    "char should be uppercased and appended to query"
                )
            }
            _ => panic!("expected GlobalSearch modal"),
        }
    }

    #[test]
    fn backspace_removes_from_global_search_query() {
        let mut app = make_test_app();
        app.modal = Some(Modal::GlobalSearch {
            query: "TSLA".into(),
        });
        press(&mut app, KeyCode::Backspace);
        match &app.modal {
            Some(Modal::GlobalSearch { query }) => assert_eq!(query, "TSL"),
            _ => panic!("expected GlobalSearch modal"),
        }
    }

    #[test]
    fn enter_with_non_empty_query_transitions_to_symbol_detail() {
        let mut app = make_test_app();
        app.modal = Some(Modal::GlobalSearch {
            query: "NVDA".into(),
        });
        press(&mut app, KeyCode::Enter);
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "NVDA"),
            "Enter with non-empty query should switch to SymbolDetail; got: {:?}",
            app.modal
        );
    }

    #[test]
    fn enter_with_empty_query_closes_global_search_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::GlobalSearch { query: "".into() });
        press(&mut app, KeyCode::Enter);
        assert!(
            app.modal.is_none(),
            "Enter with empty query should close GlobalSearch"
        );
    }

    #[test]
    fn unhandled_key_keeps_global_search_modal_open() {
        let mut app = make_test_app();
        app.modal = Some(Modal::GlobalSearch { query: "NV".into() });
        press(&mut app, KeyCode::F(3));
        assert!(
            matches!(&app.modal, Some(Modal::GlobalSearch { query }) if query == "NV"),
            "unhandled key should keep GlobalSearch open; got: {:?}",
            app.modal
        );
    }

    // ── SymbolDetail additional keys ─────────────────────────────────────────

    #[test]
    fn o_key_in_symbol_detail_opens_buy_order_entry() {
        let mut app = make_test_app();
        app.modal = Some(Modal::SymbolDetail("TSLA".into()));
        press(&mut app, KeyCode::Char('o'));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "TSLA"),
            "o key should open OrderEntry for symbol; got: {:?}",
            app.modal
        );
    }

    #[test]
    fn s_key_in_symbol_detail_opens_sell_order_entry() {
        let mut app = make_test_app();
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        press(&mut app, KeyCode::Char('s'));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.side == crate::app::OrderSide::Sell),
            "s key should open OrderEntry with Sell side; got: {:?}",
            app.modal
        );
    }

    #[test]
    fn w_key_in_symbol_detail_with_no_watchlist_keeps_modal_open() {
        let mut app = make_test_app();
        app.watchlist = None;
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        press(&mut app, KeyCode::Char('w'));
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "AAPL"),
            "w with no watchlist should keep SymbolDetail open; got: {:?}",
            app.modal
        );
    }

    #[test]
    fn w_key_in_symbol_detail_when_in_watchlist_keeps_modal_open() {
        let mut app = make_test_app();
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        press(&mut app, KeyCode::Char('w'));
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "AAPL"),
            "w on watchlist symbol should keep SymbolDetail open; got: {:?}",
            app.modal
        );
    }

    #[test]
    fn w_key_in_symbol_detail_when_not_in_watchlist_keeps_modal_open() {
        let mut app = make_test_app();
        app.watchlist = Some(make_watchlist(&["TSLA"]));
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        press(&mut app, KeyCode::Char('w'));
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "AAPL"),
            "w on non-watchlist symbol should keep SymbolDetail open; got: {:?}",
            app.modal
        );
    }

    // ── SetAlert modal ────────────────────────────────────────────────────────

    fn make_set_alert_app(symbol: &str) -> crate::app::App {
        let mut app = make_test_app();
        app.modal = Some(Modal::SetAlert {
            symbol: symbol.into(),
            above_input: String::new(),
            below_input: String::new(),
            focused: crate::app::AlertField::Above,
        });
        app
    }

    #[test]
    fn set_alert_digit_appends_to_above_field() {
        let mut app = make_set_alert_app("AAPL");
        press(&mut app, KeyCode::Char('1'));
        press(&mut app, KeyCode::Char('5'));
        press(&mut app, KeyCode::Char('0'));
        match &app.modal {
            Some(Modal::SetAlert { above_input, .. }) => {
                assert_eq!(above_input, "150");
            }
            other => panic!("expected SetAlert, got: {:?}", other),
        }
    }

    #[test]
    fn set_alert_tab_switches_focus_to_below() {
        let mut app = make_set_alert_app("AAPL");
        press(&mut app, KeyCode::Tab);
        match &app.modal {
            Some(Modal::SetAlert { focused, .. }) => {
                assert_eq!(*focused, crate::app::AlertField::Below);
            }
            other => panic!("expected SetAlert, got: {:?}", other),
        }
    }

    #[test]
    fn set_alert_digit_appends_to_below_field_after_tab() {
        let mut app = make_set_alert_app("AAPL");
        press(&mut app, KeyCode::Tab); // switch to Below
        press(&mut app, KeyCode::Char('1'));
        press(&mut app, KeyCode::Char('4'));
        press(&mut app, KeyCode::Char('9'));
        match &app.modal {
            Some(Modal::SetAlert { below_input, .. }) => {
                assert_eq!(below_input, "149");
            }
            other => panic!("expected SetAlert, got: {:?}", other),
        }
    }

    #[test]
    fn set_alert_backspace_removes_from_active_field() {
        let mut app = make_set_alert_app("AAPL");
        press(&mut app, KeyCode::Char('2'));
        press(&mut app, KeyCode::Char('0'));
        press(&mut app, KeyCode::Char('0'));
        press(&mut app, KeyCode::Backspace);
        match &app.modal {
            Some(Modal::SetAlert { above_input, .. }) => {
                assert_eq!(above_input, "20");
            }
            other => panic!("expected SetAlert, got: {:?}", other),
        }
    }

    #[test]
    fn set_alert_enter_saves_alert_and_closes_modal() {
        let mut app = make_set_alert_app("AAPL");
        press(&mut app, KeyCode::Char('2'));
        press(&mut app, KeyCode::Char('0'));
        press(&mut app, KeyCode::Char('0'));
        press(&mut app, KeyCode::Enter);
        assert!(
            app.modal.is_none(),
            "modal should close after Enter; got: {:?}",
            app.modal
        );
        let alert = app.price_alerts.get("AAPL").expect("alert should be saved");
        assert_eq!(alert.above, Some(200.0));
        assert_eq!(alert.below, None);
    }

    #[test]
    fn set_alert_enter_with_empty_inputs_clears_alert() {
        let mut app = make_set_alert_app("AAPL");
        // Pre-insert an alert, then confirm with empty inputs → should clear it.
        app.price_alerts.insert(
            "AAPL".into(),
            crate::types::PriceAlert {
                above: Some(200.0),
                ..Default::default()
            },
        );
        press(&mut app, KeyCode::Enter); // inputs are empty
        assert!(
            app.modal.is_none(),
            "modal should close after Enter"
        );
        assert!(
            app.price_alerts.get("AAPL").is_none(),
            "alert should be cleared when both inputs are empty"
        );
    }

    #[test]
    fn set_alert_esc_closes_modal_without_saving() {
        let mut app = make_set_alert_app("AAPL");
        press(&mut app, KeyCode::Char('9'));
        press(&mut app, KeyCode::Char('9'));
        press(&mut app, KeyCode::Esc);
        assert!(
            app.modal.is_none(),
            "Esc should close SetAlert without saving"
        );
        assert!(
            app.price_alerts.get("AAPL").is_none(),
            "no alert should be saved after Esc"
        );
    }

    #[test]
    fn set_alert_decimal_accepted_in_above_field() {
        let mut app = make_set_alert_app("AAPL");
        press(&mut app, KeyCode::Char('1'));
        press(&mut app, KeyCode::Char('8'));
        press(&mut app, KeyCode::Char('5'));
        press(&mut app, KeyCode::Char('.'));
        press(&mut app, KeyCode::Char('5'));
        press(&mut app, KeyCode::Char('0'));
        press(&mut app, KeyCode::Enter);
        let alert = app.price_alerts.get("AAPL").expect("alert saved");
        assert!(
            (alert.above.unwrap() - 185.50).abs() < 0.001,
            "expected 185.50, got: {:?}",
            alert.above
        );
    }
}

