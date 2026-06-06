use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, Modal, StatusMessage, Tab};
use crate::clipboard;
use crate::commands::Command;
use crate::events::{Event, StreamKind};
use crate::input::{
    handle_modal_key, handle_mouse, handle_orders_key, handle_positions_key, handle_search_key,
    handle_watchlist_key,
};

/// How long to wait before re-fetching intraday bars for an open symbol-detail modal.
const INTRADAY_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

pub fn update(app: &mut App, event: Event) {
    match event {
        Event::Input(key) => handle_key(app, key),
        Event::Mouse(m) => handle_mouse(app, m),
        Event::Resize(_, _) => {
            // Ratatui re-layouts on the next draw, but without an explicit
            // trigger the UI waits up to 250 ms for the next tick. Request
            // an immediate redraw so the layout adapts right away.
            app.needs_redraw = true;
        }

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
            // Push new symbol list to the market stream for resubscription
            let symbols: Vec<String> = w.assets.iter().map(|a| a.symbol.clone()).collect();
            let _ = app.symbol_tx.send(symbols);
            if app.watchlist_state.selected().is_none() && !w.assets.is_empty() {
                app.watchlist_state.select(Some(0));
            }
            app.watchlist = Some(w);
        }
        Event::WatchlistUnavailable => {
            app.watchlist_unavailable = true;
        }
        Event::MarketQuote(q) => {
            // Evaluate price alerts before inserting the new quote so we can
            // detect the crossing direction using the previous quote value.
            evaluate_price_alert(app, &q);
            app.quotes.insert(q.symbol.clone(), q);
            // Push a streaming equity sample between REST polls so the chart
            // reflects live price movement without extra API calls.
            app.push_equity_from_quotes();
        }
        Event::TradeUpdate {
            order: o,
            event_type,
        } => {
            if app.prefs.notifications.fill_notifications_enabled {
                if let Some(msg) = fill_notification_text(&o, &event_type) {
                    app.push_fill_notification(msg);
                }
            }
            if let Some(existing) = app.orders.iter_mut().find(|x| x.id == o.id) {
                *existing = o;
            } else {
                app.orders.insert(0, o);
            }
        }
        Event::StatusMsg(msg) => app.push_status(StatusMessage::persistent(msg)),
        Event::StreamConnected(kind) => match kind {
            StreamKind::Market => {
                app.market_stream_ok = true;
                app.market_stream_reconnecting = false;
                app.market_reconnect_attempt = 0;
            }
            StreamKind::Account => {
                app.account_stream_ok = true;
                app.account_stream_reconnecting = false;
                app.account_reconnect_attempt = 0;
            }
        },
        Event::StreamReconnecting { kind, attempt } => match kind {
            StreamKind::Market => {
                app.market_stream_ok = false;
                app.market_stream_reconnecting = true;
                app.market_reconnect_attempt = attempt;
            }
            StreamKind::Account => {
                app.account_stream_ok = false;
                app.account_stream_reconnecting = true;
                app.account_reconnect_attempt = attempt;
            }
        },
        Event::StreamDisconnected(kind) => match kind {
            StreamKind::Market => {
                app.market_stream_ok = false;
                app.market_stream_reconnecting = false;
            }
            StreamKind::Account => {
                app.account_stream_ok = false;
                app.account_stream_reconnecting = false;
            }
        },
        Event::PortfolioHistoryLoaded(data) => {
            // Convert dollar values → cents (u64) to match equity_history format.
            // Keep all samples; ongoing push_equity() calls will append new ones.
            app.equity_history = data.into_iter().map(|v| (v * 100.0) as u64).collect();
        }
        Event::SnapshotsUpdated(snapshots) => {
            app.snapshots = snapshots;
        }
        Event::IntradayBarsReceived { symbol, bars } => {
            // Record fetch time before storing bars so the Tick handler can
            // schedule the next periodic refresh from this instant.
            app.intraday_fetched_at
                .insert(symbol.clone(), Instant::now());
            app.intraday_bars.insert(symbol, bars);
        }
        Event::FetchStarted => app.request_started(),
        Event::FetchComplete => app.request_finished(),
        Event::Tick => {
            // Advance spinner frame while any fetch is in-flight.
            if app.pending_requests > 0 {
                app.tick_spinner();
            }
            // Pop the front entry if it has expired; keep popping while the
            // next front is also expired so stale messages never block fresh ones.
            loop {
                match app.status_queue.front() {
                    Some(m) if m.expires_at.is_some_and(|e| e <= Instant::now()) => {
                        app.status_queue.pop_front();
                    }
                    _ => break,
                }
            }
            // Periodically re-fetch intraday bars while a symbol/position-detail modal is open.
            let detail_symbol = match &app.modal {
                Some(Modal::SymbolDetail(s)) => Some(s.clone()),
                Some(Modal::PositionDetail { symbol }) => Some(symbol.clone()),
                _ => None,
            };
            if let Some(symbol) = detail_symbol {
                let due = app
                    .intraday_fetched_at
                    .get(&symbol)
                    .map(|t| t.elapsed() >= INTRADAY_REFRESH_INTERVAL)
                    .unwrap_or(false);
                if due {
                    let _ = app
                        .command_tx
                        .try_send(Command::FetchIntradayBars(symbol.clone()));
                }
            }
        }
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
        // 'A' opens the price-alert dialog when on the Watchlist tab;
        // on every other tab it opens the About screen.
        KeyCode::Char('A') if app.active_tab != Tab::Watchlist => app.modal = Some(Modal::About),
        // '1'/'2'/'3' switch panels globally, but yield to the Orders panel so those
        // keys can switch sub-tabs (Open / Filled / Cancelled) when Orders is active.
        KeyCode::Char('1') if app.active_tab != Tab::Orders => {
            app.active_tab = Tab::Account;
        }
        KeyCode::Char('2') if app.active_tab != Tab::Orders => {
            app.equity_chart_cursor = None;
            app.active_tab = Tab::Watchlist;
        }
        KeyCode::Char('3') if app.active_tab != Tab::Orders => {
            app.equity_chart_cursor = None;
            app.active_tab = Tab::Positions;
        }
        KeyCode::Char('4') => {
            app.equity_chart_cursor = None;
            app.active_tab = Tab::Orders;
        }
        KeyCode::Tab => {
            if app.active_tab != Tab::Account {
                app.equity_chart_cursor = None;
            }
            app.active_tab = app.active_tab.next();
        }
        KeyCode::BackTab => {
            if app.active_tab != Tab::Account {
                app.equity_chart_cursor = None;
            }
            app.active_tab = app.active_tab.prev();
        }
        KeyCode::Char('r') => {
            app.push_transient_status("Refreshing…");
            app.refresh_notify.notify_one();
        }
        KeyCode::Char('T') => {
            app.cycle_theme();
            app.push_transient_status(format!("Theme: {}", app.current_theme.display_name()));
        }
        // Copy focused symbol to clipboard (Watchlist, Positions, Orders)
        KeyCode::Char('c') if app.active_tab != Tab::Orders => {
            copy_focused_symbol(app);
        }
        // Global symbol search: Ctrl-F from any tab, or '/' from non-Watchlist tabs.
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.modal = Some(Modal::GlobalSearch {
                query: String::new(),
            });
        }
        KeyCode::Char('/') if app.active_tab != Tab::Watchlist => {
            app.modal = Some(Modal::GlobalSearch {
                query: String::new(),
            });
        }
        _ => handle_panel_key(app, key),
    }
}

/// Copies the currently focused symbol to the system clipboard and sets a
/// transient status message.  If no row is selected, or the clipboard is
/// unavailable, an informational status is shown instead.
fn copy_focused_symbol(app: &mut App) {
    match app.focused_symbol() {
        None => app.push_transient_status("No symbol selected"),
        Some(symbol) => match clipboard::copy_to_clipboard(&symbol) {
            Ok(()) => app.push_transient_status(format!("Copied {symbol} to clipboard")),
            Err(e) => app.push_transient_status(e),
        },
    }
}

fn handle_panel_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match app.active_tab.clone() {
        Tab::Account => handle_account_key(app, key),
        Tab::Watchlist => handle_watchlist_key(app, key),
        Tab::Positions => handle_positions_key(app, key),
        Tab::Orders => handle_orders_key(app, key),
    }
}

/// Handle keys specific to the Account tab.
///
/// `←` / `h` and `→` / `l` move the equity-chart crosshair left and right
/// one data point at a time.  `Esc` dismisses the crosshair.
fn handle_account_key(app: &mut App, key: crossterm::event::KeyEvent) {
    // Range toggle works regardless of whether history is loaded.
    if key.code == KeyCode::Char('p') {
        app.equity_range = app.equity_range.cycle();
        app.equity_history.clear();
        app.equity_chart_cursor = None;
        let (period, timeframe) = app.equity_range.api_params();
        let _ = app.command_tx.try_send(Command::FetchPortfolioHistory {
            period: period.to_string(),
            timeframe: timeframe.to_string(),
        });
        app.push_transient_status(format!("Equity range: {}", app.equity_range.label()));
        return;
    }

    let n = app.equity_history.len();
    if n == 0 {
        return;
    }
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => {
            let cur = app.equity_chart_cursor.unwrap_or(n - 1);
            app.equity_chart_cursor = Some(cur.saturating_sub(1));
        }
        KeyCode::Right | KeyCode::Char('l') => {
            let cur = app.equity_chart_cursor.unwrap_or(0);
            app.equity_chart_cursor = Some((cur + 1).min(n - 1));
        }
        KeyCode::Esc => {
            app.equity_chart_cursor = None;
        }
        _ => {}
    }
}

/// Build a human-readable status bar notification for a trade update event.
///
/// Returns `None` for events that don't warrant a notification (e.g. `pending_new`).
fn fill_notification_text(order: &crate::types::Order, event_type: &str) -> Option<String> {
    let side = order.side.to_uppercase();
    let symbol = &order.symbol;
    let qty = order.qty.as_deref().unwrap_or("?");
    let filled_qty = &order.filled_qty;

    match event_type {
        "fill" => {
            let price_suffix = order
                .filled_avg_price
                .as_deref()
                .map(|p| format!(" @ ${p}"))
                .unwrap_or_default();
            Some(format!("✓ {side} {qty} {symbol} filled{price_suffix}"))
        }
        "partial_fill" => {
            let price_suffix = order
                .filled_avg_price
                .as_deref()
                .map(|p| format!(" @ ${p}"))
                .unwrap_or_default();
            Some(format!(
                "~ {side} {filled_qty}/{qty} {symbol} partial fill{price_suffix}"
            ))
        }
        "rejected" | "expired" | "suspended" => {
            Some(format!("✗ {side} {qty} {symbol} {event_type}"))
        }
        "canceled" => Some(format!("✗ {side} {qty} {symbol} canceled")),
        _ => None,
    }
}

/// Evaluate price alerts for an incoming market quote.
///
/// Derives a mid-price from the quote (ask → bid → fallback) and compares it
/// against any stored thresholds for the quote's symbol.  When a threshold is
/// crossed:
///   - A status bar message is pushed (e.g. `"🔔 AAPL above $185.00 — alert triggered!"`).
///   - The ASCII BEL character (`\x07`) is written to stdout so the terminal
///     rings the bell.
///   - The `triggered` flag is set so the same alert does not fire again on
///     the very next quote while the price remains above/below the threshold.
///
/// The triggered flag is reset to `false` when the price moves back across
/// the boundary (i.e. the condition is no longer met), allowing the alert to
/// fire again if the price subsequently crosses the threshold once more.
fn evaluate_price_alert(app: &mut App, q: &crate::types::Quote) {
    // Derive mid-price from ask / bid; skip if no price is available.
    let price = match (q.ap, q.bp) {
        (Some(a), Some(b)) => (a + b) / 2.0,
        (Some(a), None) => a,
        (None, Some(b)) => b,
        _ => return,
    };

    let symbol = q.symbol.clone();

    // Collect threshold values and current trigger states up front so we
    // don't hold a mutable borrow on `app.price_alerts` while calling the
    // status-push helpers (which also borrow `app` mutably).
    let (above, below, above_triggered, below_triggered) = {
        let alert = match app.price_alerts.get(&symbol) {
            Some(a) => a,
            None => return,
        };
        (
            alert.above,
            alert.below,
            alert.above_triggered,
            alert.below_triggered,
        )
    };

    // ── Above threshold ───────────────────────────────────────────────────────
    if let Some(threshold) = above {
        if price >= threshold {
            if !above_triggered {
                // Mark triggered.
                if let Some(a) = app.price_alerts.get_mut(&symbol) {
                    a.above_triggered = true;
                }
                let msg =
                    format!("🔔 {symbol} above ${threshold:.2} — alert triggered! (${price:.2})");
                app.push_transient_status(msg);
                // Ring the terminal bell.
                let _ = std::io::Write::write_all(&mut std::io::stdout(), b"\x07");
            }
        } else if above_triggered {
            // Price retreated below the threshold — reset so it can fire again.
            if let Some(a) = app.price_alerts.get_mut(&symbol) {
                a.above_triggered = false;
            }
        }
    }

    // ── Below threshold ───────────────────────────────────────────────────────
    if let Some(threshold) = below {
        if price <= threshold {
            if !below_triggered {
                if let Some(a) = app.price_alerts.get_mut(&symbol) {
                    a.below_triggered = true;
                }
                let msg =
                    format!("🔔 {symbol} below ${threshold:.2} — alert triggered! (${price:.2})");
                app.push_transient_status(msg);
                let _ = std::io::Write::write_all(&mut std::io::stdout(), b"\x07");
            }
        } else if below_triggered {
            // Price risen back above the threshold — reset.
            if let Some(a) = app.price_alerts.get_mut(&symbol) {
                a.below_triggered = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_helpers::*;
    use crate::app::{Modal, OrdersSubTab, Tab};
    use crate::commands::Command;
    use crate::events::{Event, StreamKind};
    use crate::types::{AccountInfo, MarketClock, Order, Quote};
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::layout::Rect;

    fn key(code: KeyCode) -> Event {
        Event::Input(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ctrl(code: KeyCode) -> Event {
        Event::Input(KeyEvent::new(code, KeyModifiers::CONTROL))
    }

    fn mouse_click(col: u16, row: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn mouse_move(col: u16, row: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn rect(x: u16, y: u16, w: u16, h: u16) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    // ── Data events ───────────────────────────────────────────────────────────

    #[test]
    fn resize_event_sets_needs_redraw() {
        let mut app = make_test_app();
        assert!(!app.needs_redraw);
        update(&mut app, Event::Resize(80, 24));
        assert!(app.needs_redraw, "needs_redraw must be true after resize");
    }

    #[test]
    fn resize_event_does_not_quit_or_change_state() {
        let mut app = make_test_app();
        update(&mut app, Event::Resize(120, 40));
        assert!(!app.should_quit);
        assert_eq!(app.active_tab, Tab::Account);
        assert!(app.needs_redraw);
    }

    #[test]
    fn account_updated_sets_account_and_pushes_equity() {
        let mut app = make_test_app();
        let acc = AccountInfo {
            equity: "500".into(),
            status: "ACTIVE".into(),
            ..Default::default()
        };
        update(&mut app, Event::AccountUpdated(acc));
        assert!(app.account.is_some());
        assert_eq!(app.equity_history.len(), 1);
    }

    #[test]
    fn positions_updated_empty_does_not_select() {
        let mut app = make_test_app();
        update(&mut app, Event::PositionsUpdated(vec![]));
        assert_eq!(app.positions_state.selected(), None);
    }

    #[test]
    fn positions_updated_non_empty_auto_selects_zero() {
        let mut app = make_test_app();
        let pos = vec![crate::types::Position {
            symbol: "AAPL".into(),
            qty: "10".into(),
            avg_entry_price: "100".into(),
            current_price: "110".into(),
            market_value: "1100".into(),
            unrealized_pl: "100".into(),
            unrealized_plpc: "0.1".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }];
        update(&mut app, Event::PositionsUpdated(pos));
        assert_eq!(app.positions_state.selected(), Some(0));
    }

    #[test]
    fn orders_updated_non_empty_auto_selects_zero() {
        let mut app = make_test_app();
        let orders = vec![make_order("o1", "accepted")];
        update(&mut app, Event::OrdersUpdated(orders));
        assert_eq!(app.orders_state.selected(), Some(0));
    }

    #[test]
    fn watchlist_updated_auto_selects_zero() {
        let mut app = make_test_app();
        let wl = make_watchlist(&["AAPL", "TSLA"]);
        update(&mut app, Event::WatchlistUpdated(wl));
        assert!(app.watchlist.is_some());
        assert_eq!(app.watchlist_state.selected(), Some(0));
    }

    #[test]
    fn watchlist_unavailable_sets_flag() {
        let mut app = make_test_app();
        assert!(!app.watchlist_unavailable);
        update(&mut app, Event::WatchlistUnavailable);
        assert!(app.watchlist_unavailable);
    }

    #[test]
    fn watchlist_unavailable_is_idempotent() {
        let mut app = make_test_app();
        update(&mut app, Event::WatchlistUnavailable);
        update(&mut app, Event::WatchlistUnavailable);
        assert!(app.watchlist_unavailable);
    }

    #[test]
    fn trade_update_existing_replaces_in_place() {
        let mut app = make_test_app();
        app.orders = vec![make_order("o1", "accepted")];
        let updated = Order {
            id: "o1".into(),
            status: "filled".into(),
            ..make_order("o1", "filled")
        };
        update(
            &mut app,
            Event::TradeUpdate {
                order: updated,
                event_type: "fill".to_string(),
            },
        );
        assert_eq!(app.orders.len(), 1);
        assert_eq!(app.orders[0].status, "filled");
    }

    #[test]
    fn trade_update_new_id_prepends() {
        let mut app = make_test_app();
        app.orders = vec![make_order("o1", "accepted")];
        update(
            &mut app,
            Event::TradeUpdate {
                order: make_order("o2", "accepted"),
                event_type: "pending_new".to_string(),
            },
        );
        assert_eq!(app.orders.len(), 2);
        assert_eq!(app.orders[0].id, "o2");
    }

    #[test]
    fn market_quote_inserted() {
        let mut app = make_test_app();
        let q = Quote {
            symbol: "AAPL".into(),
            ap: Some(185.0),
            bp: Some(184.9),
            ..Default::default()
        };
        update(&mut app, Event::MarketQuote(q));
        assert!(app.quotes.contains_key("AAPL"));
        assert_eq!(app.quotes["AAPL"].ap, Some(185.0));
    }

    #[test]
    fn clock_updated() {
        let mut app = make_test_app();
        let clock = MarketClock {
            is_open: true,
            ..Default::default()
        };
        update(&mut app, Event::ClockUpdated(clock));
        assert!(app.clock.as_ref().unwrap().is_open);
    }

    #[test]
    fn status_msg_updated() {
        let mut app = make_test_app();
        update(&mut app, Event::StatusMsg("hello".into()));
        assert_eq!(app.current_status_text(), "hello");
    }

    #[test]
    fn stream_connected_market_sets_flag() {
        let mut app = make_test_app();
        assert!(!app.market_stream_ok);
        update(&mut app, Event::StreamConnected(StreamKind::Market));
        assert!(app.market_stream_ok);
        assert!(
            !app.account_stream_ok,
            "account flag must remain unaffected"
        );
    }

    #[test]
    fn stream_connected_account_sets_flag() {
        let mut app = make_test_app();
        update(&mut app, Event::StreamConnected(StreamKind::Account));
        assert!(app.account_stream_ok);
        assert!(!app.market_stream_ok, "market flag must remain unaffected");
    }

    #[test]
    fn stream_disconnected_market_clears_flag() {
        let mut app = make_test_app();
        app.market_stream_ok = true;
        app.account_stream_ok = true;
        update(&mut app, Event::StreamDisconnected(StreamKind::Market));
        assert!(!app.market_stream_ok);
        assert!(app.account_stream_ok, "account flag must remain unaffected");
    }

    #[test]
    fn stream_disconnected_account_clears_flag() {
        let mut app = make_test_app();
        app.market_stream_ok = true;
        app.account_stream_ok = true;
        update(&mut app, Event::StreamDisconnected(StreamKind::Account));
        assert!(!app.account_stream_ok);
        assert!(app.market_stream_ok, "market flag must remain unaffected");
    }

    #[test]
    fn stream_connected_then_disconnected_roundtrip() {
        let mut app = make_test_app();
        update(&mut app, Event::StreamConnected(StreamKind::Market));
        update(&mut app, Event::StreamConnected(StreamKind::Account));
        assert!(app.market_stream_ok && app.account_stream_ok);
        update(&mut app, Event::StreamDisconnected(StreamKind::Market));
        assert!(!app.market_stream_ok);
        assert!(app.account_stream_ok);
    }

    // ── StreamReconnecting ────────────────────────────────────────────────────

    #[test]
    fn stream_reconnecting_market_sets_reconnecting_state() {
        let mut app = make_test_app();
        update(
            &mut app,
            Event::StreamReconnecting {
                kind: StreamKind::Market,
                attempt: 1,
            },
        );
        assert!(
            !app.market_stream_ok,
            "stream should not be ok while reconnecting"
        );
        assert!(
            app.market_stream_reconnecting,
            "reconnecting flag should be set"
        );
        assert_eq!(app.market_reconnect_attempt, 1);
        // account stream must remain unaffected
        assert!(!app.account_stream_reconnecting);
        assert_eq!(app.account_reconnect_attempt, 0);
    }

    #[test]
    fn stream_reconnecting_account_sets_reconnecting_state() {
        let mut app = make_test_app();
        update(
            &mut app,
            Event::StreamReconnecting {
                kind: StreamKind::Account,
                attempt: 2,
            },
        );
        assert!(!app.account_stream_ok);
        assert!(app.account_stream_reconnecting);
        assert_eq!(app.account_reconnect_attempt, 2);
        // market stream must remain unaffected
        assert!(!app.market_stream_reconnecting);
        assert_eq!(app.market_reconnect_attempt, 0);
    }

    #[test]
    fn stream_reconnecting_increments_attempt_counter() {
        let mut app = make_test_app();
        for attempt in 1..=3 {
            update(
                &mut app,
                Event::StreamReconnecting {
                    kind: StreamKind::Market,
                    attempt,
                },
            );
            assert_eq!(app.market_reconnect_attempt, attempt);
        }
    }

    #[test]
    fn stream_connected_clears_reconnecting_state_for_market() {
        let mut app = make_test_app();
        app.market_stream_reconnecting = true;
        app.market_reconnect_attempt = 3;
        update(&mut app, Event::StreamConnected(StreamKind::Market));
        assert!(app.market_stream_ok);
        assert!(
            !app.market_stream_reconnecting,
            "reconnecting flag should be cleared on connect"
        );
        assert_eq!(
            app.market_reconnect_attempt, 0,
            "attempt counter should reset on connect"
        );
    }

    #[test]
    fn stream_connected_clears_reconnecting_state_for_account() {
        let mut app = make_test_app();
        app.account_stream_reconnecting = true;
        app.account_reconnect_attempt = 5;
        update(&mut app, Event::StreamConnected(StreamKind::Account));
        assert!(app.account_stream_ok);
        assert!(!app.account_stream_reconnecting);
        assert_eq!(app.account_reconnect_attempt, 0);
    }

    #[test]
    fn stream_disconnected_clears_reconnecting_flag_for_market() {
        let mut app = make_test_app();
        app.market_stream_ok = true;
        app.market_stream_reconnecting = true;
        app.market_reconnect_attempt = 3;
        update(&mut app, Event::StreamDisconnected(StreamKind::Market));
        assert!(!app.market_stream_ok);
        assert!(
            !app.market_stream_reconnecting,
            "permanent disconnect should clear reconnecting flag"
        );
        // attempt counter is intentionally kept so the UI can show "OFFLINE"
        assert_eq!(app.market_reconnect_attempt, 3);
    }

    #[test]
    fn stream_disconnected_clears_reconnecting_flag_for_account() {
        let mut app = make_test_app();
        app.account_stream_ok = true;
        app.account_stream_reconnecting = true;
        app.account_reconnect_attempt = 2;
        update(&mut app, Event::StreamDisconnected(StreamKind::Account));
        assert!(!app.account_stream_ok);
        assert!(!app.account_stream_reconnecting);
        assert_eq!(app.account_reconnect_attempt, 2);
    }

    #[test]
    fn quit_event_sets_flag() {
        let mut app = make_test_app();
        update(&mut app, Event::Quit);
        assert!(app.should_quit);
    }

    // ── PortfolioHistoryLoaded ────────────────────────────────────────────────

    #[test]
    fn portfolio_history_loaded_replaces_equity_history() {
        let mut app = make_test_app();
        let data = vec![1000.0_f64, 1001.5, 1002.0];
        update(&mut app, Event::PortfolioHistoryLoaded(data));
        assert_eq!(app.equity_history, vec![100000, 100150, 100200]);
    }

    #[test]
    fn portfolio_history_loaded_overwrites_existing_history() {
        let mut app = make_test_app();
        app.equity_history = vec![99999];
        let data = vec![500.0_f64, 600.0];
        update(&mut app, Event::PortfolioHistoryLoaded(data));
        assert_eq!(app.equity_history, vec![50000, 60000]);
    }

    #[test]
    fn portfolio_history_loaded_empty_vec_clears_history() {
        let mut app = make_test_app();
        app.equity_history = vec![12345];
        update(&mut app, Event::PortfolioHistoryLoaded(vec![]));
        assert!(app.equity_history.is_empty());
    }

    #[test]
    fn portfolio_history_loaded_preserves_all_samples() {
        let mut app = make_test_app();
        let data: Vec<f64> = (0..390).map(|i| 100.0 + i as f64 * 0.1).collect();
        update(&mut app, Event::PortfolioHistoryLoaded(data));
        assert_eq!(
            app.equity_history.len(),
            390,
            "all intraday samples should be kept"
        );
    }

    #[test]
    fn tick_is_noop() {
        let mut app = make_test_app();
        let before_status = app.current_status_text().to_owned();
        update(&mut app, Event::Tick);
        assert!(!app.should_quit);
        assert_eq!(app.current_status_text(), before_status);
    }

    #[test]
    fn tick_clears_expired_transient_status_msg() {
        use std::time::{Duration, Instant};
        let mut app = make_test_app();
        // Set an already-expired transient message.
        app.status_queue.clear();
        app.status_queue.push_back(crate::app::StatusMessage {
            text: "Order submitted".into(),
            expires_at: Some(Instant::now() - Duration::from_secs(1)),
        });
        update(&mut app, Event::Tick);
        assert!(
            app.current_status_text().is_empty(),
            "expired transient message should be cleared"
        );
    }

    #[test]
    fn tick_does_not_clear_unexpired_transient_status_msg() {
        use std::time::{Duration, Instant};
        let mut app = make_test_app();
        // Set a transient message that expires in the far future.
        app.status_queue.clear();
        app.status_queue.push_back(crate::app::StatusMessage {
            text: "Refreshing…".into(),
            expires_at: Some(Instant::now() + Duration::from_secs(60)),
        });
        update(&mut app, Event::Tick);
        assert_eq!(
            app.current_status_text(),
            "Refreshing…",
            "non-expired message must not be cleared"
        );
    }

    #[test]
    fn tick_does_not_clear_persistent_status_msg() {
        let mut app = make_test_app();
        app.status_queue.clear();
        app.status_queue
            .push_back(crate::app::StatusMessage::persistent("Loading…"));
        update(&mut app, Event::Tick);
        assert_eq!(
            app.current_status_text(),
            "Loading…",
            "persistent message must survive tick"
        );
    }

    #[test]
    fn status_msg_transient_has_expiry() {
        let msg = crate::app::StatusMessage::with_ttl(
            "Submitting order…",
            std::time::Duration::from_secs(3),
        );
        assert!(!msg.text.is_empty());
        assert!(
            msg.expires_at.is_some(),
            "transient message must have an expiry"
        );
    }

    #[test]
    fn status_msg_persistent_has_no_expiry() {
        let msg = crate::app::StatusMessage::persistent("Error: unauthorized");
        assert!(
            msg.expires_at.is_none(),
            "persistent message must have no expiry"
        );
    }

    #[test]
    fn status_queue_multiple_messages_display_first_then_second() {
        use crate::app::StatusMessage;
        use std::time::{Duration, Instant};
        let mut app = make_test_app();
        // Push an expired transient message first, then a persistent one.
        app.status_queue.push_back(StatusMessage {
            text: "First".into(),
            expires_at: Some(Instant::now() - Duration::from_secs(1)),
        });
        app.status_queue
            .push_back(StatusMessage::persistent("Second"));
        // Front is "First" (expired), so current text shows it first.
        assert_eq!(app.current_status_text(), "First");
        // After a tick, "First" expires and "Second" becomes current.
        update(&mut app, Event::Tick);
        assert_eq!(app.current_status_text(), "Second");
    }

    #[test]
    fn status_queue_cap_drops_oldest_when_full() {
        use crate::app::StatusMessage;
        let mut app = make_test_app();
        for i in 0..5 {
            app.push_status(StatusMessage::persistent(format!("msg{i}")));
        }
        assert_eq!(app.status_queue.len(), 5);
        // Pushing a 6th should drop the oldest (msg0) and keep msg1..msg5.
        app.push_status(StatusMessage::persistent("msg5"));
        assert_eq!(app.status_queue.len(), 5);
        // The oldest "msg0" should be gone; the front is now "msg1".
        assert_eq!(app.current_status_text(), "msg1");
    }

    #[test]
    fn status_queue_tick_drains_all_expired_messages() {
        use crate::app::StatusMessage;
        use std::time::{Duration, Instant};
        let mut app = make_test_app();
        // Push three expired transient messages.
        for i in 0..3 {
            app.status_queue.push_back(StatusMessage {
                text: format!("old{i}"),
                expires_at: Some(Instant::now() - Duration::from_secs(1)),
            });
        }
        app.push_status(StatusMessage::persistent("fresh"));
        update(&mut app, Event::Tick);
        // All expired messages should be cleared, leaving only "fresh".
        assert_eq!(app.current_status_text(), "fresh");
        assert_eq!(app.status_queue.len(), 1);
    }

    #[test]
    fn push_status_first_message_becomes_current() {
        use crate::app::StatusMessage;
        let mut app = make_test_app();
        assert_eq!(app.current_status_text(), "");
        app.push_status(StatusMessage::persistent("hello"));
        assert_eq!(app.current_status_text(), "hello");
    }

    #[test]
    fn key_q_quits() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn key_ctrl_c_quits() {
        let mut app = make_test_app();
        update(&mut app, ctrl(KeyCode::Char('c')));
        assert!(app.should_quit);
    }

    #[test]
    fn key_question_mark_opens_help() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Char('?')));
        assert!(matches!(app.modal, Some(Modal::Help)));
    }

    #[test]
    fn key_question_mark_toggles_help_closed_when_open() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Help);
        update(&mut app, key(KeyCode::Char('?')));
        assert!(
            app.modal.is_none(),
            "second ? should dismiss the Help overlay"
        );
    }

    #[test]
    fn help_modal_any_key_closes() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Help);
        update(&mut app, key(KeyCode::Enter));
        assert!(app.modal.is_none(), "any key should close Help modal");
    }

    #[test]
    fn key_uppercase_a_opens_about() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Char('A')));
        assert!(matches!(app.modal, Some(Modal::About)));
    }

    #[test]
    fn about_modal_any_key_closes() {
        let mut app = make_test_app();
        app.modal = Some(Modal::About);
        update(&mut app, key(KeyCode::Enter));
        assert!(app.modal.is_none(), "any key should close About modal");
    }

    #[test]
    fn about_modal_space_closes() {
        let mut app = make_test_app();
        app.modal = Some(Modal::About);
        update(&mut app, key(KeyCode::Char(' ')));
        assert!(app.modal.is_none());
    }

    #[test]
    fn key_1_switches_to_account() {
        let mut app = make_test_app();
        app.active_tab = Tab::Positions; // not Orders, so '1' switches tab
        update(&mut app, key(KeyCode::Char('1')));
        assert_eq!(app.active_tab, Tab::Account);
    }

    #[test]
    fn key_4_switches_to_orders() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Char('4')));
        assert_eq!(app.active_tab, Tab::Orders);
    }

    #[test]
    fn key_tab_cycles_forward() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Tab));
        assert_eq!(app.active_tab, Tab::Watchlist);
    }

    #[test]
    fn key_backtab_cycles_backward() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::BackTab));
        assert_eq!(app.active_tab, Tab::Orders);
    }

    #[test]
    fn key_esc_closes_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Help);
        update(&mut app, key(KeyCode::Esc));
        assert!(app.modal.is_none());
    }

    #[test]
    fn key_r_sets_refreshing_status() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Char('r')));
        assert_eq!(app.current_status_text(), "Refreshing…");
    }

    // ── Watchlist panel keys ──────────────────────────────────────────────────

    fn watchlist_app() -> App {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = Some(make_watchlist(&["AAPL", "TSLA", "NVDA"]));
        app.watchlist_state.select(Some(0));
        app
    }

    #[test]
    fn watchlist_j_moves_down() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.watchlist_state.selected(), Some(1));
    }

    #[test]
    fn watchlist_j_clamps_at_end() {
        let mut app = watchlist_app();
        app.watchlist_state.select(Some(2)); // last row
        update(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.watchlist_state.selected(), Some(2));
    }

    #[test]
    fn watchlist_k_moves_up() {
        let mut app = watchlist_app();
        app.watchlist_state.select(Some(2));
        update(&mut app, key(KeyCode::Char('k')));
        assert_eq!(app.watchlist_state.selected(), Some(1));
    }

    #[test]
    fn watchlist_k_clamps_at_zero() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('k')));
        assert_eq!(app.watchlist_state.selected(), Some(0));
    }

    #[test]
    fn watchlist_gg_jumps_to_top() {
        let mut app = watchlist_app();
        app.watchlist_state.select(Some(2));
        update(&mut app, key(KeyCode::Char('g')));
        // first 'g' — sets pending, no jump yet
        assert_eq!(app.watchlist_state.selected(), Some(2));
        assert!(app.pending_g_at.is_some());
        update(&mut app, key(KeyCode::Char('g')));
        // second 'g' within timeout → jump to top
        assert_eq!(app.watchlist_state.selected(), Some(0));
        assert!(app.pending_g_at.is_none());
    }

    #[test]
    fn watchlist_g_single_sets_pending_no_jump() {
        let mut app = watchlist_app();
        app.watchlist_state.select(Some(2));
        update(&mut app, key(KeyCode::Char('g')));
        assert_eq!(
            app.watchlist_state.selected(),
            Some(2),
            "single g must not jump"
        );
        assert!(app.pending_g_at.is_some());
    }

    #[test]
    fn watchlist_g_then_other_key_clears_pending() {
        let mut app = watchlist_app();
        app.watchlist_state.select(Some(2));
        update(&mut app, key(KeyCode::Char('g')));
        assert!(app.pending_g_at.is_some());
        update(&mut app, key(KeyCode::Char('j'))); // any other key
        assert!(app.pending_g_at.is_none());
        assert_eq!(
            app.watchlist_state.selected(),
            Some(2),
            "pending cleared, no jump"
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn watchlist_G_jumps_to_bottom() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('G')));
        assert_eq!(app.watchlist_state.selected(), Some(2));
    }

    #[test]
    fn watchlist_enter_opens_symbol_detail() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Enter));
        assert!(matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "AAPL"));
    }

    #[test]
    fn watchlist_o_opens_order_entry_with_symbol() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('o')));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "AAPL"));
    }

    #[test]
    fn watchlist_a_opens_add_symbol() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('a')));
        assert!(matches!(&app.modal, Some(Modal::AddSymbol { .. })));
    }

    // ── Price alert: 'A' key (SetAlert modal) ─────────────────────────────────

    #[test]
    fn watchlist_uppercase_a_opens_set_alert_modal_with_symbol() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('A')));
        assert!(
            matches!(
                &app.modal,
                Some(Modal::SetAlert { symbol, .. }) if symbol == "AAPL"
            ),
            "expected SetAlert modal for AAPL, got: {:?}",
            app.modal
        );
    }

    #[test]
    fn watchlist_uppercase_a_without_selection_does_nothing() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        // No row selected
        update(&mut app, key(KeyCode::Char('A')));
        assert!(
            app.modal.is_none(),
            "no modal should open without a selection"
        );
    }

    #[test]
    fn watchlist_uppercase_a_prefills_existing_alert() {
        let mut app = watchlist_app();
        app.price_alerts.insert(
            "AAPL".into(),
            crate::types::PriceAlert {
                above: Some(200.0),
                below: Some(150.0),
                ..Default::default()
            },
        );
        update(&mut app, key(KeyCode::Char('A')));
        match &app.modal {
            Some(Modal::SetAlert {
                above_input,
                below_input,
                ..
            }) => {
                assert_eq!(above_input, "200.00");
                assert_eq!(below_input, "150.00");
            }
            other => panic!("expected SetAlert modal, got: {:?}", other),
        }
    }

    #[test]
    fn uppercase_a_on_non_watchlist_tab_opens_about() {
        let (mut app, _rx) = make_app_with_cmd();
        app.active_tab = Tab::Positions;
        update(&mut app, key(KeyCode::Char('A')));
        assert!(
            matches!(&app.modal, Some(Modal::About)),
            "expected About modal on non-watchlist tab, got: {:?}",
            app.modal
        );
    }

    // ── evaluate_price_alert ──────────────────────────────────────────────────

    fn make_quote(symbol: &str, ask: f64) -> crate::types::Quote {
        crate::types::Quote {
            symbol: symbol.into(),
            ap: Some(ask),
            bp: None,
            ..Default::default()
        }
    }

    #[test]
    fn alert_above_fires_when_price_crosses_threshold() {
        let (mut app, _rx) = make_app_with_cmd();
        app.price_alerts.insert(
            "AAPL".into(),
            crate::types::PriceAlert {
                above: Some(200.0),
                ..Default::default()
            },
        );
        update(&mut app, Event::MarketQuote(make_quote("AAPL", 201.0)));
        assert!(
            app.current_status_text().contains("above"),
            "status should mention 'above': {}",
            app.current_status_text()
        );
        assert!(
            app.price_alerts["AAPL"].above_triggered,
            "above_triggered should be set"
        );
    }

    #[test]
    fn alert_above_does_not_fire_below_threshold() {
        let (mut app, _rx) = make_app_with_cmd();
        app.price_alerts.insert(
            "AAPL".into(),
            crate::types::PriceAlert {
                above: Some(200.0),
                ..Default::default()
            },
        );
        update(&mut app, Event::MarketQuote(make_quote("AAPL", 199.0)));
        assert_eq!(
            app.current_status_text(),
            "",
            "no status should be set below threshold"
        );
        assert!(
            !app.price_alerts["AAPL"].above_triggered,
            "above_triggered should stay false"
        );
    }

    #[test]
    fn alert_above_does_not_fire_twice_consecutively() {
        let (mut app, _rx) = make_app_with_cmd();
        app.price_alerts.insert(
            "AAPL".into(),
            crate::types::PriceAlert {
                above: Some(200.0),
                ..Default::default()
            },
        );
        update(&mut app, Event::MarketQuote(make_quote("AAPL", 201.0)));
        let first_status = app.current_status_text().to_string();
        // Second tick still above threshold — should NOT fire again.
        // Drain the status so we can check nothing new was pushed.
        app.status_queue.clear();
        update(&mut app, Event::MarketQuote(make_quote("AAPL", 202.0)));
        assert_eq!(
            app.current_status_text(),
            "",
            "second consecutive quote above threshold should not re-fire; first was: {first_status}"
        );
    }

    #[test]
    fn alert_above_resets_and_refires_after_price_retreats() {
        let (mut app, _rx) = make_app_with_cmd();
        app.price_alerts.insert(
            "AAPL".into(),
            crate::types::PriceAlert {
                above: Some(200.0),
                above_triggered: true,
                ..Default::default()
            },
        );
        // Price drops below threshold → triggered flag should reset.
        update(&mut app, Event::MarketQuote(make_quote("AAPL", 198.0)));
        assert!(
            !app.price_alerts["AAPL"].above_triggered,
            "above_triggered should reset when price retreats"
        );
        // Price rises back above threshold → should fire again.
        update(&mut app, Event::MarketQuote(make_quote("AAPL", 201.0)));
        assert!(
            app.price_alerts["AAPL"].above_triggered,
            "above_triggered should re-fire after price crosses again"
        );
    }

    #[test]
    fn alert_below_fires_when_price_falls_below_threshold() {
        let (mut app, _rx) = make_app_with_cmd();
        app.price_alerts.insert(
            "AAPL".into(),
            crate::types::PriceAlert {
                below: Some(150.0),
                ..Default::default()
            },
        );
        update(&mut app, Event::MarketQuote(make_quote("AAPL", 149.0)));
        assert!(
            app.current_status_text().contains("below"),
            "status should mention 'below': {}",
            app.current_status_text()
        );
        assert!(app.price_alerts["AAPL"].below_triggered);
    }

    #[test]
    fn alert_no_fire_for_symbol_without_alert() {
        let (mut app, _rx) = make_app_with_cmd();
        // No alerts configured for TSLA.
        update(&mut app, Event::MarketQuote(make_quote("TSLA", 999.0)));
        assert_eq!(app.current_status_text(), "");
    }

    #[test]
    fn alert_uses_mid_price_when_both_ask_and_bid_present() {
        let (mut app, _rx) = make_app_with_cmd();
        app.price_alerts.insert(
            "AAPL".into(),
            crate::types::PriceAlert {
                above: Some(200.0),
                ..Default::default()
            },
        );
        // Ask=201, bid=199 → mid = 200.0; exactly at threshold → fires.
        let q = crate::types::Quote {
            symbol: "AAPL".into(),
            ap: Some(201.0),
            bp: Some(199.0),
            ..Default::default()
        };
        update(&mut app, Event::MarketQuote(q));
        assert!(
            app.price_alerts["AAPL"].above_triggered,
            "mid-price at threshold should trigger alert"
        );
    }

    #[test]
    fn watchlist_d_opens_confirm_remove_watchlist() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('d')));
        assert!(matches!(
            &app.modal,
            Some(Modal::ConfirmRemoveWatchlist { symbol, .. }) if symbol == "AAPL"
        ));
    }

    #[test]
    fn watchlist_slash_starts_search() {
        let mut app = watchlist_app();
        update(&mut app, key(KeyCode::Char('/')));
        assert!(app.searching);
    }

    // ── Orders panel keys ─────────────────────────────────────────────────────

    fn orders_app() -> App {
        let mut app = make_test_app();
        app.active_tab = Tab::Orders;
        app.orders = vec![make_order("o1", "accepted"), make_order("o2", "accepted")];
        app.orders_state.select(Some(0));
        app
    }

    #[test]
    fn orders_key_1_switches_to_open_subtab() {
        let mut app = orders_app();
        app.orders_subtab = OrdersSubTab::Filled;
        update(&mut app, key(KeyCode::Char('1')));
        assert_eq!(app.orders_subtab, OrdersSubTab::Open);
        assert_eq!(app.active_tab, Tab::Orders); // tab must NOT change
    }

    #[test]
    fn orders_key_2_switches_to_filled_subtab() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Char('2')));
        assert_eq!(app.orders_subtab, OrdersSubTab::Filled);
        assert_eq!(app.active_tab, Tab::Orders);
    }

    #[test]
    fn orders_key_3_switches_to_cancelled_subtab() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Char('3')));
        assert_eq!(app.orders_subtab, OrdersSubTab::Cancelled);
        assert_eq!(app.active_tab, Tab::Orders);
    }

    #[test]
    fn key_1_from_other_panels_still_switches_tab() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        update(&mut app, key(KeyCode::Char('1')));
        assert_eq!(app.active_tab, Tab::Account);
    }

    #[test]
    fn orders_c_opens_confirm_for_selected() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Char('c')));
        assert!(matches!(&app.modal, Some(Modal::Confirm { .. })));
    }

    #[test]
    fn orders_o_opens_blank_order_entry() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Char('o')));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol.is_empty()));
    }

    // ── Modal keys ────────────────────────────────────────────────────────────

    #[test]
    fn modal_tab_advances_focused_field() {
        use crate::app::{OrderEntryState, OrderField};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new(String::new());
        state.focused_field = OrderField::Qty;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Tab));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.focused_field == OrderField::Price)
        );
    }

    #[test]
    fn modal_char_appends_to_symbol_field() {
        use crate::app::{OrderEntryState, OrderField};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new(String::new());
        state.focused_field = OrderField::Symbol;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Char('A')));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "A"));
    }

    #[test]
    fn modal_digit_appends_to_qty_field() {
        use crate::app::{OrderEntryState, OrderField};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new(String::new());
        state.focused_field = OrderField::Qty;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Char('5')));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.qty_input == "5"));
    }

    #[test]
    fn modal_non_digit_ignored_in_qty_field() {
        use crate::app::{OrderEntryState, OrderField};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new(String::new());
        state.focused_field = OrderField::Qty;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Char('x')));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.qty_input.is_empty()));
    }

    #[test]
    fn modal_backspace_removes_last_char_from_symbol() {
        use crate::app::{OrderEntryState, OrderField};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new("AB".into());
        state.focused_field = OrderField::Symbol;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Backspace));
        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "A"));
    }

    // ── Search keys ───────────────────────────────────────────────────────────

    #[test]
    fn search_char_appends_to_query() {
        let mut app = watchlist_app();
        app.searching = true;
        update(&mut app, key(KeyCode::Char('A')));
        assert_eq!(app.search_query, "A");
    }

    #[test]
    fn search_esc_exits_search_mode() {
        let mut app = watchlist_app();
        app.searching = true;
        update(&mut app, key(KeyCode::Esc));
        assert!(!app.searching);
    }

    #[test]
    fn search_enter_exits_search_mode() {
        let mut app = watchlist_app();
        app.searching = true;
        update(&mut app, key(KeyCode::Enter));
        assert!(!app.searching);
    }

    // ── Phase 2: command sends ────────────────────────────────────────────────

    fn app_with_rx() -> (App, tokio::sync::mpsc::Receiver<Command>) {
        use crate::config::{AlpacaConfig, AlpacaEnv};
        let (command_tx, command_rx) = tokio::sync::mpsc::channel(16);
        let (symbol_tx, _) = tokio::sync::watch::channel(vec![]);
        let app = App::new(
            AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: AlpacaEnv::Paper,
                dry_run: false,
            },
            crate::prefs::AppPrefs::default(),
            std::sync::Arc::new(tokio::sync::Notify::new()),
            command_tx,
            symbol_tx,
        );
        (app, command_rx)
    }

    #[test]
    fn order_entry_submit_sends_submit_order_command() {
        use crate::app::{OrderEntryState, OrderField};
        use crate::types::AccountInfo;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Submit;
        state.qty_input = "10".into();
        state.price_input = "185.00".into();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_none(), "modal should close after submit");
        assert_eq!(app.current_status_text(), "Submitting order…");
        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::SubmitOrder { symbol, .. } if symbol == "AAPL"),
            "expected SubmitOrder for AAPL"
        );
    }

    #[test]
    fn order_entry_submit_market_order_omits_price() {
        use crate::app::{FullOrderType, OrderEntryState, OrderField};
        use crate::types::AccountInfo;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let mut state = OrderEntryState::new("TSLA".into());
        state.focused_field = OrderField::Submit;
        state.order_type = FullOrderType::Market;
        state.qty_input = "5".into();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::SubmitOrder { order_type, limit_price: None, .. }
                if order_type == "market"),
            "market order should have no price"
        );
    }

    #[test]
    fn confirm_cancel_order_sends_cancel_command() {
        use crate::app::ConfirmAction;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.active_tab = Tab::Orders;
        app.modal = Some(Modal::Confirm {
            message: "Cancel?".into(),
            action: ConfirmAction::CancelOrder("order-xyz".into()),
            confirmed: false,
        });

        update(&mut app, key(KeyCode::Char('y')));

        assert!(app.modal.is_none());
        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::CancelOrder(id) if id == "order-xyz"),
            "expected CancelOrder command"
        );
    }

    #[test]
    fn confirm_remove_watchlist_sends_remove_command() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::ConfirmRemoveWatchlist {
            symbol: "TLRY".into(),
            watchlist_id: "wl-id".into(),
        });

        update(&mut app, key(KeyCode::Char('y')));

        assert!(app.modal.is_none());
        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::RemoveFromWatchlist { symbol, .. } if symbol == "TLRY"),
            "expected RemoveFromWatchlist command"
        );
    }

    #[test]
    fn confirm_remove_watchlist_enter_confirms() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::ConfirmRemoveWatchlist {
            symbol: "AAPL".into(),
            watchlist_id: "wl-id".into(),
        });

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_none(), "Enter should close the modal");
        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::RemoveFromWatchlist { symbol, .. } if symbol == "AAPL"),
            "expected RemoveFromWatchlist command on Enter"
        );
    }

    #[test]
    fn confirm_remove_watchlist_n_cancels() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::ConfirmRemoveWatchlist {
            symbol: "AAPL".into(),
            watchlist_id: "wl-id".into(),
        });

        update(&mut app, key(KeyCode::Char('n')));

        assert!(
            app.modal.is_none(),
            "'n' should close the modal without action"
        );
        assert!(
            cmd_rx.try_recv().is_err(),
            "no command should be sent on cancel"
        );
    }

    #[test]
    fn confirm_remove_watchlist_esc_cancels() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::ConfirmRemoveWatchlist {
            symbol: "AAPL".into(),
            watchlist_id: "wl-id".into(),
        });

        update(&mut app, key(KeyCode::Esc));

        assert!(
            app.modal.is_none(),
            "Esc should close the modal without action"
        );
        assert!(
            cmd_rx.try_recv().is_err(),
            "no command should be sent on Esc"
        );
    }

    #[test]
    fn watchlist_d_no_confirm_pref_sends_remove_directly() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.active_tab = Tab::Watchlist;
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        app.watchlist_state.select(Some(0));
        app.prefs.safety.confirm_watchlist_remove = false;

        update(&mut app, key(KeyCode::Char('d')));

        assert!(app.modal.is_none(), "no modal when pref is false");
        let cmd = cmd_rx.try_recv().expect("command should be sent directly");
        assert!(
            matches!(cmd, Command::RemoveFromWatchlist { symbol, .. } if symbol == "AAPL"),
            "expected RemoveFromWatchlist command"
        );
    }

    #[test]
    fn confirm_remove_watchlist_unhandled_key_keeps_modal_open() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::ConfirmRemoveWatchlist {
            symbol: "AAPL".into(),
            watchlist_id: "wl-id".into(),
        });

        // Press an unrecognized key — modal should stay open and no command sent
        update(&mut app, key(KeyCode::F(1)));

        assert!(
            matches!(&app.modal, Some(Modal::ConfirmRemoveWatchlist { symbol, .. }) if symbol == "AAPL"),
            "unrecognized key should keep ConfirmRemoveWatchlist modal open"
        );
        assert!(
            cmd_rx.try_recv().is_err(),
            "no command should be sent on unrecognized key"
        );
    }

    #[test]
    fn add_symbol_enter_sends_add_command() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::AddSymbol {
            input: "NVDA".into(),
            watchlist_id: "wl-id".into(),
        });

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_none());
        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::AddToWatchlist { symbol, .. } if symbol == "NVDA"),
            "expected AddToWatchlist command"
        );
    }

    #[test]
    fn add_symbol_empty_input_sends_no_command() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::AddSymbol {
            input: String::new(),
            watchlist_id: "wl-id".into(),
        });

        update(&mut app, key(KeyCode::Enter));

        assert!(cmd_rx.try_recv().is_err(), "no command for empty input");
    }

    // ── GlobalSearch modal ────────────────────────────────────────────────────

    #[test]
    fn ctrl_f_opens_global_search_modal() {
        let mut app = make_test_app();
        update(&mut app, ctrl(KeyCode::Char('f')));
        assert!(
            matches!(&app.modal, Some(Modal::GlobalSearch { query }) if query.is_empty()),
            "Ctrl-F should open GlobalSearch with empty query"
        );
    }

    #[test]
    fn slash_opens_global_search_on_account_tab() {
        let mut app = make_test_app();
        app.active_tab = Tab::Account;
        update(&mut app, key(KeyCode::Char('/')));
        assert!(
            matches!(&app.modal, Some(Modal::GlobalSearch { .. })),
            "'/' on Account tab should open GlobalSearch"
        );
    }

    #[test]
    fn slash_opens_global_search_on_positions_tab() {
        let mut app = make_test_app();
        app.active_tab = Tab::Positions;
        update(&mut app, key(KeyCode::Char('/')));
        assert!(
            matches!(&app.modal, Some(Modal::GlobalSearch { .. })),
            "'/' on Positions tab should open GlobalSearch"
        );
    }

    #[test]
    fn slash_opens_global_search_on_orders_tab() {
        let mut app = make_test_app();
        app.active_tab = Tab::Orders;
        update(&mut app, key(KeyCode::Char('/')));
        assert!(
            matches!(&app.modal, Some(Modal::GlobalSearch { .. })),
            "'/' on Orders tab should open GlobalSearch"
        );
    }

    #[test]
    fn slash_does_not_open_global_search_on_watchlist_tab() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        update(&mut app, key(KeyCode::Char('/')));
        assert!(
            !matches!(&app.modal, Some(Modal::GlobalSearch { .. })),
            "'/' on Watchlist tab must NOT open GlobalSearch (it activates watchlist search)"
        );
    }

    #[test]
    fn global_search_char_key_appends_uppercase() {
        let mut app = make_test_app();
        app.modal = Some(Modal::GlobalSearch {
            query: String::new(),
        });
        update(&mut app, key(KeyCode::Char('a')));
        assert!(
            matches!(&app.modal, Some(Modal::GlobalSearch { query }) if query == "A"),
            "char should be uppercased and appended"
        );
    }

    #[test]
    fn global_search_backspace_removes_last_char() {
        let mut app = make_test_app();
        app.modal = Some(Modal::GlobalSearch { query: "AA".into() });
        update(&mut app, key(KeyCode::Backspace));
        assert!(
            matches!(&app.modal, Some(Modal::GlobalSearch { query }) if query == "A"),
            "Backspace should remove last character"
        );
    }

    #[test]
    fn global_search_esc_dismisses_modal() {
        let mut app = make_test_app();
        app.modal = Some(Modal::GlobalSearch {
            query: "GOO".into(),
        });
        update(&mut app, key(KeyCode::Esc));
        assert!(app.modal.is_none(), "Esc should close GlobalSearch");
    }

    #[test]
    fn global_search_enter_with_query_opens_symbol_detail() {
        let (mut app, _rx) = app_with_rx();
        app.modal = Some(Modal::GlobalSearch {
            query: "GOOG".into(),
        });
        update(&mut app, key(KeyCode::Enter));
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "GOOG"),
            "Enter should transition GlobalSearch → SymbolDetail"
        );
    }

    #[test]
    fn global_search_enter_dispatches_fetch_intraday_bars() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::GlobalSearch {
            query: "TSLA".into(),
        });
        update(&mut app, key(KeyCode::Enter));
        let cmd = cmd_rx
            .try_recv()
            .expect("FetchIntradayBars command should be dispatched");
        assert!(
            matches!(cmd, Command::FetchIntradayBars(s) if s == "TSLA"),
            "expected FetchIntradayBars for TSLA"
        );
    }

    #[test]
    fn global_search_enter_with_empty_query_closes_modal() {
        let (mut app, mut cmd_rx) = app_with_rx();
        app.modal = Some(Modal::GlobalSearch {
            query: String::new(),
        });
        update(&mut app, key(KeyCode::Enter));
        assert!(
            app.modal.is_none(),
            "Enter on empty query should close modal"
        );
        assert!(
            cmd_rx.try_recv().is_err(),
            "no command should be sent for empty query"
        );
    }

    #[test]
    fn global_search_backspace_on_empty_query_is_noop() {
        let mut app = make_test_app();
        app.modal = Some(Modal::GlobalSearch {
            query: String::new(),
        });
        update(&mut app, key(KeyCode::Backspace));
        assert!(
            matches!(&app.modal, Some(Modal::GlobalSearch { query }) if query.is_empty()),
            "Backspace on empty query should keep modal open with empty query"
        );
    }

    #[test]
    fn global_search_unhandled_key_keeps_modal_open() {
        let mut app = make_test_app();
        app.modal = Some(Modal::GlobalSearch { query: "MS".into() });
        // Arrow keys are unhandled; modal should remain open with unchanged query.
        update(&mut app, key(KeyCode::Up));
        assert!(
            matches!(&app.modal, Some(Modal::GlobalSearch { query }) if query == "MS"),
            "unhandled key should keep GlobalSearch open with unchanged query"
        );
    }

    #[test]
    fn ctrl_f_opens_search_even_when_on_watchlist_tab() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        update(&mut app, ctrl(KeyCode::Char('f')));
        assert!(
            matches!(&app.modal, Some(Modal::GlobalSearch { .. })),
            "Ctrl-F should open GlobalSearch regardless of active tab"
        );
    }

    #[test]
    fn watchlist_updated_pushes_symbols_to_symbol_tx() {
        use crate::config::{AlpacaConfig, AlpacaEnv};
        use tokio::sync::watch;
        let (command_tx, _) = tokio::sync::mpsc::channel(1);
        let (symbol_tx, symbol_rx) = watch::channel(vec![]);
        let mut app = App::new(
            AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: AlpacaEnv::Paper,
                dry_run: false,
            },
            crate::prefs::AppPrefs::default(),
            std::sync::Arc::new(tokio::sync::Notify::new()),
            command_tx,
            symbol_tx,
        );

        let wl = make_watchlist(&["AAPL", "TSLA", "NVDA"]);
        update(&mut app, Event::WatchlistUpdated(wl));

        assert!(
            symbol_rx.has_changed().unwrap_or(false),
            "symbol_tx should have been updated"
        );
        let symbols = symbol_rx.borrow().clone();
        assert_eq!(symbols, vec!["AAPL", "TSLA", "NVDA"]);
    }

    // ── send_command error path: channel full / closed (regression for #7) ────

    fn app_with_capacity(cap: usize) -> (App, tokio::sync::mpsc::Receiver<Command>) {
        use crate::config::{AlpacaConfig, AlpacaEnv};
        let (command_tx, command_rx) = tokio::sync::mpsc::channel(cap);
        let (symbol_tx, _) = tokio::sync::watch::channel(vec![]);
        let app = App::new(
            AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: AlpacaEnv::Paper,
                dry_run: false,
            },
            crate::prefs::AppPrefs::default(),
            std::sync::Arc::new(tokio::sync::Notify::new()),
            command_tx,
            symbol_tx,
        );
        (app, command_rx)
    }

    #[test]
    fn channel_full_sets_busy_status_msg() {
        // Channel capacity 1, pre-fill it so the next try_send hits Full.
        let (mut app, _rx) = app_with_capacity(1);
        // Fill the channel
        let _ = app
            .command_tx
            .try_send(Command::CancelOrder("dummy".into()));

        // Now trigger another command via update() — should hit TrySendError::Full
        use crate::app::{OrderEntryState, OrderField};
        use crate::types::AccountInfo;
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Submit;
        state.qty_input = "1".into();
        state.price_input = "100.00".into();
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Enter));

        assert_eq!(
            app.current_status_text(),
            "System busy — please retry",
            "full channel should show busy message"
        );
    }

    #[test]
    fn mouse_click_tab_bar_switches_tab() {
        let (mut app, _rx) = app_with_capacity(4);
        // Tab bar at row 2, full width 80.
        // Actual ratatui Tabs layout (` label ` + `|` divider):
        //   Tab 0 "1:Account"   (len 9): cols  0..10  (width 11)
        //   divider `|`:               col  11
        //   Tab 1 "2:Watchlist" (len 11): cols 12..24 (width 13)
        //   divider `|`:               col  25
        //   Tab 2 "3:Positions" (len 11): cols 26..38 (width 13)
        //   divider `|`:               col  39
        //   Tab 3 "4:Orders"    (len  8): cols 40..49 (width 10)
        app.hit_areas.tab_bar = rect(0, 2, 80, 1);
        assert_eq!(app.active_tab, Tab::Account);

        // Click inside "2:Watchlist" (col 18 is within 12..24)
        update(&mut app, mouse_click(18, 2));
        assert_eq!(app.active_tab, Tab::Watchlist);

        // Click inside "3:Positions" (col 32 is within 26..38)
        update(&mut app, mouse_click(32, 2));
        assert_eq!(app.active_tab, Tab::Positions);

        // Click inside "4:Orders" (col 45 is within 40..49)
        update(&mut app, mouse_click(45, 2));
        assert_eq!(app.active_tab, Tab::Orders);

        // Click back on "1:Account" (col 5 is within 0..10)
        update(&mut app, mouse_click(5, 2));
        assert_eq!(app.active_tab, Tab::Account);
    }

    #[test]
    fn mouse_non_left_click_ignored() {
        let (mut app, _rx) = app_with_capacity(4);
        app.hit_areas.tab_bar = rect(0, 2, 80, 1);
        app.active_tab = Tab::Watchlist;
        // MouseMove event — must not switch tab
        update(&mut app, mouse_move(25, 2));
        assert_eq!(app.active_tab, Tab::Watchlist);
    }

    #[test]
    fn mouse_click_orders_subtab() {
        let (mut app, _rx) = app_with_capacity(4);
        app.active_tab = Tab::Orders;
        // Simulate per-subtab rects as rendered from actual label widths (0 orders each):
        //   "1:Open (0)"      → len=10 → width=12, x=0
        //   "2:Filled (0)"    → len=12 → width=14, x=13 (12 + 1 divider)
        //   "3:Cancelled (0)" → len=15 → width=17, x=28 (13 + 14 + 1 divider)
        app.hit_areas.orders_subtab_rects = vec![
            rect(0, 5, 12, 1),  // Open
            rect(13, 5, 14, 1), // Filled
            rect(28, 5, 17, 1), // Cancelled
        ];
        assert_eq!(app.orders_subtab, OrdersSubTab::Open);

        // Click within Filled subtab rect
        update(&mut app, mouse_click(13, 5));
        assert_eq!(app.orders_subtab, OrdersSubTab::Filled);

        // Click within Cancelled subtab rect
        update(&mut app, mouse_click(28, 5));
        assert_eq!(app.orders_subtab, OrdersSubTab::Cancelled);

        // Click back within Open subtab rect
        update(&mut app, mouse_click(0, 5));
        assert_eq!(app.orders_subtab, OrdersSubTab::Open);
    }

    #[test]
    fn mouse_click_list_row_selects_item() {
        use crate::types::{Asset, Watchlist};

        let (mut app, _rx) = app_with_capacity(4);
        let make_asset = |sym: &str| Asset {
            id: sym.to_string(),
            symbol: sym.to_string(),
            name: sym.to_string(),
            exchange: "NASDAQ".to_string(),
            asset_class: "us_equity".to_string(),
            tradable: true,
            shortable: false,
            fractionable: false,
            easy_to_borrow: false,
        };
        app.watchlist = Some(Watchlist {
            id: "wl1".to_string(),
            name: "Test".to_string(),
            assets: vec![make_asset("AAPL"), make_asset("GOOG")],
        });
        app.watchlist_state.select(Some(0));
        app.active_tab = Tab::Watchlist;

        // List data starts at row 10 (border + header already accounted for)
        app.hit_areas.list_data_start_y = 10;

        // Click on row 11 → data_row 1 → idx 1 → selects second asset
        update(&mut app, mouse_click(5, 11));
        assert_eq!(app.watchlist_state.selected(), Some(1));
    }

    #[test]
    fn channel_closed_sets_stopped_status_msg() {
        // Drop the receiver to close the channel.
        let (mut app, rx) = app_with_capacity(8);
        drop(rx);

        use crate::app::{OrderEntryState, OrderField};
        use crate::types::AccountInfo;
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Submit;
        state.qty_input = "1".into();
        state.price_input = "100.00".into();
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Enter));

        assert_eq!(
            app.current_status_text(),
            "Command handler stopped — restart app",
            "closed channel should show stopped message"
        );
    }

    // ── Validation gate tests ─────────────────────────────────────────────────

    fn order_entry_submit_state(symbol: &str) -> crate::app::OrderEntryState {
        use crate::app::{FullOrderType, OrderEntryState, OrderField};
        let mut s = OrderEntryState::new(symbol.into());
        s.focused_field = OrderField::Submit;
        s.order_type = FullOrderType::Limit;
        s.qty_input = "10".into();
        s.price_input = "100.00".into();
        s
    }

    #[test]
    fn validation_empty_symbol_keeps_modal_open() {
        let (mut app, _rx) = app_with_capacity(4);
        let mut state = order_entry_submit_state("");
        state.symbol.clear();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(
            app.modal.is_some(),
            "modal must stay open on validation failure"
        );
        assert!(
            !app.current_status_text().is_empty(),
            "status_msg must contain error text"
        );
    }

    #[test]
    fn validation_zero_qty_keeps_modal_open() {
        let (mut app, _rx) = app_with_capacity(4);
        let mut state = order_entry_submit_state("AAPL");
        state.qty_input = "0".into();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_some());
        assert!(!app.current_status_text().is_empty());
    }

    #[test]
    fn validation_non_numeric_price_on_limit_keeps_modal_open() {
        let (mut app, _rx) = app_with_capacity(4);
        let mut state = order_entry_submit_state("AAPL");
        state.price_input = "bad".into();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_some());
        assert!(!app.current_status_text().is_empty());
    }

    #[test]
    fn validation_exceeds_buying_power_keeps_modal_open() {
        use crate::types::AccountInfo;
        let (mut app, _rx) = app_with_capacity(4);
        app.account = Some(AccountInfo {
            buying_power: "500".into(), // 10 × 100 = 1000 > 500
            ..Default::default()
        });
        let state = order_entry_submit_state("AAPL");
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_some());
        assert!(!app.current_status_text().is_empty());
    }

    #[test]
    fn validation_pass_sends_command_and_closes_modal() {
        use crate::types::AccountInfo;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let state = order_entry_submit_state("AAPL");
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_none(), "modal should close on valid submit");
        cmd_rx.try_recv().expect("command should be sent");
    }

    // ── IntradayBarsReceived ──────────────────────────────────────────────────

    #[test]
    fn intraday_bars_received_stores_bars() {
        let (mut app, _rx) = app_with_rx();
        update(
            &mut app,
            Event::IntradayBarsReceived {
                symbol: "AAPL".into(),
                bars: vec![14200, 14215, 14198],
            },
        );
        assert_eq!(
            app.intraday_bars.get("AAPL"),
            Some(&vec![14200u64, 14215, 14198])
        );
    }

    #[test]
    fn intraday_bars_received_overwrites_existing_bars() {
        let (mut app, _rx) = app_with_rx();
        app.intraday_bars.insert("AAPL".into(), vec![100, 200]);
        update(
            &mut app,
            Event::IntradayBarsReceived {
                symbol: "AAPL".into(),
                bars: vec![300, 400, 500],
            },
        );
        assert_eq!(app.intraday_bars.get("AAPL"), Some(&vec![300u64, 400, 500]));
    }

    // ── SymbolDetail modal key handling ───────────────────────────────────────

    #[test]
    fn symbol_detail_o_opens_order_entry() {
        let (mut app, _rx) = app_with_rx();
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        update(&mut app, key(KeyCode::Char('o')));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "AAPL" && s.side == crate::app::OrderSide::Buy),
            "o key should open buy order entry for symbol"
        );
    }

    #[test]
    fn symbol_detail_s_opens_sell_order_entry() {
        let (mut app, _rx) = app_with_rx();
        app.modal = Some(Modal::SymbolDetail("NVDA".into()));
        update(&mut app, key(KeyCode::Char('s')));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "NVDA" && s.side == crate::app::OrderSide::Sell),
            "s key should open sell order entry for symbol"
        );
    }

    #[test]
    fn symbol_detail_w_sends_add_watchlist_command() {
        use crate::types::Watchlist;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.watchlist = Some(Watchlist {
            id: "wl-1".into(),
            name: "Primary".into(),
            assets: vec![],
        });
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        update(&mut app, key(KeyCode::Char('w')));
        // Modal stays open
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "AAPL"),
            "w key should keep symbol detail modal open"
        );
        // Command dispatched
        let cmd = cmd_rx.try_recv().expect("AddToWatchlist command expected");
        assert!(
            matches!(cmd, Command::AddToWatchlist { watchlist_id, symbol }
                if watchlist_id == "wl-1" && symbol == "AAPL")
        );
    }

    #[test]
    fn symbol_detail_w_sends_remove_watchlist_command_when_in_watchlist() {
        use crate::types::{Asset, Watchlist};
        let (mut app, mut cmd_rx) = app_with_rx();
        let asset = Asset {
            id: "asset-1".into(),
            symbol: "AAPL".into(),
            name: "Apple Inc.".into(),
            exchange: "NASDAQ".into(),
            asset_class: "us_equity".into(),
            tradable: true,
            shortable: true,
            fractionable: true,
            easy_to_borrow: true,
        };
        app.watchlist = Some(Watchlist {
            id: "wl-1".into(),
            name: "Primary".into(),
            assets: vec![asset],
        });
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        update(&mut app, key(KeyCode::Char('w')));
        let cmd = cmd_rx
            .try_recv()
            .expect("RemoveFromWatchlist command expected");
        assert!(
            matches!(cmd, Command::RemoveFromWatchlist { watchlist_id, symbol }
                if watchlist_id == "wl-1" && symbol == "AAPL")
        );
    }

    #[test]
    fn symbol_detail_other_key_keeps_modal_open() {
        let (mut app, _rx) = app_with_rx();
        app.modal = Some(Modal::SymbolDetail("TSLA".into()));
        update(&mut app, key(KeyCode::Char('j')));
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "TSLA"),
            "unknown key should keep symbol detail modal open"
        );
    }

    #[test]
    fn symbol_detail_esc_closes_modal() {
        let (mut app, _rx) = app_with_rx();
        app.modal = Some(Modal::SymbolDetail("TSLA".into()));
        update(&mut app, key(KeyCode::Esc));
        assert!(app.modal.is_none(), "Esc should close the modal");
    }

    #[test]
    fn symbol_detail_w_with_no_watchlist_keeps_modal_open() {
        // When there is no watchlist, pressing 'w' should keep the modal open
        // without dispatching any command (covers the wl_info == None branch).
        let (mut app, mut cmd_rx) = app_with_rx();
        app.watchlist = None;
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        update(&mut app, key(KeyCode::Char('w')));
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "AAPL"),
            "modal should remain open when no watchlist is set"
        );
        assert!(
            cmd_rx.try_recv().is_err(),
            "no command should be dispatched without a watchlist"
        );
    }

    // ── Positions panel key handling ─────────────────────────────────────────

    fn positions_app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
        let (mut app, rx) = app_with_rx();
        app.active_tab = Tab::Positions;
        app.positions = vec![crate::types::Position {
            symbol: "AAPL".into(),
            qty: "10".into(),
            avg_entry_price: "150.00".into(),
            current_price: "155.00".into(),
            market_value: "1550.00".into(),
            unrealized_pl: "50.00".into(),
            unrealized_plpc: "0.033".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }];
        app.positions_state.select(Some(0));
        (app, rx)
    }

    #[test]
    fn positions_enter_opens_position_detail_and_dispatches_fetch() {
        let (mut app, mut cmd_rx) = positions_app();
        update(&mut app, key(KeyCode::Enter));
        assert!(
            matches!(&app.modal, Some(Modal::PositionDetail { symbol }) if symbol == "AAPL"),
            "Enter on a position should open PositionDetail for that symbol"
        );
        let cmd = cmd_rx
            .try_recv()
            .expect("FetchIntradayBars should be dispatched");
        assert!(
            matches!(cmd, Command::FetchIntradayBars(s) if s == "AAPL"),
            "expected FetchIntradayBars for AAPL"
        );
    }

    #[test]
    fn positions_enter_with_no_selection_does_nothing() {
        let (mut app, _rx) = app_with_rx();
        app.active_tab = Tab::Positions;
        app.positions_state.select(None);
        update(&mut app, key(KeyCode::Enter));
        assert!(
            app.modal.is_none(),
            "Enter with no selection should not open a modal"
        );
    }

    #[test]
    fn positions_o_opens_sell_order_entry() {
        let (mut app, _rx) = positions_app();
        update(&mut app, key(KeyCode::Char('o')));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.symbol == "AAPL" && s.side == crate::app::OrderSide::Sell),
            "o key in positions should open SELL order entry for selected symbol"
        );
    }

    #[test]
    fn positions_s_cycles_sort_column() {
        let (mut app, _rx) = positions_app();
        assert_eq!(app.positions_sort.col, crate::app::PositionSortCol::None);
        update(&mut app, key(KeyCode::Char('s')));
        assert_eq!(
            app.positions_sort.col,
            crate::app::PositionSortCol::Symbol,
            "s key in positions should cycle sort column to Symbol"
        );
    }

    #[test]
    fn order_side_cycle_next_wraps() {
        use crate::app::OrderSide;
        assert_eq!(OrderSide::Buy.cycle_next(), OrderSide::Sell);
        assert_eq!(OrderSide::Sell.cycle_next(), OrderSide::SellShort);
        assert_eq!(OrderSide::SellShort.cycle_next(), OrderSide::Buy);
    }

    #[test]
    fn order_side_cycle_prev_wraps() {
        use crate::app::OrderSide;
        assert_eq!(OrderSide::Buy.cycle_prev(), OrderSide::SellShort);
        assert_eq!(OrderSide::Sell.cycle_prev(), OrderSide::Buy);
        assert_eq!(OrderSide::SellShort.cycle_prev(), OrderSide::Sell);
    }

    #[test]
    fn order_side_as_str() {
        use crate::app::OrderSide;
        assert_eq!(OrderSide::Buy.as_str(), "buy");
        assert_eq!(OrderSide::Sell.as_str(), "sell");
        assert_eq!(OrderSide::SellShort.as_str(), "sell_short");
    }

    #[test]
    fn modal_side_right_arrow_cycles_forward() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Side;
        state.side = OrderSide::Buy;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Right));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.side == OrderSide::Sell),
            "right arrow should cycle Buy → Sell"
        );
    }

    #[test]
    fn modal_side_left_arrow_cycles_backward() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Side;
        state.side = OrderSide::Sell;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Left));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.side == OrderSide::Buy),
            "left arrow should cycle Sell → Buy"
        );
    }

    #[test]
    fn modal_side_down_arrow_cycles_forward() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Side;
        state.side = OrderSide::Buy;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Down));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.side == OrderSide::Sell),
            "down arrow should cycle Buy → Sell"
        );
    }

    #[test]
    fn modal_side_up_arrow_cycles_backward() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Side;
        state.side = OrderSide::Sell;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Up));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.side == OrderSide::Buy),
            "up arrow should cycle Sell → Buy"
        );
    }

    #[test]
    fn modal_order_type_down_arrow_toggles() {
        use crate::app::{FullOrderType, OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::OrderType;
        state.order_type = FullOrderType::Limit;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Down));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.order_type == FullOrderType::Stop),
            "down arrow on OrderType should cycle Limit → Stop"
        );
    }

    #[test]
    fn modal_order_type_up_arrow_toggles() {
        use crate::app::{FullOrderType, OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::OrderType;
        state.order_type = FullOrderType::Limit;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Up));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.order_type == FullOrderType::Market),
            "up arrow on OrderType should cycle Limit → Market"
        );
    }

    #[test]
    fn modal_tif_down_arrow_toggles() {
        use crate::app::{OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::TimeInForce;
        state.gtc_order = false;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Down));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.gtc_order),
            "down arrow on TIF should toggle to GTC"
        );
    }

    #[test]
    fn modal_tif_up_arrow_toggles() {
        use crate::app::{OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::TimeInForce;
        state.gtc_order = true;
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Up));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if !s.gtc_order),
            "up arrow on TIF should toggle to DAY"
        );
    }

    #[test]
    fn modal_up_down_on_text_field_is_noop() {
        use crate::app::{OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let mut state = OrderEntryState::new("AAPL".into());
        state.focused_field = OrderField::Qty;
        state.qty_input = "10".into();
        app.modal = Some(Modal::OrderEntry(state));
        update(&mut app, key(KeyCode::Down));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.qty_input == "10"),
            "down arrow on text field should not modify input"
        );
        update(&mut app, key(KeyCode::Up));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.qty_input == "10"),
            "up arrow on text field should not modify input"
        );
    }

    #[test]
    fn order_entry_submit_sell_short_sends_correct_side() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        use crate::types::AccountInfo;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let mut state = OrderEntryState::new("AAPL".into());
        state.side = OrderSide::SellShort;
        state.focused_field = OrderField::Submit;
        state.qty_input = "5".into();
        state.price_input = "180.00".into();
        app.modal = Some(Modal::OrderEntry(state));

        update(&mut app, key(KeyCode::Enter));

        assert!(app.modal.is_none(), "modal should close after submit");
        let cmd = cmd_rx.try_recv().expect("command should be sent");
        assert!(
            matches!(cmd, Command::SubmitOrder { side, .. } if side == "sell_short"),
            "expected sell_short side in SubmitOrder"
        );
    }

    // ── Orders navigation (j/k/g/G) ──────────────────────────────────────────

    #[test]
    fn orders_j_moves_down() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.orders_state.selected(), Some(1));
    }

    #[test]
    fn orders_j_clamps_at_end() {
        let mut app = orders_app();
        app.orders_state.select(Some(1)); // last item
        update(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.orders_state.selected(), Some(1));
    }

    #[test]
    fn orders_k_moves_up() {
        let mut app = orders_app();
        app.orders_state.select(Some(1));
        update(&mut app, key(KeyCode::Char('k')));
        assert_eq!(app.orders_state.selected(), Some(0));
    }

    #[test]
    fn orders_k_clamps_at_zero() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Char('k')));
        assert_eq!(app.orders_state.selected(), Some(0));
    }

    #[test]
    fn orders_gg_jumps_to_top() {
        let mut app = orders_app();
        app.orders_state.select(Some(1));
        update(&mut app, key(KeyCode::Char('g')));
        assert_eq!(
            app.orders_state.selected(),
            Some(1),
            "single g must not jump"
        );
        update(&mut app, key(KeyCode::Char('g')));
        assert_eq!(app.orders_state.selected(), Some(0));
        assert!(app.pending_g_at.is_none());
    }

    #[test]
    fn orders_g_single_sets_pending_no_jump() {
        let mut app = orders_app();
        app.orders_state.select(Some(1));
        update(&mut app, key(KeyCode::Char('g')));
        assert_eq!(app.orders_state.selected(), Some(1));
        assert!(app.pending_g_at.is_some());
    }

    #[test]
    fn orders_g_then_other_key_clears_pending() {
        let mut app = orders_app();
        app.orders_state.select(Some(1));
        update(&mut app, key(KeyCode::Char('g')));
        update(&mut app, key(KeyCode::Char('k')));
        assert!(app.pending_g_at.is_none());
    }

    #[test]
    #[allow(non_snake_case)]
    fn orders_G_jumps_to_bottom() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Char('G')));
        assert_eq!(app.orders_state.selected(), Some(1));
    }

    #[test]
    fn orders_down_arrow_moves_down() {
        let mut app = orders_app();
        update(&mut app, key(KeyCode::Down));
        assert_eq!(app.orders_state.selected(), Some(1));
    }

    #[test]
    fn orders_up_arrow_moves_up() {
        let mut app = orders_app();
        app.orders_state.select(Some(1));
        update(&mut app, key(KeyCode::Up));
        assert_eq!(app.orders_state.selected(), Some(0));
    }

    #[test]
    fn orders_c_with_no_selection_does_nothing() {
        let mut app = orders_app();
        app.orders_state.select(None);
        update(&mut app, key(KeyCode::Char('c')));
        assert!(
            app.modal.is_none(),
            "c with no selection should not open confirm"
        );
    }

    // ── Positions navigation (j/k/g/G) ───────────────────────────────────────

    #[test]
    fn positions_j_moves_down() {
        let (mut app, _rx) = positions_app();
        // Add a second position so j can move
        app.positions.push(crate::types::Position {
            symbol: "TSLA".into(),
            qty: "5".into(),
            avg_entry_price: "200.00".into(),
            current_price: "210.00".into(),
            market_value: "1050.00".into(),
            unrealized_pl: "50.00".into(),
            unrealized_plpc: "0.05".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        });
        update(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.positions_state.selected(), Some(1));
    }

    #[test]
    fn positions_j_clamps_at_end() {
        let (mut app, _rx) = positions_app();
        update(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.positions_state.selected(), Some(0)); // only 1 item, stays at 0
    }

    #[test]
    fn positions_k_clamps_at_zero() {
        let (mut app, _rx) = positions_app();
        update(&mut app, key(KeyCode::Char('k')));
        assert_eq!(app.positions_state.selected(), Some(0));
    }

    #[test]
    fn positions_gg_jumps_to_top() {
        let (mut app, _rx) = positions_app();
        // Add a second position so we can meaningfully test jump-to-top
        app.positions.push(crate::types::Position {
            symbol: "TSLA".into(),
            qty: "5".into(),
            avg_entry_price: "200.00".into(),
            current_price: "210.00".into(),
            market_value: "1050.00".into(),
            unrealized_pl: "50.00".into(),
            unrealized_plpc: "0.05".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        });
        app.positions_state.select(Some(1));
        update(&mut app, key(KeyCode::Char('g')));
        assert_eq!(
            app.positions_state.selected(),
            Some(1),
            "single g must not jump"
        );
        update(&mut app, key(KeyCode::Char('g')));
        assert_eq!(app.positions_state.selected(), Some(0));
        assert!(app.pending_g_at.is_none());
    }

    #[test]
    fn positions_g_single_sets_pending_no_jump() {
        let (mut app, _rx) = positions_app();
        app.positions.push(crate::types::Position {
            symbol: "TSLA".into(),
            qty: "5".into(),
            avg_entry_price: "200.00".into(),
            current_price: "210.00".into(),
            market_value: "1050.00".into(),
            unrealized_pl: "50.00".into(),
            unrealized_plpc: "0.05".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        });
        app.positions_state.select(Some(1));
        update(&mut app, key(KeyCode::Char('g')));
        assert_eq!(app.positions_state.selected(), Some(1));
        assert!(app.pending_g_at.is_some());
    }

    #[test]
    fn positions_g_then_other_key_clears_pending() {
        let (mut app, _rx) = positions_app();
        update(&mut app, key(KeyCode::Char('g')));
        update(&mut app, key(KeyCode::Char('j')));
        assert!(app.pending_g_at.is_none());
    }

    #[test]
    #[allow(non_snake_case)]
    fn positions_G_jumps_to_bottom() {
        let (mut app, _rx) = positions_app();
        update(&mut app, key(KeyCode::Char('G')));
        assert_eq!(app.positions_state.selected(), Some(0)); // 1 item, bottom = 0
    }

    #[test]
    fn positions_down_arrow_moves_down() {
        let (mut app, _rx) = positions_app();
        // one item; down clamps at 0
        update(&mut app, key(KeyCode::Down));
        assert_eq!(app.positions_state.selected(), Some(0));
    }

    #[test]
    fn positions_up_arrow_clamps_at_zero() {
        let (mut app, _rx) = positions_app();
        update(&mut app, key(KeyCode::Up));
        assert_eq!(app.positions_state.selected(), Some(0));
    }

    #[test]
    fn positions_j_with_no_positions_is_noop() {
        let (mut app, _rx) = app_with_rx();
        app.active_tab = Tab::Positions;
        update(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.positions_state.selected(), None);
    }

    // ── Watchlist edge cases ──────────────────────────────────────────────────

    #[test]
    fn watchlist_d_with_no_selection_does_nothing() {
        let mut app = watchlist_app();
        app.watchlist_state.select(None);
        update(&mut app, key(KeyCode::Char('d')));
        assert!(
            app.modal.is_none(),
            "d with no selection should not open confirm"
        );
    }

    #[test]
    fn watchlist_enter_with_no_selection_does_nothing() {
        let mut app = watchlist_app();
        app.watchlist_state.select(None);
        update(&mut app, key(KeyCode::Enter));
        assert!(
            app.modal.is_none(),
            "enter with no selection should not open modal"
        );
    }

    #[test]
    fn watchlist_a_without_watchlist_does_nothing() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = None;
        update(&mut app, key(KeyCode::Char('a')));
        assert!(
            app.modal.is_none(),
            "a with no watchlist should not open AddSymbol"
        );
    }

    #[test]
    fn watchlist_d_without_watchlist_does_nothing() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = None;
        update(&mut app, key(KeyCode::Char('d')));
        assert!(app.modal.is_none());
    }

    // ── Mouse modal handler ───────────────────────────────────────────────────

    #[test]
    fn mouse_click_modal_submit_button_submits_order() {
        use crate::app::OrderEntryState;
        use crate::types::AccountInfo;
        let (mut app, mut cmd_rx) = app_with_rx();
        app.account = Some(AccountInfo {
            buying_power: "100000".into(),
            ..Default::default()
        });
        let mut state = OrderEntryState::new("AAPL".into());
        state.qty_input = "10".into();
        state.price_input = "150.00".into();
        app.modal = Some(Modal::OrderEntry(state));
        // Place submit button at (10, 20) with size 10x1
        app.hit_areas.modal_submit = Some(rect(10, 20, 10, 1));

        update(&mut app, mouse_click(12, 20));

        assert!(
            app.modal.is_none(),
            "modal should close after clicking submit"
        );
        let cmd = cmd_rx
            .try_recv()
            .expect("SubmitOrder command should be dispatched");
        assert!(matches!(cmd, Command::SubmitOrder { .. }));
    }

    #[test]
    fn mouse_click_modal_field_side_left_third_selects_buy() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        let (mut app, _rx) = app_with_rx();
        let state = OrderEntryState::new("AAPL".into());
        app.modal = Some(Modal::OrderEntry(state));
        // Side field at x=10, width=30; left third = x < 10+10=20
        app.hit_areas.modal_fields = vec![(OrderField::Side, rect(10, 5, 30, 1))];

        update(&mut app, mouse_click(10, 5)); // left third

        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s))
                if s.side == OrderSide::Buy && s.focused_field == OrderField::Side));
    }

    #[test]
    fn mouse_click_modal_field_side_middle_third_selects_sell() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        let (mut app, _rx) = app_with_rx();
        let state = OrderEntryState::new("AAPL".into());
        app.modal = Some(Modal::OrderEntry(state));
        // Side field at x=10, width=30; middle third = x in [20, 30)
        app.hit_areas.modal_fields = vec![(OrderField::Side, rect(10, 5, 30, 1))];

        update(&mut app, mouse_click(22, 5)); // middle third

        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s))
                if s.side == OrderSide::Sell && s.focused_field == OrderField::Side));
    }

    #[test]
    fn mouse_click_modal_field_side_right_third_selects_sell_short() {
        use crate::app::{OrderEntryState, OrderField, OrderSide};
        let (mut app, _rx) = app_with_rx();
        let state = OrderEntryState::new("AAPL".into());
        app.modal = Some(Modal::OrderEntry(state));
        // Side field at x=10, width=30; right third starts at x >= 10+20=30
        app.hit_areas.modal_fields = vec![(OrderField::Side, rect(10, 5, 30, 1))];

        update(&mut app, mouse_click(32, 5)); // right third

        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s))
                if s.side == OrderSide::SellShort && s.focused_field == OrderField::Side));
    }

    #[test]
    fn mouse_click_modal_field_order_type_left_half_selects_limit() {
        use crate::app::{FullOrderType, OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let state = OrderEntryState::new("AAPL".into());
        app.modal = Some(Modal::OrderEntry(state));
        // OrderType at x=10, width=20; offset=2 → section = 2*5/20 = 0 → Market
        app.hit_areas.modal_fields = vec![(OrderField::OrderType, rect(10, 6, 20, 1))];

        update(&mut app, mouse_click(12, 6)); // near left edge → Market

        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s))
                if s.order_type == FullOrderType::Market && s.focused_field == OrderField::OrderType));
    }

    #[test]
    fn mouse_click_modal_field_order_type_right_half_selects_market() {
        use crate::app::{FullOrderType, OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let state = OrderEntryState::new("AAPL".into());
        app.modal = Some(Modal::OrderEntry(state));
        // OrderType at x=10, width=20; section index 4 (rightmost 4/5) → TrailingStop
        app.hit_areas.modal_fields = vec![(OrderField::OrderType, rect(10, 6, 20, 1))];

        update(&mut app, mouse_click(28, 6)); // near right edge

        assert!(matches!(&app.modal, Some(Modal::OrderEntry(s))
                if s.order_type == FullOrderType::TrailingStop && s.focused_field == OrderField::OrderType));
    }

    #[test]
    fn mouse_click_modal_field_other_focuses_field() {
        use crate::app::{OrderEntryState, OrderField};
        let (mut app, _rx) = app_with_rx();
        let state = OrderEntryState::new("AAPL".into());
        app.modal = Some(Modal::OrderEntry(state));
        app.hit_areas.modal_fields = vec![(OrderField::Qty, rect(10, 7, 20, 1))];

        update(&mut app, mouse_click(15, 7));

        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.focused_field == OrderField::Qty)
        );
    }

    #[test]
    fn mouse_click_confirm_yes_button_accepts() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        app.watchlist_state.select(Some(0));
        // Open a ConfirmRemoveWatchlist modal first
        update(&mut app, key(KeyCode::Char('d')));
        assert!(matches!(
            &app.modal,
            Some(Modal::ConfirmRemoveWatchlist { .. })
        ));
        // Place confirm buttons at x=10, width=20; left half = Yes
        app.hit_areas.modal_confirm_buttons = Some(rect(10, 10, 20, 1));

        update(&mut app, mouse_click(12, 10)); // left half → Yes

        // Confirm 'y' should close modal and dispatch remove command
        assert!(app.modal.is_none(), "yes click should close confirm modal");
    }

    #[test]
    fn mouse_click_confirm_no_button_cancels() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        app.watchlist_state.select(Some(0));
        // Open a ConfirmRemoveWatchlist modal first
        update(&mut app, key(KeyCode::Char('d')));
        assert!(matches!(
            &app.modal,
            Some(Modal::ConfirmRemoveWatchlist { .. })
        ));
        // Place confirm buttons at x=10, width=20; right half = No
        app.hit_areas.modal_confirm_buttons = Some(rect(10, 10, 20, 1));

        update(&mut app, mouse_click(22, 10)); // right half → No

        assert!(app.modal.is_none(), "no click should close confirm modal");
    }

    #[test]
    fn mouse_click_outside_modal_does_not_close_it() {
        use crate::app::OrderEntryState;
        let (mut app, _rx) = app_with_rx();
        let state = OrderEntryState::new("AAPL".into());
        app.modal = Some(Modal::OrderEntry(state));
        // No hit_areas set; click at (0, 0) hits nothing
        update(&mut app, mouse_click(0, 0));
        assert!(
            app.modal.is_some(),
            "click outside modal regions should leave modal open"
        );
    }

    // ── List row clicks for Positions and Orders tabs ─────────────────────────

    #[test]
    fn mouse_click_list_row_selects_positions_item() {
        let (mut app, _rx) = positions_app();
        app.positions.push(crate::types::Position {
            symbol: "TSLA".into(),
            qty: "5".into(),
            avg_entry_price: "200.00".into(),
            current_price: "210.00".into(),
            market_value: "1050.00".into(),
            unrealized_pl: "50.00".into(),
            unrealized_plpc: "0.05".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        });
        app.active_tab = Tab::Positions;
        app.hit_areas.list_data_start_y = 10;

        update(&mut app, mouse_click(5, 11)); // row 11 → data_row 1 → idx 1
        assert_eq!(app.positions_state.selected(), Some(1));
    }

    #[test]
    fn mouse_click_list_row_selects_orders_item() {
        let mut app = orders_app();
        app.hit_areas.list_data_start_y = 10;

        update(&mut app, mouse_click(5, 11)); // row 11 → data_row 1 → idx 1
        assert_eq!(app.orders_state.selected(), Some(1));
    }

    #[test]
    fn mouse_click_list_row_account_tab_is_noop() {
        let mut app = make_test_app();
        app.active_tab = Tab::Account;
        app.hit_areas.list_data_start_y = 10;
        // Should not panic and nothing should change
        update(&mut app, mouse_click(5, 11));
        // no assertion needed; just verifying no panic
    }

    #[test]
    fn mouse_click_list_row_out_of_bounds_does_not_panic() {
        let mut app = watchlist_app();
        app.hit_areas.list_data_start_y = 10;
        // Click on row 99 — no data item there
        update(&mut app, mouse_click(5, 99));
        // Selection should remain at 0 (not moved to invalid index)
        assert_eq!(app.watchlist_state.selected(), Some(0));
    }

    // ── Search handler edge cases ─────────────────────────────────────────────

    #[test]
    fn search_backspace_pops_char() {
        let mut app = make_test_app();
        app.searching = true;
        app.search_query = "AB".into();
        update(&mut app, key(KeyCode::Backspace));
        assert_eq!(app.search_query, "A");
    }

    #[test]
    fn search_backspace_on_empty_is_noop() {
        let mut app = make_test_app();
        app.searching = true;
        app.search_query = String::new();
        update(&mut app, key(KeyCode::Backspace));
        assert!(app.search_query.is_empty());
    }

    #[test]
    fn search_char_resets_watchlist_selection() {
        let mut app = watchlist_app();
        app.searching = true;
        app.watchlist_state.select(Some(2));
        update(&mut app, key(KeyCode::Char('A')));
        assert_eq!(app.watchlist_state.selected(), Some(0));
    }

    // ── Theme cycling (T key) ─────────────────────────────────────────────────

    #[test]
    fn t_key_cycles_theme_default_to_dark() {
        use crate::ui::theme::Theme;
        let mut app = make_test_app();
        assert_eq!(app.current_theme, Theme::Default);
        update(&mut app, key(KeyCode::Char('T')));
        assert_eq!(app.current_theme, Theme::Dark);
    }

    #[test]
    fn t_key_cycles_dark_to_high_contrast() {
        use crate::ui::theme::Theme;
        let mut app = make_test_app();
        app.current_theme = Theme::Dark;
        update(&mut app, key(KeyCode::Char('T')));
        assert_eq!(app.current_theme, Theme::HighContrast);
    }

    #[test]
    fn t_key_wraps_high_contrast_to_default() {
        use crate::ui::theme::Theme;
        let mut app = make_test_app();
        app.current_theme = Theme::HighContrast;
        update(&mut app, key(KeyCode::Char('T')));
        assert_eq!(app.current_theme, Theme::Default);
    }

    #[test]
    fn t_key_sets_status_message() {
        let mut app = make_test_app();
        update(&mut app, key(KeyCode::Char('T')));
        let status = app.current_status_text();
        assert!(
            status.contains("Theme:"),
            "Status should contain 'Theme:' after T key, got: {:?}",
            status
        );
    }

    #[test]
    fn t_key_three_presses_returns_to_default() {
        use crate::ui::theme::Theme;
        let mut app = make_test_app();
        for _ in 0..3 {
            update(&mut app, key(KeyCode::Char('T')));
        }
        assert_eq!(app.current_theme, Theme::Default);
    }

    // ── Fetch counter / spinner ───────────────────────────────────────────────

    #[test]
    fn fetch_started_increments_pending_requests() {
        let mut app = make_test_app();
        assert_eq!(app.pending_requests, 0);
        update(&mut app, Event::FetchStarted);
        assert_eq!(app.pending_requests, 1);
        update(&mut app, Event::FetchStarted);
        assert_eq!(app.pending_requests, 2);
    }

    #[test]
    fn fetch_complete_decrements_pending_requests() {
        let mut app = make_test_app();
        update(&mut app, Event::FetchStarted);
        update(&mut app, Event::FetchStarted);
        update(&mut app, Event::FetchComplete);
        assert_eq!(app.pending_requests, 1);
    }

    #[test]
    fn fetch_complete_sets_last_updated_when_counter_reaches_zero() {
        let mut app = make_test_app();
        assert!(app.last_updated.is_none());
        update(&mut app, Event::FetchStarted);
        update(&mut app, Event::FetchComplete);
        assert!(
            app.last_updated.is_some(),
            "last_updated should be set once all fetches complete"
        );
    }

    #[test]
    fn fetch_complete_does_not_set_last_updated_while_still_pending() {
        let mut app = make_test_app();
        update(&mut app, Event::FetchStarted);
        update(&mut app, Event::FetchStarted);
        update(&mut app, Event::FetchComplete); // still 1 in-flight
        assert!(
            app.last_updated.is_none(),
            "last_updated must not be set while fetches are still in-flight"
        );
    }

    #[test]
    fn fetch_complete_does_not_underflow_pending_requests() {
        let mut app = make_test_app();
        // Simulate spurious extra FetchComplete without FetchStarted
        update(&mut app, Event::FetchComplete);
        assert_eq!(
            app.pending_requests, 0,
            "saturating_sub should prevent underflow"
        );
    }

    #[test]
    fn tick_advances_spinner_when_requests_pending() {
        let mut app = make_test_app();
        app.pending_requests = 1;
        let before = app.spinner_tick;
        update(&mut app, Event::Tick);
        assert_eq!(app.spinner_tick, before.wrapping_add(1));
    }

    #[test]
    fn tick_does_not_advance_spinner_when_idle() {
        let mut app = make_test_app();
        assert_eq!(app.pending_requests, 0);
        let before = app.spinner_tick;
        update(&mut app, Event::Tick);
        assert_eq!(
            app.spinner_tick, before,
            "spinner should not advance when idle"
        );
    }

    #[test]
    fn spinner_frame_cycles_through_ten_frames() {
        let mut app = make_test_app();
        let frames: Vec<char> = (0..10)
            .map(|_| {
                let f = app.spinner_frame();
                app.spinner_tick = app.spinner_tick.wrapping_add(1);
                f
            })
            .collect();
        // All frames distinct within a cycle
        let unique: std::collections::HashSet<char> = frames.iter().copied().collect();
        assert_eq!(unique.len(), 10, "spinner should have 10 distinct frames");
        // Frame 10 wraps back to frame 0
        assert_eq!(
            app.spinner_frame(),
            frames[0],
            "spinner should wrap after 10 ticks"
        );
    }

    // ── copy symbol ('c' key) ─────────────────────────────────────────────────

    #[test]
    fn c_key_on_watchlist_with_selection_sets_status_message() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = Some(make_watchlist(&["AAPL", "TSLA"]));
        app.watchlist_state.select(Some(0));
        update(&mut app, key(KeyCode::Char('c')));
        let status = app.current_status_text();
        // Either "Copied AAPL to clipboard" or a clipboard error — either way non-empty
        assert!(
            !status.is_empty(),
            "pressing 'c' with a selection must always set a status message"
        );
        assert!(
            status.contains("AAPL") || status.contains("Clipboard") || status.contains("clipboard"),
            "status should mention symbol or clipboard; got: {status:?}"
        );
    }

    #[test]
    fn c_key_on_watchlist_without_selection_sets_no_symbol_selected_message() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        // no selection
        update(&mut app, key(KeyCode::Char('c')));
        assert_eq!(app.current_status_text(), "No symbol selected");
    }

    #[test]
    fn c_key_on_positions_with_selection_sets_status_message() {
        let (mut app, _rx) = positions_app();
        update(&mut app, key(KeyCode::Char('c')));
        let status = app.current_status_text();
        assert!(
            !status.is_empty(),
            "pressing 'c' on positions with selection must set a status message"
        );
    }

    #[test]
    fn c_key_on_orders_does_not_copy_but_opens_cancel_modal() {
        let mut app = orders_app();
        // Orders tab: 'c' should still trigger cancel confirm, not copy
        update(&mut app, key(KeyCode::Char('c')));
        assert!(
            matches!(app.modal, Some(Modal::Confirm { .. })),
            "pressing 'c' in Orders should open cancel confirm modal, not copy"
        );
        // Status should not be set to a copy message
        let status = app.current_status_text();
        assert!(
            !status.contains("Copied"),
            "Orders 'c' must not trigger copy; got: {status:?}"
        );
    }

    #[test]
    fn c_key_on_account_tab_sets_no_symbol_selected() {
        let mut app = make_test_app();
        assert_eq!(app.active_tab, Tab::Account);
        update(&mut app, key(KeyCode::Char('c')));
        assert_eq!(app.current_status_text(), "No symbol selected");
    }

    // ── fill_notification_text ──────────────────────────────────────────────

    #[test]
    fn fill_notification_fill_with_price() {
        let order = Order {
            filled_avg_price: Some("173.42".into()),
            ..make_order("o1", "filled")
        };
        let msg = fill_notification_text(&order, "fill").unwrap();
        assert_eq!(msg, "✓ BUY 10 AAPL filled @ $173.42");
    }

    #[test]
    fn fill_notification_fill_without_price() {
        let order = make_order("o1", "filled");
        let msg = fill_notification_text(&order, "fill").unwrap();
        assert_eq!(msg, "✓ BUY 10 AAPL filled");
    }

    #[test]
    fn fill_notification_partial_fill_with_price() {
        let order = Order {
            filled_qty: "5".into(),
            filled_avg_price: Some("173.40".into()),
            ..make_order("o1", "partially_filled")
        };
        let msg = fill_notification_text(&order, "partial_fill").unwrap();
        assert_eq!(msg, "~ BUY 5/10 AAPL partial fill @ $173.40");
    }

    #[test]
    fn fill_notification_partial_fill_without_price() {
        let order = Order {
            filled_qty: "3".into(),
            ..make_order("o1", "partially_filled")
        };
        let msg = fill_notification_text(&order, "partial_fill").unwrap();
        assert_eq!(msg, "~ BUY 3/10 AAPL partial fill");
    }

    #[test]
    fn fill_notification_rejected() {
        let order = make_order("o1", "rejected");
        let msg = fill_notification_text(&order, "rejected").unwrap();
        assert_eq!(msg, "✗ BUY 10 AAPL rejected");
    }

    #[test]
    fn fill_notification_expired() {
        let order = make_order("o1", "expired");
        let msg = fill_notification_text(&order, "expired").unwrap();
        assert_eq!(msg, "✗ BUY 10 AAPL expired");
    }

    #[test]
    fn fill_notification_canceled() {
        let order = make_order("o1", "canceled");
        let msg = fill_notification_text(&order, "canceled").unwrap();
        assert_eq!(msg, "✗ BUY 10 AAPL canceled");
    }

    #[test]
    fn fill_notification_pending_new_is_none() {
        let order = make_order("o1", "pending_new");
        assert!(fill_notification_text(&order, "pending_new").is_none());
    }

    #[test]
    fn fill_notification_unknown_event_is_none() {
        let order = make_order("o1", "accepted");
        assert!(fill_notification_text(&order, "replaced").is_none());
    }

    // ── TradeUpdate event dispatch ──────────────────────────────────────────

    #[test]
    fn trade_update_fill_pushes_notification() {
        let mut app = make_test_app();
        app.prefs.notifications.fill_notifications_enabled = true;
        let order = Order {
            filled_avg_price: Some("173.42".into()),
            ..make_order("o1", "filled")
        };
        update(
            &mut app,
            Event::TradeUpdate {
                order,
                event_type: "fill".to_string(),
            },
        );
        let status = app.current_status_text();
        assert!(
            status.contains("✓") && status.contains("AAPL"),
            "expected fill notification, got: {status:?}"
        );
    }

    #[test]
    fn trade_update_fill_skipped_when_notifications_disabled() {
        let mut app = make_test_app();
        app.prefs.notifications.fill_notifications_enabled = false;
        let order = Order {
            filled_avg_price: Some("173.42".into()),
            ..make_order("o1", "filled")
        };
        update(
            &mut app,
            Event::TradeUpdate {
                order,
                event_type: "fill".to_string(),
            },
        );
        let status = app.current_status_text();
        assert!(
            !status.contains("✓"),
            "expected no notification when disabled, got: {status:?}"
        );
    }

    #[test]
    fn trade_update_rejected_pushes_notification() {
        let mut app = make_test_app();
        app.prefs.notifications.fill_notifications_enabled = true;
        update(
            &mut app,
            Event::TradeUpdate {
                order: make_order("o1", "rejected"),
                event_type: "rejected".to_string(),
            },
        );
        let status = app.current_status_text();
        assert!(
            status.contains("✗") && status.contains("AAPL"),
            "expected rejected notification, got: {status:?}"
        );
    }

    #[test]
    fn trade_update_pending_no_notification() {
        let mut app = make_test_app();
        app.prefs.notifications.fill_notifications_enabled = true;
        update(
            &mut app,
            Event::TradeUpdate {
                order: make_order("o1", "pending_new"),
                event_type: "pending_new".to_string(),
            },
        );
        let status = app.current_status_text();
        assert!(
            !status.contains("✓") && !status.contains("✗") && !status.contains("~"),
            "pending_new should not push a notification, got: {status:?}"
        );
    }

    // ── MarketQuote streaming equity ──────────────────────────────────────────

    fn make_position(symbol: &str, qty: &str, price: &str) -> crate::types::Position {
        crate::types::Position {
            symbol: symbol.into(),
            qty: qty.into(),
            avg_entry_price: price.into(),
            current_price: price.into(),
            market_value: "0".into(),
            unrealized_pl: "0".into(),
            unrealized_plpc: "0".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }
    }

    #[test]
    fn market_quote_pushes_equity_when_positions_present() {
        let mut app = make_test_app();
        app.account = Some(crate::types::AccountInfo {
            cash: "0.00".into(),
            ..Default::default()
        });
        app.positions = vec![make_position("AAPL", "10", "150.00")];
        update(
            &mut app,
            Event::MarketQuote(Quote {
                symbol: "AAPL".into(),
                ap: Some(200.00),
                bp: None,
                ..Default::default()
            }),
        );
        // Quote stored
        assert!(app.quotes.contains_key("AAPL"));
        // Equity pushed: 10 × $200 = $2000 → 200000 cents
        assert_eq!(
            app.equity_history,
            vec![200_000],
            "equity_history should have streaming sample"
        );
    }

    #[test]
    fn market_quote_no_equity_push_without_positions() {
        let mut app = make_test_app();
        // No positions — push_equity_from_quotes should skip silently
        update(
            &mut app,
            Event::MarketQuote(Quote {
                symbol: "AAPL".into(),
                ap: Some(200.00),
                bp: None,
                ..Default::default()
            }),
        );
        assert!(
            app.equity_history.is_empty(),
            "no equity sample without positions"
        );
    }

    // ── IntradayBarsReceived fetch timestamp ──────────────────────────────────

    #[test]
    fn intraday_bars_received_records_fetched_at_timestamp() {
        let mut app = make_test_app();
        let before = std::time::Instant::now();
        update(
            &mut app,
            Event::IntradayBarsReceived {
                symbol: "MSFT".into(),
                bars: vec![10_000, 10_050],
            },
        );
        let after = std::time::Instant::now();
        let ts = app
            .intraday_fetched_at
            .get("MSFT")
            .expect("fetched_at should be recorded");
        assert!(
            *ts >= before && *ts <= after,
            "fetched_at should be close to now"
        );
        // Bars also stored
        assert_eq!(
            app.intraday_bars.get("MSFT"),
            Some(&vec![10_000u64, 10_050])
        );
    }

    // ── Tick intraday refresh ─────────────────────────────────────────────────

    #[test]
    fn tick_dispatches_intraday_refresh_when_due() {
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(4);
        let (symbol_tx, _) = tokio::sync::watch::channel(vec![]);
        let mut app = crate::app::App::new(
            crate::config::AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: crate::config::AlpacaEnv::Paper,
                dry_run: false,
            },
            crate::prefs::AppPrefs::default(),
            std::sync::Arc::new(tokio::sync::Notify::new()),
            cmd_tx,
            symbol_tx,
        );
        // Open a SymbolDetail modal for AAPL
        app.modal = Some(crate::app::Modal::SymbolDetail("AAPL".into()));
        // Simulate a fetch that happened > 60 seconds ago
        app.intraday_fetched_at.insert(
            "AAPL".into(),
            std::time::Instant::now() - std::time::Duration::from_secs(61),
        );
        update(&mut app, Event::Tick);
        // A FetchIntradayBars command should have been dispatched
        let cmd = cmd_rx.try_recv().expect("command should be dispatched");
        assert!(
            matches!(cmd, Command::FetchIntradayBars(s) if s == "AAPL"),
            "expected FetchIntradayBars for AAPL"
        );
    }

    #[test]
    fn tick_skips_intraday_refresh_when_not_due() {
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(4);
        let (symbol_tx, _) = tokio::sync::watch::channel(vec![]);
        let mut app = crate::app::App::new(
            crate::config::AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: crate::config::AlpacaEnv::Paper,
                dry_run: false,
            },
            crate::prefs::AppPrefs::default(),
            std::sync::Arc::new(tokio::sync::Notify::new()),
            cmd_tx,
            symbol_tx,
        );
        app.modal = Some(crate::app::Modal::SymbolDetail("AAPL".into()));
        // Fetched just now — not due yet
        app.intraday_fetched_at
            .insert("AAPL".into(), std::time::Instant::now());
        update(&mut app, Event::Tick);
        assert!(
            cmd_rx.try_recv().is_err(),
            "no refresh command when interval not elapsed"
        );
    }

    #[test]
    fn tick_skips_intraday_refresh_when_no_modal() {
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(4);
        let (symbol_tx, _) = tokio::sync::watch::channel(vec![]);
        let mut app = crate::app::App::new(
            crate::config::AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: crate::config::AlpacaEnv::Paper,
                dry_run: false,
            },
            crate::prefs::AppPrefs::default(),
            std::sync::Arc::new(tokio::sync::Notify::new()),
            cmd_tx,
            symbol_tx,
        );
        // No modal open
        app.intraday_fetched_at.insert(
            "AAPL".into(),
            std::time::Instant::now() - std::time::Duration::from_secs(61),
        );
        update(&mut app, Event::Tick);
        assert!(
            cmd_rx.try_recv().is_err(),
            "no refresh command without an open modal"
        );
    }

    #[test]
    fn tick_skips_intraday_refresh_when_never_fetched() {
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(4);
        let (symbol_tx, _) = tokio::sync::watch::channel(vec![]);
        let mut app = crate::app::App::new(
            crate::config::AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: crate::config::AlpacaEnv::Paper,
                dry_run: false,
            },
            crate::prefs::AppPrefs::default(),
            std::sync::Arc::new(tokio::sync::Notify::new()),
            cmd_tx,
            symbol_tx,
        );
        // Modal open but bars were never fetched (no entry in intraday_fetched_at)
        app.modal = Some(crate::app::Modal::SymbolDetail("AAPL".into()));
        update(&mut app, Event::Tick);
        assert!(
            cmd_rx.try_recv().is_err(),
            "no refresh command when bars have never been fetched"
        );
    }

    #[test]
    fn tick_dispatches_intraday_refresh_for_position_detail_modal() {
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(4);
        let (symbol_tx, _) = tokio::sync::watch::channel(vec![]);
        let mut app = crate::app::App::new(
            crate::config::AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: crate::config::AlpacaEnv::Paper,
                dry_run: false,
            },
            crate::prefs::AppPrefs::default(),
            std::sync::Arc::new(tokio::sync::Notify::new()),
            cmd_tx,
            symbol_tx,
        );
        app.modal = Some(crate::app::Modal::PositionDetail {
            symbol: "TSLA".into(),
        });
        app.intraday_fetched_at.insert(
            "TSLA".into(),
            std::time::Instant::now() - std::time::Duration::from_secs(61),
        );
        update(&mut app, Event::Tick);
        let cmd = cmd_rx.try_recv().expect("command should be dispatched");
        assert!(
            matches!(cmd, Command::FetchIntradayBars(s) if s == "TSLA"),
            "expected FetchIntradayBars for TSLA"
        );
    }

    // ── p key / equity range toggle ───────────────────────────────────────────

    fn make_app_with_cmd() -> (App, tokio::sync::mpsc::Receiver<Command>) {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(8);
        let (symbol_tx, _) = tokio::sync::watch::channel(vec![]);
        let app = App::new(
            crate::config::AlpacaConfig {
                base_url: "http://localhost".into(),
                key: "k".into(),
                secret: "s".into(),
                env: crate::config::AlpacaEnv::Paper,
                dry_run: false,
            },
            crate::prefs::AppPrefs::default(),
            std::sync::Arc::new(tokio::sync::Notify::new()),
            cmd_tx,
            symbol_tx,
        );
        (app, cmd_rx)
    }

    #[test]
    fn p_key_cycles_equity_range_from_one_day_to_one_week() {
        let (mut app, _rx) = make_app_with_cmd();
        app.active_tab = Tab::Account;
        app.equity_history = vec![1, 2, 3];
        update(&mut app, key(KeyCode::Char('p')));
        assert_eq!(app.equity_range, crate::app::EquityRange::OneWeek);
    }

    #[test]
    fn p_key_clears_equity_history_on_range_change() {
        let (mut app, _rx) = make_app_with_cmd();
        app.active_tab = Tab::Account;
        app.equity_history = vec![1, 2, 3];
        update(&mut app, key(KeyCode::Char('p')));
        assert!(
            app.equity_history.is_empty(),
            "history should be cleared when range changes"
        );
    }

    #[test]
    fn p_key_clears_crosshair_cursor_on_range_change() {
        let (mut app, _rx) = make_app_with_cmd();
        app.active_tab = Tab::Account;
        app.equity_history = vec![1, 2, 3];
        app.equity_chart_cursor = Some(2);
        update(&mut app, key(KeyCode::Char('p')));
        assert!(
            app.equity_chart_cursor.is_none(),
            "cursor should be cleared when range changes"
        );
    }

    #[test]
    fn p_key_dispatches_fetch_portfolio_history_command() {
        let (mut app, mut cmd_rx) = make_app_with_cmd();
        app.active_tab = Tab::Account;
        app.equity_history = vec![1, 2, 3];
        update(&mut app, key(KeyCode::Char('p')));
        let cmd = cmd_rx
            .try_recv()
            .expect("FetchPortfolioHistory command should be dispatched");
        assert!(
            matches!(
                cmd,
                Command::FetchPortfolioHistory { period, timeframe }
                    if period == "1W" && timeframe == "1H"
            ),
            "command should carry the new range's API params"
        );
    }

    #[test]
    fn p_key_dispatches_correct_params_for_ytd() {
        let (mut app, mut cmd_rx) = make_app_with_cmd();
        app.active_tab = Tab::Account;
        app.equity_range = crate::app::EquityRange::OneMonth;
        app.equity_history = vec![1];
        update(&mut app, key(KeyCode::Char('p')));
        let cmd = cmd_rx.try_recv().expect("command dispatched");
        assert!(
            matches!(
                cmd,
                Command::FetchPortfolioHistory { period, timeframe }
                    if period == "YTD" && timeframe == "1D"
            ),
            "expected YTD/1D params"
        );
    }

    #[test]
    fn p_key_sets_status_message_with_new_range_label() {
        let (mut app, _rx) = make_app_with_cmd();
        app.active_tab = Tab::Account;
        app.equity_history = vec![1];
        update(&mut app, key(KeyCode::Char('p')));
        assert_eq!(app.current_status_text(), "Equity range: 1W");
    }

    #[test]
    fn p_key_cycles_all_four_ranges() {
        let (mut app, _rx) = make_app_with_cmd();
        app.active_tab = Tab::Account;
        app.equity_history = vec![1];
        assert_eq!(app.equity_range, crate::app::EquityRange::OneDay);
        update(&mut app, key(KeyCode::Char('p')));
        assert_eq!(app.equity_range, crate::app::EquityRange::OneWeek);
        app.equity_history = vec![1];
        update(&mut app, key(KeyCode::Char('p')));
        assert_eq!(app.equity_range, crate::app::EquityRange::OneMonth);
        app.equity_history = vec![1];
        update(&mut app, key(KeyCode::Char('p')));
        assert_eq!(app.equity_range, crate::app::EquityRange::Ytd);
        app.equity_history = vec![1];
        update(&mut app, key(KeyCode::Char('p')));
        assert_eq!(
            app.equity_range,
            crate::app::EquityRange::OneDay,
            "should wrap back to 1D"
        );
    }

    #[test]
    fn p_key_on_empty_history_is_handled_gracefully() {
        let (mut app, mut cmd_rx) = make_app_with_cmd();
        app.active_tab = Tab::Account;
        // equity_history is empty; p key should still cycle and dispatch command
        update(&mut app, key(KeyCode::Char('p')));
        assert_eq!(app.equity_range, crate::app::EquityRange::OneWeek);
        assert!(
            cmd_rx.try_recv().is_ok(),
            "command should still be dispatched"
        );
    }
}
