use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, ConfirmAction, Modal, OrderEntryState, OrderField, OrdersSubTab, Tab};
use crate::events::Event;

pub fn update(app: &mut App, event: Event) {
    match event {
        Event::Input(key) => handle_key(app, key),
        Event::Mouse(_) => {}
        Event::Resize(_, _) => {}

        Event::AccountUpdated(a) => {
            app.account = Some(a);
            app.push_equity();
        }
        Event::PositionsUpdated(p) => {
            app.positions = p;
            if app.positions_state.selected().is_none() && !app.positions.is_empty() {
                app.positions_state.select(Some(0));
            }
        }
        Event::OrdersUpdated(o) => {
            app.orders = o;
            if app.orders_state.selected().is_none() && !app.orders.is_empty() {
                app.orders_state.select(Some(0));
            }
        }
        Event::ClockUpdated(c) => app.clock = Some(c),
        Event::WatchlistUpdated(w) => {
            if app.watchlist_state.selected().is_none() && !w.assets.is_empty() {
                app.watchlist_state.select(Some(0));
            }
            app.watchlist = Some(w);
        }
        Event::MarketQuote(q) => {
            app.quotes.insert(q.symbol.clone(), q);
        }
        Event::TradeUpdate(o) => {
            if let Some(existing) = app.orders.iter_mut().find(|x| x.id == o.id) {
                *existing = o;
            } else {
                app.orders.insert(0, o);
            }
        }
        Event::StatusMsg(msg) => app.status_msg = msg,
        Event::Tick => {}
        Event::Quit => app.should_quit = true,
    }
}

fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) {
    // Modal takes full priority
    if app.modal.is_some() {
        handle_modal_key(app, key);
        return;
    }

    // Search mode intercepts printable keys
    if app.searching {
        handle_search_key(app, key);
        return;
    }

    // Global shortcuts
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true
        }
        KeyCode::Char('?') => app.modal = Some(Modal::Help),
        KeyCode::Char('1') => app.active_tab = Tab::Account,
        KeyCode::Char('2') => app.active_tab = Tab::Watchlist,
        KeyCode::Char('3') => app.active_tab = Tab::Positions,
        KeyCode::Char('4') => app.active_tab = Tab::Orders,
        KeyCode::Tab => app.active_tab = app.active_tab.next(),
        KeyCode::BackTab => app.active_tab = app.active_tab.prev(),
        KeyCode::Char('r') => {
            app.status_msg = "Refreshing…".into();
            app.refresh_notify.notify_one();
        }
        _ => handle_panel_key(app, key),
    }
}

fn handle_panel_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match app.active_tab.clone() {
        Tab::Account => {}
        Tab::Watchlist => handle_watchlist_key(app, key),
        Tab::Positions => handle_positions_key(app, key),
        Tab::Orders => handle_orders_key(app, key),
    }
}

fn handle_watchlist_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let len = app.watchlist.as_ref().map(|w| w.assets.len()).unwrap_or(0);

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                let i = app.watchlist_state.selected().unwrap_or(0);
                app.watchlist_state.select(Some((i + 1).min(len - 1)));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let i = app.watchlist_state.selected().unwrap_or(0);
            app.watchlist_state.select(Some(i.saturating_sub(1)));
        }
        KeyCode::Char('g') => app.watchlist_state.select(Some(0)),
        KeyCode::Char('G') => {
            if len > 0 {
                app.watchlist_state.select(Some(len - 1));
            }
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

fn handle_positions_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let len = app.positions.len();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                let i = app.positions_state.selected().unwrap_or(0);
                app.positions_state.select(Some((i + 1).min(len - 1)));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let i = app.positions_state.selected().unwrap_or(0);
            app.positions_state.select(Some(i.saturating_sub(1)));
        }
        KeyCode::Char('g') => app.positions_state.select(Some(0)),
        KeyCode::Char('G') => {
            if len > 0 {
                app.positions_state.select(Some(len - 1));
            }
        }
        KeyCode::Enter => {
            if let Some(symbol) = app.selected_position_symbol() {
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

fn handle_orders_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let orders = app.filtered_orders();
    let len = orders.len();

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if len > 0 {
                let i = app.orders_state.selected().unwrap_or(0);
                app.orders_state.select(Some((i + 1).min(len - 1)));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let i = app.orders_state.selected().unwrap_or(0);
            app.orders_state.select(Some(i.saturating_sub(1)));
        }
        KeyCode::Char('g') => app.orders_state.select(Some(0)),
        KeyCode::Char('G') => {
            if len > 0 {
                app.orders_state.select(Some(len - 1));
            }
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
                    message: format!("Cancel order {}?", &id[..8]),
                    action: ConfirmAction::CancelOrder(id),
                    confirmed: false,
                });
            }
        }
        _ => {}
    }
}

fn handle_modal_key(app: &mut App, key: crossterm::event::KeyEvent) {
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
                    OrderField::Qty => {
                        if c.is_ascii_digit() || c == '.' {
                            state.qty_input.push(c);
                        }
                    }
                    OrderField::Price => {
                        if c.is_ascii_digit() || c == '.' {
                            state.price_input.push(c);
                        }
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
                        // Order submission handled in main loop via command channel (Phase 2)
                        app.status_msg = "Order submission coming in Phase 2".into();
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
        } => {
            match key.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Char('y') | KeyCode::Char('n') => {
                    confirmed = matches!(key.code, KeyCode::Char('y') | KeyCode::Left);
                    if confirmed {
                        // Trigger action via notify + status (Phase 2: send command via channel)
                        match &action {
                            ConfirmAction::CancelOrder(id) => {
                                app.status_msg = format!("Cancelling order {}…", &id[..8]);
                            }
                            ConfirmAction::RemoveFromWatchlist { symbol, .. } => {
                                app.status_msg = format!("Removing {}…", symbol);
                            }
                        }
                        app.refresh_notify.notify_one();
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
            }
        }

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
                    app.status_msg = format!("Adding {}…", input);
                    app.refresh_notify.notify_one();
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

fn handle_search_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.searching = false;
        }
        KeyCode::Backspace => {
            app.search_query.pop();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.watchlist_state.select(Some(0));
        }
        _ => {}
    }
}
