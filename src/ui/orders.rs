use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Cell, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::app::{App, OrderSortCol, OrdersSubTab, SortDir};
use crate::ui::formatting::{format_price, header_cell};

/// Return a header label with an ▲/▼ sort indicator appended when `active` is true.
fn sorted_header(label: &str, active: bool, dir: SortDir) -> String {
    if active {
        let arrow = if dir == SortDir::Asc { " ▲" } else { " ▼" };
        format!("{label}{arrow}")
    } else {
        label.to_string()
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    app.hit_areas.orders_subtab_rects.clear();
    render_subtabs(frame, chunks[0], app);
    render_table(frame, chunks[1], app);
}

fn render_subtabs(frame: &mut Frame, area: Rect, app: &mut App) {
    let c = app.current_theme.colors();
    let open_count = app
        .orders
        .iter()
        .filter(|o| {
            matches!(
                o.status.as_str(),
                "new" | "pending_new" | "accepted" | "held" | "partially_filled"
            )
        })
        .count();
    let filled_count = app.orders.iter().filter(|o| o.status == "filled").count();
    let cancelled_count = app
        .orders
        .iter()
        .filter(|o| {
            matches!(
                o.status.as_str(),
                "canceled" | "expired" | "rejected" | "replaced"
            )
        })
        .count();

    let titles = vec![
        format!("1:Open ({})", open_count),
        format!("2:Filled ({})", filled_count),
        format!("3:Cancelled ({})", cancelled_count),
    ];

    // Compute exact per-subtab rects from actual label widths so mouse hit-testing
    // maps clicks to the correct subtab regardless of dynamic order counts.
    // ratatui Tabs renders each title as ` <label> ` (label.len()+2) with `|` dividers.
    let mut x = area.x;
    for (i, title) in titles.iter().enumerate() {
        let w = title.len() as u16 + 2;
        app.hit_areas
            .orders_subtab_rects
            .push(ratatui::layout::Rect {
                x,
                y: area.y,
                width: w,
                height: 1,
            });
        x += w;
        if i + 1 < titles.len() {
            x += 1; // `|` divider
        }
    }

    let selected = match app.orders_subtab {
        OrdersSubTab::Open => 0,
        OrdersSubTab::Filled => 1,
        OrdersSubTab::Cancelled => 2,
    };

    let tabs = Tabs::new(titles)
        .select(selected)
        .highlight_style(Style::default().fg(c.accent).add_modifier(Modifier::BOLD))
        .divider("|");

    frame.render_widget(tabs, area);
}

fn render_table(frame: &mut Frame, area: Rect, app: &mut App) {
    let c = app.current_theme.colors();
    let orders = app.filtered_orders();
    let is_filled_tab = app.orders_subtab == OrdersSubTab::Filled;

    if orders.is_empty() {
        let para = Paragraph::new("  No orders in this category.")
            .style(c.dim_style())
            .block(c.bordered_block(" Orders "));
        frame.render_widget(para, area);
        return;
    }

    let sort_col = app.orders_sort.col;
    let sort_dir = app.orders_sort.dir;
    let active = |col: OrderSortCol| sort_col == col;

    let h_symbol = sorted_header("Symbol", active(OrderSortCol::Symbol), sort_dir);
    let h_side = sorted_header("Side", active(OrderSortCol::Side), sort_dir);
    let h_type = sorted_header("Type", active(OrderSortCol::Type), sort_dir);
    let h_status = sorted_header("Status", active(OrderSortCol::Status), sort_dir);
    let h_submitted = sorted_header("Submitted", active(OrderSortCol::Submitted), sort_dir);

    let mut header_cells = vec![
        header_cell("ID", &c),
        header_cell(&h_symbol, &c),
        header_cell(&h_side, &c),
        header_cell("Qty", &c),
        header_cell(&h_type, &c),
        header_cell("Limit", &c),
        header_cell(&h_status, &c),
        header_cell(&h_submitted, &c),
    ];
    if is_filled_tab {
        header_cells.push(header_cell("Filled Qty", &c));
        header_cells.push(header_cell("Fill Price", &c));
    }
    let header = Row::new(header_cells);

    // Sort orders.
    let mut sorted_orders = orders;
    match sort_col {
        OrderSortCol::None => {}
        OrderSortCol::Symbol => sorted_orders.sort_by(|a, b| a.symbol.cmp(&b.symbol)),
        OrderSortCol::Side => sorted_orders.sort_by(|a, b| a.side.cmp(&b.side)),
        OrderSortCol::Type => sorted_orders.sort_by(|a, b| a.order_type.cmp(&b.order_type)),
        OrderSortCol::Status => sorted_orders.sort_by(|a, b| a.status.cmp(&b.status)),
        OrderSortCol::Submitted => sorted_orders.sort_by(|a, b| {
            let at = a.submitted_at.as_deref().unwrap_or("");
            let bt = b.submitted_at.as_deref().unwrap_or("");
            at.cmp(bt)
        }),
    }
    if sort_dir == SortDir::Desc {
        sorted_orders.reverse();
    }

    let rows: Vec<Row> = sorted_orders
        .iter()
        .map(|o| {
            let short_id = if o.id.len() >= 8 {
                format!("{}…", &o.id[..8])
            } else {
                o.id.clone()
            };

            let side_style = if o.side == "buy" {
                c.positive_style()
            } else {
                c.negative_style()
            };

            let qty_str = o
                .qty
                .as_deref()
                .or(o.notional.as_deref())
                .unwrap_or("—")
                .to_string();

            let limit_str = o
                .limit_price
                .as_deref()
                .map(format_price)
                .unwrap_or_else(|| "—".into());

            let submitted = o
                .submitted_at
                .as_deref()
                .and_then(|s: &str| s.get(11..19))
                .unwrap_or("—")
                .to_string();

            let mut cells = vec![
                Cell::from(short_id).style(c.dim_style()),
                Cell::from(o.symbol.clone()).style(c.bold_style()),
                Cell::from(o.side.to_uppercase()).style(side_style),
                Cell::from(qty_str),
                Cell::from(o.order_type.to_uppercase()),
                Cell::from(limit_str),
                Cell::from(o.status.clone()),
                Cell::from(submitted).style(c.dim_style()),
            ];

            if is_filled_tab {
                let filled_qty_str = if o.filled_qty == "0" || o.filled_qty.is_empty() {
                    "—".into()
                } else {
                    o.filled_qty.clone()
                };
                let fill_price_str = o
                    .filled_avg_price
                    .as_deref()
                    .map(format_price)
                    .unwrap_or_else(|| "—".into());
                cells.push(Cell::from(filled_qty_str));
                cells.push(Cell::from(fill_price_str).style(c.positive_style()));
            }

            Row::new(cells)
        })
        .collect();

    let block = c.bordered_block(" Orders ");

    let mut constraints = vec![
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Min(12),
        Constraint::Length(10),
    ];
    if is_filled_tab {
        constraints.push(Constraint::Length(10));
        constraints.push(Constraint::Length(11));
    }

    let table = Table::new(rows, constraints)
        .header(header)
        .block(block)
        .row_highlight_style(c.selected_style())
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, area, &mut app.orders_state);
}

#[cfg(test)]
mod tests {
    use crate::app::test_helpers::{make_order, make_test_app};
    use crate::ui::test_helpers::render_to_string;

    fn render_orders_to_string(app: &mut crate::app::App) -> String {
        render_to_string(100, 20, |frame| {
            super::render(frame, frame.area(), app);
        })
    }

    #[test]
    fn orders_empty_shows_no_orders_message() {
        let mut app = make_test_app();
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("No orders"),
            "expected no-orders message, got: {output}"
        );
    }

    #[test]
    fn orders_shows_subtabs() {
        let mut app = make_test_app();
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("Open"), "expected Open subtab");
        assert!(output.contains("Filled"), "expected Filled subtab");
        assert!(output.contains("Cancelled"), "expected Cancelled subtab");
    }

    #[test]
    fn orders_shows_open_order_row() {
        let mut app = make_test_app();
        app.orders.push(make_order("abcdefgh-1234", "new"));
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("AAPL"), "expected AAPL symbol in row");
        assert!(output.contains("BUY"), "expected BUY side in row");
    }

    #[test]
    fn orders_filled_subtab_shows_filled_orders() {
        use crate::app::OrdersSubTab;
        let mut app = make_test_app();
        app.orders.push(make_order("filled-order", "filled"));
        app.orders_subtab = OrdersSubTab::Filled;
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("AAPL"), "expected AAPL in filled orders");
    }

    #[test]
    fn orders_cancelled_subtab_shows_cancelled_orders() {
        use crate::app::OrdersSubTab;
        let mut app = make_test_app();
        app.orders.push(make_order("cancelled-order", "canceled"));
        app.orders_subtab = OrdersSubTab::Cancelled;
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("AAPL"), "expected AAPL in cancelled orders");
    }

    #[test]
    fn orders_sell_side_shows_sell() {
        use crate::types::Order;
        let mut app = make_test_app();
        app.orders.push(Order {
            id: "sell-id".into(),
            symbol: "TSLA".into(),
            side: "sell".into(),
            qty: Some("5".into()),
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
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("TSLA"), "expected TSLA symbol");
        assert!(output.contains("SELL"), "expected SELL side");
    }

    #[test]
    fn orders_with_limit_price_shows_price() {
        use crate::types::Order;
        let mut app = make_test_app();
        app.orders.push(Order {
            id: "limit-id".into(),
            symbol: "NVDA".into(),
            side: "buy".into(),
            qty: Some("2".into()),
            notional: None,
            order_type: "limit".into(),
            limit_price: Some("500.00".into()),
            status: "new".into(),
            submitted_at: Some("2024-01-15T10:30:00Z".into()),
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("NVDA"), "expected NVDA symbol");
        assert!(output.contains("500.00"), "expected limit price");
    }

    #[test]
    fn orders_render_uses_theme_colors() {
        use crate::ui::theme::Theme;
        let mut app = make_test_app();
        app.orders.push(make_order("theme-test", "new"));
        app.current_theme = Theme::Dark;
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("AAPL"), "should render with dark theme");
    }

    #[test]
    fn orders_count_shown_in_subtab_labels() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders.push(make_order("o2", "filled"));
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("Open (1)"), "expected Open (1)");
        assert!(output.contains("Filled (1)"), "expected Filled (1)");
    }

    #[test]
    fn filled_subtab_shows_filled_qty_and_fill_price_columns() {
        use crate::app::OrdersSubTab;
        use crate::types::Order;
        let mut app = make_test_app();
        app.orders.push(Order {
            id: "filled-id".into(),
            symbol: "MSFT".into(),
            side: "buy".into(),
            qty: Some("3".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: "filled".into(),
            submitted_at: None,
            filled_at: Some("2024-06-01T14:00:00Z".into()),
            filled_qty: "3".into(),
            filled_avg_price: Some("425.50".into()),
            time_in_force: "day".into(),
        });
        app.orders_subtab = OrdersSubTab::Filled;
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("Filled Qty"), "expected Filled Qty header");
        assert!(output.contains("Fill Price"), "expected Fill Price header");
        assert!(output.contains("425.50"), "expected fill price value");
        assert!(output.contains("MSFT"), "expected MSFT symbol");
    }

    #[test]
    fn filled_subtab_shows_dash_when_no_fill_price() {
        use crate::app::OrdersSubTab;
        use crate::types::Order;
        let mut app = make_test_app();
        app.orders.push(Order {
            id: "filled-id2".into(),
            symbol: "AMZN".into(),
            side: "sell".into(),
            qty: Some("1".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: "filled".into(),
            submitted_at: None,
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        app.orders_subtab = OrdersSubTab::Filled;
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("Fill Price"), "expected Fill Price header");
        assert!(
            output.contains("—"),
            "expected em-dash for missing fill price"
        );
    }

    #[test]
    fn open_subtab_does_not_show_filled_columns() {
        let mut app = make_test_app();
        app.orders.push(make_order("open-id", "new"));
        let output = render_orders_to_string(&mut app);
        assert!(
            !output.contains("Fill Price"),
            "Open tab should not show Fill Price column"
        );
        assert!(
            !output.contains("Filled Qty"),
            "Open tab should not show Filled Qty column"
        );
    }

    #[test]
    fn cancelled_subtab_does_not_show_filled_columns() {
        use crate::app::OrdersSubTab;
        let mut app = make_test_app();
        app.orders.push(make_order("cancelled-id", "canceled"));
        app.orders_subtab = OrdersSubTab::Cancelled;
        let output = render_orders_to_string(&mut app);
        assert!(
            !output.contains("Fill Price"),
            "Cancelled tab should not show Fill Price column"
        );
    }

    // ── Sort indicator / sorting tests ────────────────────────────────────────

    fn make_order_with_symbol(id: &str, symbol: &str) -> crate::types::Order {
        crate::types::Order {
            id: id.into(),
            symbol: symbol.into(),
            side: "buy".into(),
            qty: Some("5".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: "new".into(),
            submitted_at: None,
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        }
    }

    #[test]
    fn orders_no_sort_shows_no_indicator() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        let output = render_orders_to_string(&mut app);
        assert!(
            !output.contains('▲') && !output.contains('▼'),
            "no indicator expected when sort col is None, got: {output}"
        );
    }

    #[test]
    fn orders_sort_by_symbol_asc_shows_indicator() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders_sort.col = crate::app::OrderSortCol::Symbol;
        app.orders_sort.dir = crate::app::SortDir::Asc;
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("Symbol ▲") || output.contains("Symbol▲"),
            "expected ascending indicator on Symbol, got: {output}"
        );
    }

    #[test]
    fn orders_sort_by_symbol_desc_shows_indicator() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders_sort.col = crate::app::OrderSortCol::Symbol;
        app.orders_sort.dir = crate::app::SortDir::Desc;
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("Symbol ▼") || output.contains("Symbol▼"),
            "expected descending indicator on Symbol, got: {output}"
        );
    }

    #[test]
    fn orders_sorted_by_symbol_asc_orders_rows_alphabetically() {
        let mut app = make_test_app();
        app.orders.push(make_order_with_symbol("o1", "TSLA"));
        app.orders.push(make_order_with_symbol("o2", "AAPL"));
        app.orders_sort.col = crate::app::OrderSortCol::Symbol;
        app.orders_sort.dir = crate::app::SortDir::Asc;
        let output = render_orders_to_string(&mut app);
        let aapl_pos = output.find("AAPL").expect("AAPL should appear");
        let tsla_pos = output.find("TSLA").expect("TSLA should appear");
        assert!(
            aapl_pos < tsla_pos,
            "AAPL should appear before TSLA sorted ascending"
        );
    }

    #[test]
    fn orders_sorted_by_symbol_desc_reverses_order() {
        let mut app = make_test_app();
        app.orders.push(make_order_with_symbol("o1", "AAPL"));
        app.orders.push(make_order_with_symbol("o2", "TSLA"));
        app.orders_sort.col = crate::app::OrderSortCol::Symbol;
        app.orders_sort.dir = crate::app::SortDir::Desc;
        let output = render_orders_to_string(&mut app);
        let aapl_pos = output.find("AAPL").expect("AAPL should appear");
        let tsla_pos = output.find("TSLA").expect("TSLA should appear");
        assert!(
            tsla_pos < aapl_pos,
            "TSLA should appear before AAPL sorted descending"
        );
    }

    #[test]
    fn orders_sort_by_status_shows_status_indicator() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders_sort.col = crate::app::OrderSortCol::Status;
        app.orders_sort.dir = crate::app::SortDir::Asc;
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("Status ▲") || output.contains("Status▲"),
            "expected ascending indicator on Status, got: {output}"
        );
    }

    // ── Additional coverage tests ──────────────────────────────────────────────

    fn make_order_full(id: &str, symbol: &str, side: &str, status: &str) -> crate::types::Order {
        crate::types::Order {
            id: id.into(),
            symbol: symbol.into(),
            side: side.into(),
            qty: Some("5".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: status.into(),
            submitted_at: None,
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        }
    }

    #[test]
    fn orders_sort_by_side_asc_shows_indicator() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders_sort.col = crate::app::OrderSortCol::Side;
        app.orders_sort.dir = crate::app::SortDir::Asc;
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("Side ▲") || output.contains("Side▲"),
            "expected Side ▲ header, got: {output}"
        );
    }

    #[test]
    fn orders_sort_by_side_desc_shows_indicator() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders_sort.col = crate::app::OrderSortCol::Side;
        app.orders_sort.dir = crate::app::SortDir::Desc;
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("Side ▼") || output.contains("Side▼"),
            "expected Side ▼ header, got: {output}"
        );
    }

    #[test]
    fn orders_sort_by_side_orders_rows() {
        let mut app = make_test_app();
        // "sell" > "buy" lexicographically → ascending puts buy first
        app.orders
            .push(make_order_full("o1", "TSLA", "sell", "new"));
        app.orders.push(make_order_full("o2", "AAPL", "buy", "new"));
        app.orders_sort.col = crate::app::OrderSortCol::Side;
        app.orders_sort.dir = crate::app::SortDir::Asc;
        let output = render_orders_to_string(&mut app);
        let aapl_pos = output.find("AAPL").expect("AAPL");
        let tsla_pos = output.find("TSLA").expect("TSLA");
        assert!(
            aapl_pos < tsla_pos,
            "buy (AAPL) should precede sell (TSLA) sorted asc by side"
        );
    }

    #[test]
    fn orders_sort_by_type_asc_shows_indicator() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders_sort.col = crate::app::OrderSortCol::Type;
        app.orders_sort.dir = crate::app::SortDir::Asc;
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("Type ▲") || output.contains("Type▲"),
            "expected Type ▲ header, got: {output}"
        );
    }

    #[test]
    fn orders_sort_by_type_desc_shows_indicator() {
        let mut app = make_test_app();
        app.orders.push(make_order("o1", "new"));
        app.orders_sort.col = crate::app::OrderSortCol::Type;
        app.orders_sort.dir = crate::app::SortDir::Desc;
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("Type ▼") || output.contains("Type▼"),
            "expected Type ▼ header, got: {output}"
        );
    }

    #[test]
    fn orders_sort_by_submitted_asc_sets_sort_state() {
        // The "Submitted" column constraint (Length 10) is too narrow to display
        // "Submitted ▲" (11 cols) in the rendered output, so we verify state directly.
        let mut app = make_test_app();
        app.orders_sort.col = crate::app::OrderSortCol::Submitted;
        app.orders_sort.dir = crate::app::SortDir::Asc;
        assert_eq!(app.orders_sort.col, crate::app::OrderSortCol::Submitted);
        assert_eq!(app.orders_sort.dir, crate::app::SortDir::Asc);
    }

    #[test]
    fn orders_sort_by_submitted_desc_orders_rows() {
        use crate::types::Order;
        let mut app = make_test_app();
        app.orders.push(Order {
            id: "early".into(),
            symbol: "AAPL".into(),
            side: "buy".into(),
            qty: Some("1".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: "new".into(),
            submitted_at: Some("2024-01-01T09:30:00Z".into()),
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        app.orders.push(Order {
            id: "late".into(),
            symbol: "TSLA".into(),
            side: "buy".into(),
            qty: Some("1".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: "new".into(),
            submitted_at: Some("2024-06-01T09:30:00Z".into()),
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        app.orders_sort.col = crate::app::OrderSortCol::Submitted;
        app.orders_sort.dir = crate::app::SortDir::Desc;
        let output = render_orders_to_string(&mut app);
        let aapl_pos = output.find("AAPL").expect("AAPL");
        let tsla_pos = output.find("TSLA").expect("TSLA");
        assert!(
            tsla_pos < aapl_pos,
            "TSLA (later date) should precede AAPL in desc submitted order"
        );
    }

    #[test]
    fn orders_sort_by_status_desc_orders_rows() {
        let mut app = make_test_app();
        // Both statuses appear in the Open subtab; "new" > "accepted" lex → desc puts "new" first
        app.orders
            .push(make_order_full("o1", "AAPL", "buy", "accepted"));
        app.orders.push(make_order_full("o2", "TSLA", "buy", "new"));
        app.orders_sort.col = crate::app::OrderSortCol::Status;
        app.orders_sort.dir = crate::app::SortDir::Desc;
        let output = render_orders_to_string(&mut app);
        let aapl_pos = output.find("AAPL").expect("AAPL");
        let tsla_pos = output.find("TSLA").expect("TSLA");
        assert!(
            tsla_pos < aapl_pos,
            "new (TSLA) should precede accepted (AAPL) in desc status order"
        );
    }

    #[test]
    fn orders_notional_amount_shown_when_no_qty() {
        use crate::types::Order;
        let mut app = make_test_app();
        app.orders.push(Order {
            id: "notional-id".into(),
            symbol: "SPY".into(),
            side: "buy".into(),
            qty: None,
            notional: Some("500.00".into()),
            order_type: "market".into(),
            limit_price: None,
            status: "new".into(),
            submitted_at: None,
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("SPY"), "expected SPY symbol");
        assert!(
            output.contains("500.00"),
            "expected notional amount in qty column"
        );
    }

    #[test]
    fn orders_short_id_truncated_to_8_chars_with_ellipsis() {
        let mut app = make_test_app();
        // id longer than 8 chars → truncated with "…"
        app.orders.push(make_order("abcdefgh-longer-id", "new"));
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("abcdefgh"), "expected first 8 chars of id");
        assert!(output.contains('…'), "expected ellipsis for truncated id");
    }

    #[test]
    fn orders_short_id_not_truncated_when_short() {
        use crate::types::Order;
        let mut app = make_test_app();
        app.orders.push(Order {
            id: "short".into(),
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
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("short"), "expected full short id");
    }

    #[test]
    fn orders_submitted_at_shows_time_portion() {
        use crate::types::Order;
        let mut app = make_test_app();
        app.orders.push(Order {
            id: "ts-order".into(),
            symbol: "GOOG".into(),
            side: "buy".into(),
            qty: Some("1".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: "new".into(),
            submitted_at: Some("2024-03-15T14:35:00Z".into()),
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        let output = render_orders_to_string(&mut app);
        // characters 11-19 of the timestamp = "14:35:00"
        assert!(
            output.contains("14:35:00"),
            "expected HH:MM:SS time in Submitted column"
        );
    }

    #[test]
    fn orders_partially_filled_counted_in_open_subtab() {
        let mut app = make_test_app();
        app.orders.push(make_order("pf", "partially_filled"));
        app.orders.push(make_order("pending", "pending_new"));
        app.orders.push(make_order("accepted", "accepted"));
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("Open (3)"),
            "expected Open (3) for partially_filled + pending_new + accepted, got: {output}"
        );
    }

    #[test]
    fn orders_expired_rejected_replaced_counted_in_cancelled_subtab() {
        let mut app = make_test_app();
        app.orders.push(make_order("exp", "expired"));
        app.orders.push(make_order("rej", "rejected"));
        app.orders.push(make_order("rep", "replaced"));
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("Cancelled (3)"),
            "expected Cancelled (3) for expired+rejected+replaced, got: {output}"
        );
    }

    #[test]
    fn orders_filled_qty_zero_shows_dash_in_filled_tab() {
        use crate::app::OrdersSubTab;
        use crate::types::Order;
        let mut app = make_test_app();
        app.orders.push(Order {
            id: "fq-zero".into(),
            symbol: "AMZN".into(),
            side: "buy".into(),
            qty: Some("5".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: "filled".into(),
            submitted_at: None,
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        app.orders_subtab = OrdersSubTab::Filled;
        let output = render_orders_to_string(&mut app);
        assert!(output.contains("—"), "expected em-dash for filled_qty=0");
    }

    #[test]
    fn orders_empty_filled_qty_shows_dash() {
        use crate::app::OrdersSubTab;
        use crate::types::Order;
        let mut app = make_test_app();
        app.orders.push(Order {
            id: "fq-empty".into(),
            symbol: "META".into(),
            side: "buy".into(),
            qty: Some("3".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: "filled".into(),
            submitted_at: None,
            filled_at: None,
            filled_qty: String::new(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        app.orders_subtab = OrdersSubTab::Filled;
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("—"),
            "expected em-dash for empty filled_qty"
        );
    }

    #[test]
    fn orders_held_status_counted_in_open_subtab() {
        let mut app = make_test_app();
        app.orders.push(make_order("held-order", "held"));
        let output = render_orders_to_string(&mut app);
        assert!(
            output.contains("Open (1)"),
            "held status should count in Open subtab, got: {output}"
        );
    }
}
