use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::app::{App, OrdersSubTab};
use crate::ui::theme;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    app.hit_areas.orders_subtab_bar = Some(chunks[0]);
    render_subtabs(frame, chunks[0], app);
    render_table(frame, chunks[1], app);
}

fn render_subtabs(frame: &mut Frame, area: Rect, app: &App) {
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

    let selected = match app.orders_subtab {
        OrdersSubTab::Open => 0,
        OrdersSubTab::Filled => 1,
        OrdersSubTab::Cancelled => 2,
    };

    let tabs = Tabs::new(titles)
        .select(selected)
        .highlight_style(
            Style::default()
                .fg(theme::BRAND_CYAN)
                .add_modifier(Modifier::BOLD),
        )
        .divider("|");

    frame.render_widget(tabs, area);
}

fn render_table(frame: &mut Frame, area: Rect, app: &mut App) {
    let orders = app.filtered_orders();

    if orders.is_empty() {
        let para = Paragraph::new("  No orders in this category.")
            .style(Style::default().fg(theme::DIM))
            .block(
                Block::default()
                    .title(" Orders ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::BORDER_COLOR)),
            );
        frame.render_widget(para, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("ID").style(theme::style_header()),
        Cell::from("Symbol").style(theme::style_header()),
        Cell::from("Side").style(theme::style_header()),
        Cell::from("Qty").style(theme::style_header()),
        Cell::from("Type").style(theme::style_header()),
        Cell::from("Limit").style(theme::style_header()),
        Cell::from("Status").style(theme::style_header()),
        Cell::from("Submitted").style(theme::style_header()),
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
                Style::default().fg(theme::GREEN)
            } else {
                Style::default().fg(theme::RED)
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
                Cell::from(short_id).style(theme::style_dim()),
                Cell::from(o.symbol.clone()).style(theme::style_bold()),
                Cell::from(o.side.to_uppercase()).style(side_style),
                Cell::from(qty_str),
                Cell::from(o.order_type.to_uppercase()),
                Cell::from(limit_str),
                Cell::from(o.status.clone()),
                Cell::from(submitted).style(theme::style_dim()),
            ])
        })
        .collect();

    let block = Block::default()
        .title(" Orders ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER_COLOR));

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
    .row_highlight_style(theme::style_selected())
    .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, area, &mut app.orders_state);
}
