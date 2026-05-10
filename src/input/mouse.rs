use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::{App, Modal, OrderField, OrdersSubTab, Tab};

/// Returns `true` if (`col`, `row`) is inside `rect`.
fn hit(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
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
    if hit(tab_bar, col, row) && tab_bar.width >= 4 {
        let tab_w = tab_bar.width / 4;
        let idx = ((col - tab_bar.x) / tab_w).min(3) as usize;
        app.active_tab = Tab::from_index(idx);
        return;
    }

    // ── Orders sub-tab bar ───────────────────────────────────────────────────
    if let Some(subtab_rect) = app.hit_areas.orders_subtab_bar {
        if hit(subtab_rect, col, row) && app.active_tab == Tab::Orders && subtab_rect.width >= 3 {
            let tab_w = subtab_rect.width / 3;
            let idx = ((col - subtab_rect.x) / tab_w).min(2);
            app.orders_subtab = match idx {
                0 => OrdersSubTab::Open,
                1 => OrdersSubTab::Filled,
                _ => OrdersSubTab::Cancelled,
            };
            app.orders_state.select(Some(0));
            return;
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
                    // Left half of the row → BUY, right half → SELL.
                    state.side_buy = col < rect.x + rect.width / 2;
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
