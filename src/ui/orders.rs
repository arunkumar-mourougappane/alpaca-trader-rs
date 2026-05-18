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

    let header = Row::new(vec![
        Cell::from("ID").style(c.header_style()),
        Cell::from("Symbol").style(c.header_style()),
        Cell::from("Side").style(c.header_style()),
        Cell::from("Qty").style(c.header_style()),
        Cell::from("Type").style(c.header_style()),
        Cell::from("Limit").style(c.header_style()),
        Cell::from("Status").style(c.header_style()),
        Cell::from("Submitted").style(c.header_style()),
    ]);

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

            Row::new(vec![
                Cell::from(short_id).style(c.dim_style()),
                Cell::from(o.symbol.clone()).style(c.bold_style()),
                Cell::from(o.side.to_uppercase()).style(side_style),
                Cell::from(qty_str),
                Cell::from(o.order_type.to_uppercase()),
                Cell::from(limit_str),
                Cell::from(o.status.clone()),
                Cell::from(submitted).style(c.dim_style()),
            ])
        })
        .collect();

    let block = Block::default()
        .title(" Orders ")
        .borders(Borders::ALL)
        .border_style(c.border_fg_style());

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Min(12),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(c.selected_style())
    .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, area, &mut app.orders_state);
}
