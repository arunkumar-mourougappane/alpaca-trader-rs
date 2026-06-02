use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::{App, FullOrderType, Modal, OrderField, OrdersSubTab, Tab};

/// Maximum interval between two clicks on the same row to be considered a double-click.
const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(400);

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

    // ── Equity chart click (Account tab) ─────────────────────────────────────
    if app.active_tab == Tab::Account {
        let chart_area = app.hit_areas.equity_chart_area;
        if chart_area.height > 0 && hit(chart_area, col, row) {
            let n = app.equity_history.len();
            if n > 0 {
                // Mirror the plot-area offsets used in render_chart_crosshair:
                //   plot_x = area.x + 9,  plot_w = area.width - 11
                let plot_x = chart_area.x + 9;
                let plot_w = chart_area.width.saturating_sub(11);
                if plot_w > 0 && col >= plot_x {
                    let offset = (col - plot_x) as usize;
                    // Map terminal column → data-point index (clamped)
                    let idx = if n <= 1 {
                        0
                    } else {
                        ((offset * (n - 1)) / (plot_w as usize - 1)).min(n - 1)
                    };
                    app.equity_chart_cursor = Some(idx);
                }
            }
        }
        return;
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

        let row_in_bounds = match app.active_tab {
            Tab::Watchlist => {
                let len = app.watchlist.as_ref().map(|w| w.assets.len()).unwrap_or(0);
                if idx < len {
                    app.watchlist_state.select(Some(idx));
                    true
                } else {
                    false
                }
            }
            Tab::Positions => {
                if idx < app.positions.len() {
                    app.positions_state.select(Some(idx));
                    true
                } else {
                    false
                }
            }
            Tab::Orders => {
                let len = app.filtered_orders().len();
                if idx < len {
                    app.orders_state.select(Some(idx));
                    true
                } else {
                    false
                }
            }
            Tab::Account => false,
        };

        if row_in_bounds {
            // Double-click: same row within DOUBLE_CLICK_INTERVAL → open detail (Enter)
            let is_double = app.last_click.as_ref().is_some_and(|(last_row, last_at)| {
                *last_row == row && last_at.elapsed() < DOUBLE_CLICK_INTERVAL
            });
            if is_double {
                app.last_click = None;
                crate::input::handle_key_as_enter(app);
            } else {
                app.last_click = Some((row, Instant::now()));
            }
        }
    }
}

fn handle_modal_mouse(app: &mut App, col: u16, row: u16) {
    // ── Click outside modal popup → dismiss ───────────────────────────────────
    if let Some(popup) = app.hit_areas.modal_popup_area {
        if !hit(popup, col, row) {
            // Only dismiss modals that don't require explicit confirmation.
            let dismissable = matches!(
                &app.modal,
                Some(Modal::Help)
                    | Some(Modal::About)
                    | Some(Modal::SymbolDetail(_))
                    | Some(Modal::PositionDetail { .. })
            );
            if dismissable {
                app.modal = None;
                return;
            }
        }
    }

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
                    // Map click column across 5 equal sections →
                    // MARKET | LIMIT | STOP | STOP-LMT | TRAIL
                    let w = rect.width.max(5) as usize;
                    let offset = col.saturating_sub(rect.x) as usize;
                    let section = (offset * 5 / w).min(4);
                    state.order_type = match section {
                        0 => FullOrderType::Market,
                        1 => FullOrderType::Limit,
                        2 => FullOrderType::Stop,
                        3 => FullOrderType::StopLimit,
                        _ => FullOrderType::TrailingStop,
                    };
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

#[cfg(test)]
mod tests {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    use crate::app::test_helpers::make_test_app;
    use crate::app::{Modal, Tab};

    fn left_click(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    fn right_click(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    fn scroll_down(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    // ── non-Left-click is a no-op ─────────────────────────────────────────────

    #[test]
    fn right_click_is_ignored() {
        let mut app = make_test_app();
        app.hit_areas.tab_bar = Rect::new(0, 0, 80, 1);
        super::handle_mouse(&mut app, right_click(5, 0));
        // No tab switch
        assert_eq!(app.active_tab, Tab::Account);
    }

    #[test]
    fn scroll_event_is_ignored() {
        let mut app = make_test_app();
        app.hit_areas.tab_bar = Rect::new(0, 0, 80, 1);
        super::handle_mouse(&mut app, scroll_down(5, 0));
        assert_eq!(app.active_tab, Tab::Account);
    }

    // ── Tab bar ───────────────────────────────────────────────────────────────

    #[test]
    fn click_tab_bar_switches_to_watchlist() {
        let mut app = make_test_app();
        // Tab bar at row 0. "1:Account"=11 chars → width 13, then "|" → x=14.
        // "2:Watchlist"=11 chars → width 13, starts at x=14.
        app.hit_areas.tab_bar = Rect::new(0, 0, 80, 1);
        // x=15 is inside "2:Watchlist" tab rect
        super::handle_mouse(&mut app, left_click(15, 0));
        assert_eq!(app.active_tab, Tab::Watchlist);
    }

    #[test]
    fn click_tab_bar_outside_any_tab_is_noop() {
        let mut app = make_test_app();
        // height=0 → tab bar disabled
        app.hit_areas.tab_bar = Rect::new(0, 0, 80, 0);
        super::handle_mouse(&mut app, left_click(5, 0));
        assert_eq!(app.active_tab, Tab::Account);
    }

    // ── List row selection ────────────────────────────────────────────────────

    #[test]
    fn single_click_row_selects_position() {
        let mut app = make_test_app();
        app.active_tab = Tab::Positions;
        app.positions.push(crate::types::Position {
            symbol: "AAPL".into(),
            qty: "1".into(),
            avg_entry_price: "100".into(),
            current_price: "110".into(),
            market_value: "110".into(),
            unrealized_pl: "10".into(),
            unrealized_plpc: "0.1".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        });
        app.positions.push(crate::types::Position {
            symbol: "TSLA".into(),
            qty: "2".into(),
            avg_entry_price: "200".into(),
            current_price: "210".into(),
            market_value: "420".into(),
            unrealized_pl: "20".into(),
            unrealized_plpc: "0.05".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        });
        app.hit_areas.list_data_start_y = 5;
        // Click on row 6 → data_row=1 (second position)
        super::handle_mouse(&mut app, left_click(10, 6));
        assert_eq!(app.positions_state.selected(), Some(1));
    }

    #[test]
    fn single_click_out_of_bounds_row_is_ignored() {
        let mut app = make_test_app();
        app.active_tab = Tab::Positions;
        app.hit_areas.list_data_start_y = 5;
        // No positions, click at row 6 → idx=1 >= len=0 → noop
        super::handle_mouse(&mut app, left_click(10, 6));
        assert_eq!(app.positions_state.selected(), None);
    }

    // ── Double-click opens detail modal ───────────────────────────────────────

    #[test]
    fn double_click_watchlist_row_opens_symbol_detail() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        app.watchlist = Some(crate::types::Watchlist {
            id: "w1".into(),
            name: "Default".into(),
            assets: vec![crate::types::Asset {
                exchange: "NASDAQ".into(),
                tradable: true,
                shortable: true,
                fractionable: true,
                easy_to_borrow: true,
                id: "a1".into(),
                symbol: "AAPL".into(),
                name: "Apple Inc.".into(),
                asset_class: "us_equity".into(),
            }],
        });
        app.watchlist_state.select(Some(0));
        app.hit_areas.list_data_start_y = 3;

        // First click: selects row, records last_click
        super::handle_mouse(&mut app, left_click(10, 3));
        assert!(app.modal.is_none(), "first click should not open modal");
        assert!(app.last_click.is_some(), "last_click should be set");

        // Second click immediately after: should open SymbolDetail
        super::handle_mouse(&mut app, left_click(10, 3));
        assert!(
            matches!(&app.modal, Some(Modal::SymbolDetail(s)) if s == "AAPL"),
            "double-click should open SymbolDetail, got: {:?}",
            app.modal
        );
    }

    #[test]
    fn double_click_positions_row_opens_position_detail() {
        let mut app = make_test_app();
        app.active_tab = Tab::Positions;
        app.positions.push(crate::types::Position {
            symbol: "TSLA".into(),
            qty: "5".into(),
            avg_entry_price: "200".into(),
            current_price: "220".into(),
            market_value: "1100".into(),
            unrealized_pl: "100".into(),
            unrealized_plpc: "0.1".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        });
        app.positions_state.select(Some(0));
        app.hit_areas.list_data_start_y = 3;

        super::handle_mouse(&mut app, left_click(10, 3));
        assert!(app.modal.is_none());

        super::handle_mouse(&mut app, left_click(10, 3));
        assert!(
            matches!(&app.modal, Some(Modal::PositionDetail { symbol }) if symbol == "TSLA"),
            "double-click should open PositionDetail, got: {:?}",
            app.modal
        );
    }

    #[test]
    fn double_click_on_different_row_does_not_open_modal() {
        let mut app = make_test_app();
        app.active_tab = Tab::Positions;
        app.positions.push(crate::types::Position {
            symbol: "A".into(),
            qty: "1".into(),
            avg_entry_price: "1".into(),
            current_price: "1".into(),
            market_value: "1".into(),
            unrealized_pl: "0".into(),
            unrealized_plpc: "0".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        });
        app.positions.push(crate::types::Position {
            symbol: "B".into(),
            qty: "1".into(),
            avg_entry_price: "1".into(),
            current_price: "1".into(),
            market_value: "1".into(),
            unrealized_pl: "0".into(),
            unrealized_plpc: "0".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        });
        app.hit_areas.list_data_start_y = 3;

        // Click row 3 (first position)
        super::handle_mouse(&mut app, left_click(10, 3));
        // Click row 4 (second position) — different row, no modal
        super::handle_mouse(&mut app, left_click(10, 4));
        assert!(
            app.modal.is_none(),
            "clicks on different rows should not trigger double-click"
        );
    }

    // ── Outside-modal dismiss ─────────────────────────────────────────────────

    #[test]
    fn click_outside_help_modal_dismisses_it() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Help);
        // Popup occupies columns 20-60, rows 5-30
        app.hit_areas.modal_popup_area = Some(Rect::new(20, 5, 40, 25));
        // Click at (0, 0) — outside the popup
        super::handle_mouse(&mut app, left_click(0, 0));
        assert!(app.modal.is_none(), "click outside Help should dismiss it");
    }

    #[test]
    fn click_inside_help_modal_does_not_dismiss() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Help);
        app.hit_areas.modal_popup_area = Some(Rect::new(20, 5, 40, 25));
        // Click inside popup
        super::handle_mouse(&mut app, left_click(30, 10));
        // Help modal stays (no other interactive elements inside it to change modal to None)
        // It stays as Help unless a key dismisses it
        assert!(
            app.modal.is_some(),
            "click inside Help should not dismiss it"
        );
    }

    #[test]
    fn click_outside_order_entry_modal_does_not_dismiss() {
        let mut app = make_test_app();
        app.modal = Some(Modal::OrderEntry(crate::app::OrderEntryState::new(
            "AAPL".into(),
        )));
        app.hit_areas.modal_popup_area = Some(Rect::new(20, 5, 40, 40));
        // Click outside — OrderEntry is not in the dismissable set
        super::handle_mouse(&mut app, left_click(0, 0));
        assert!(
            app.modal.is_some(),
            "OrderEntry should not be dismissable by outside click"
        );
    }

    #[test]
    fn click_outside_symbol_detail_dismisses_it() {
        let mut app = make_test_app();
        app.modal = Some(Modal::SymbolDetail("AAPL".into()));
        app.hit_areas.modal_popup_area = Some(Rect::new(10, 3, 60, 40));
        super::handle_mouse(&mut app, left_click(0, 0));
        assert!(
            app.modal.is_none(),
            "click outside SymbolDetail should dismiss it"
        );
    }

    #[test]
    fn click_outside_position_detail_dismisses_it() {
        let mut app = make_test_app();
        app.modal = Some(Modal::PositionDetail {
            symbol: "TSLA".into(),
        });
        app.hit_areas.modal_popup_area = Some(Rect::new(10, 3, 60, 40));
        super::handle_mouse(&mut app, left_click(0, 0));
        assert!(
            app.modal.is_none(),
            "click outside PositionDetail should dismiss it"
        );
    }

    #[test]
    fn no_modal_popup_area_does_not_panic() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Help);
        app.hit_areas.modal_popup_area = None;
        // Should not dismiss (no popup area registered) and not panic
        super::handle_mouse(&mut app, left_click(0, 0));
        // Help stays (no popup registered, so no dismiss logic runs, but
        // handle_modal_mouse falls through to other handlers and nothing changes)
        // This test just verifies no panic occurs.
    }

    #[test]
    fn click_outside_about_modal_dismisses_it() {
        let mut app = make_test_app();
        app.modal = Some(Modal::About);
        app.hit_areas.modal_popup_area = Some(Rect::new(20, 5, 40, 25));
        super::handle_mouse(&mut app, left_click(0, 0));
        assert!(app.modal.is_none(), "click outside About should dismiss it");
    }

    #[test]
    fn single_click_orders_row_selects_item() {
        use crate::types::Order;
        let mut app = make_test_app();
        app.active_tab = Tab::Orders;
        app.orders.push(Order {
            id: "o1".into(),
            symbol: "AAPL".into(),
            side: "buy".into(),
            qty: Some("1".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: "new".into(),
            submitted_at: None,
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        app.hit_areas.list_data_start_y = 4;
        super::handle_mouse(&mut app, left_click(10, 4));
        assert_eq!(app.orders_state.selected(), Some(0));
    }

    // ── 5-way OrderType click tests ───────────────────────────────────────────

    fn order_entry_with_order_type_hit_area(rect: Rect) -> crate::app::App {
        let mut app = make_test_app();
        let state = crate::app::OrderEntryState::new("AAPL".into());
        app.modal = Some(Modal::OrderEntry(state));
        app.hit_areas.modal_fields = vec![(crate::app::OrderField::OrderType, rect)];
        app
    }

    #[test]
    fn click_order_type_section_0_selects_market() {
        use crate::app::FullOrderType;
        // rect x=0, width=50; offset 0 → section 0*5/50=0 → Market
        let mut app = order_entry_with_order_type_hit_area(Rect::new(0, 5, 50, 1));
        super::handle_mouse(&mut app, left_click(0, 5));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.order_type == FullOrderType::Market)
        );
    }

    #[test]
    fn click_order_type_section_1_selects_limit() {
        use crate::app::FullOrderType;
        // rect x=0, width=50; offset 10 → section 10*5/50=1 → Limit
        let mut app = order_entry_with_order_type_hit_area(Rect::new(0, 5, 50, 1));
        super::handle_mouse(&mut app, left_click(10, 5));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.order_type == FullOrderType::Limit)
        );
    }

    #[test]
    fn click_order_type_section_2_selects_stop() {
        use crate::app::FullOrderType;
        // rect x=0, width=50; offset 20 → section 20*5/50=2 → Stop
        let mut app = order_entry_with_order_type_hit_area(Rect::new(0, 5, 50, 1));
        super::handle_mouse(&mut app, left_click(20, 5));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.order_type == FullOrderType::Stop)
        );
    }

    #[test]
    fn click_order_type_section_3_selects_stop_limit() {
        use crate::app::FullOrderType;
        // rect x=0, width=50; offset 30 → section 30*5/50=3 → StopLimit
        let mut app = order_entry_with_order_type_hit_area(Rect::new(0, 5, 50, 1));
        super::handle_mouse(&mut app, left_click(30, 5));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.order_type == FullOrderType::StopLimit)
        );
    }

    #[test]
    fn click_order_type_section_4_selects_trailing_stop() {
        use crate::app::FullOrderType;
        // rect x=0, width=50; offset 40 → section 40*5/50=4 → TrailingStop
        let mut app = order_entry_with_order_type_hit_area(Rect::new(0, 5, 50, 1));
        super::handle_mouse(&mut app, left_click(40, 5));
        assert!(
            matches!(&app.modal, Some(Modal::OrderEntry(s)) if s.order_type == FullOrderType::TrailingStop)
        );
    }
}
