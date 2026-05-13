use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::{App, Modal, OrderField, OrdersSubTab, Tab};

/// The tab labels exactly as rendered by the `Tabs` widget in `dashboard::render_tabs`.
/// Each tab renders as ` <label> ` (one leading/trailing space), width = label.len() + 2.
/// Between tabs there is a `|` divider (1 col).
const TAB_LABELS: &[&str] = &["1:Account", "2:Watchlist", "3:Positions", "4:Orders"];

/// Returns `true` if (`col`, `row`) is inside `rect`.
fn hit(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

/// Compute the exact `Rect` for each tab based on actual rendered label widths.
/// Matches how ratatui's `Tabs` widget lays out: ` label ` then `|` divider between tabs.
fn tab_rects(tab_bar: Rect) -> Vec<Rect> {
    let mut rects = Vec::with_capacity(TAB_LABELS.len());
    let mut x = tab_bar.x;
    for (i, label) in TAB_LABELS.iter().enumerate() {
        let w = label.len() as u16 + 2; // 1 leading space + label + 1 trailing space
        rects.push(Rect {
            x,
            y: tab_bar.y,
            width: w,
            height: 1,
        });
        x += w;
        if i + 1 < TAB_LABELS.len() {
            x += 1; // `|` divider
        }
    }
    rects
}

pub(crate) fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
        return;
    }

    let col = mouse.column;
    let row = mouse.row;

    // Modals have exclusive input priority.
    if app.modal.is_some() {
        handle_modal_mouse(app, col, row);
        return;
    }

    // ── Tab bar ──────────────────────────────────────────────────────────────
    let tab_bar = app.hit_areas.tab_bar;
    if tab_bar.height > 0 {
        for (idx, rect) in tab_rects(tab_bar).iter().enumerate() {
            if hit(*rect, col, row) {
                app.active_tab = Tab::from_index(idx);
                return;
            }
        }
    }

    // ── Orders sub-tab bar ───────────────────────────────────────────────────
    if app.active_tab == Tab::Orders && !app.hit_areas.orders_subtab_rects.is_empty() {
        for (idx, rect) in app.hit_areas.orders_subtab_rects.clone().iter().enumerate() {
            if hit(*rect, col, row) {
                app.orders_subtab = match idx {
                    0 => OrdersSubTab::Open,
                    1 => OrdersSubTab::Filled,
                    _ => OrdersSubTab::Cancelled,
                };
                app.orders_state.select(Some(0));
                return;
            }
        }
    }

    // ── List row ─────────────────────────────────────────────────────────────
    let start_y = app.hit_areas.list_data_start_y;
    if start_y > 0 && row >= start_y {
        let data_row = (row - start_y) as usize;
        let offset = match app.active_tab {
            Tab::Watchlist => app.watchlist_state.offset(),
            Tab::Positions => app.positions_state.offset(),
            Tab::Orders => app.orders_state.offset(),
            Tab::Account => return,
        };
        let idx = data_row + offset;
        match app.active_tab {
            Tab::Watchlist => {
                let len = app.watchlist.as_ref().map(|w| w.assets.len()).unwrap_or(0);
                if idx < len {
                    app.watchlist_state.select(Some(idx));
                }
            }
            Tab::Positions => {
                if idx < app.positions.len() {
                    app.positions_state.select(Some(idx));
                }
            }
            Tab::Orders => {
                let len = app.filtered_orders().len();
                if idx < len {
                    app.orders_state.select(Some(idx));
                }
            }
            Tab::Account => {}
        }
    }
}

fn handle_modal_mouse(app: &mut App, col: u16, row: u16) {
    // ── OrderEntry: submit button ─────────────────────────────────────────────
    if let Some(submit_rect) = app.hit_areas.modal_submit {
        if hit(submit_rect, col, row) {
            if let Some(Modal::OrderEntry(ref mut state)) = app.modal {
                state.focused_field = OrderField::Submit;
            }
            crate::input::handle_modal_key(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            return;
        }
    }

    // ── OrderEntry: field focus + radio button toggles ────────────────────────
    let clicked = app
        .hit_areas
        .modal_fields
        .iter()
        .find(|(_, rect)| hit(*rect, col, row))
        .map(|(field, rect)| (field.clone(), *rect));

    if let Some((field, rect)) = clicked {
        match field {
            OrderField::Side => {
                if let Some(Modal::OrderEntry(ref mut state)) = app.modal {
                    state.focused_field = OrderField::Side;
                    // Left third → BUY, middle third → SELL, right third → SELL SHORT.
                    let third = rect.width / 3;
                    state.side = if col < rect.x + third {
                        crate::app::OrderSide::Buy
                    } else if col < rect.x + 2 * third {
                        crate::app::OrderSide::Sell
                    } else {
                        crate::app::OrderSide::SellShort
                    };
                }
            }
            OrderField::OrderType => {
                if let Some(Modal::OrderEntry(ref mut state)) = app.modal {
                    state.focused_field = OrderField::OrderType;
                    // Left half → LIMIT, right half → MARKET.
                    state.market_order = col >= rect.x + rect.width / 2;
                }
            }
            other => {
                if let Some(Modal::OrderEntry(ref mut state)) = app.modal {
                    state.focused_field = other;
                }
            }
        }
        return;
    }

    // ── Confirm: yes / no buttons ─────────────────────────────────────────────
    if let Some(btn_rect) = app.hit_areas.modal_confirm_buttons {
        if hit(btn_rect, col, row) {
            let code = if col < btn_rect.x + btn_rect.width / 2 {
                KeyCode::Char('y')
            } else {
                KeyCode::Char('n')
            };
            crate::input::handle_modal_key(app, KeyEvent::new(code, KeyModifiers::NONE));
        }
    }
}
