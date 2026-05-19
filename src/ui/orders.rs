use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::app::{App, OrdersSubTab};

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
            .block(
                Block::default()
                    .title(" Orders ")
                    .borders(Borders::ALL)
                    .border_style(c.border_fg_style()),
            );
        frame.render_widget(para, area);
        return;
    }

    let mut header_cells = vec![
        Cell::from("ID").style(c.header_style()),
        Cell::from("Symbol").style(c.header_style()),
        Cell::from("Side").style(c.header_style()),
        Cell::from("Qty").style(c.header_style()),
        Cell::from("Type").style(c.header_style()),
        Cell::from("Limit").style(c.header_style()),
        Cell::from("Status").style(c.header_style()),
        Cell::from("Submitted").style(c.header_style()),
    ];
    if is_filled_tab {
        header_cells.push(Cell::from("Filled Qty").style(c.header_style()));
        header_cells.push(Cell::from("Fill Price").style(c.header_style()));
    }
    let header = Row::new(header_cells);

    let rows: Vec<Row> = orders
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
                .map(|p: &str| format!("${:.2}", p.parse::<f64>().unwrap_or(0.0)))
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
                    .map(|p| format!("${:.2}", p.parse::<f64>().unwrap_or(0.0)))
                    .unwrap_or_else(|| "—".into());
                cells.push(Cell::from(filled_qty_str));
                cells.push(Cell::from(fill_price_str).style(c.positive_style()));
            }

            Row::new(cells)
        })
        .collect();

    let block = Block::default()
        .title(" Orders ")
        .borders(Borders::ALL)
        .border_style(c.border_fg_style());

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
    use ratatui::{backend::TestBackend, Terminal};

    use crate::app::test_helpers::{make_order, make_test_app};

    fn render_orders_to_string(app: &mut crate::app::App) -> String {
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                super::render(frame, frame.area(), app);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut out = String::new();
        for row in 0..buf.area.height {
            for col in 0..buf.area.width {
                out.push_str(buf[(col, row)].symbol());
            }
            out.push('\n');
        }
        out
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
}
