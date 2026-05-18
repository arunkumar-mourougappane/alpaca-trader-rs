use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let c = app.current_theme.colors();

    if app.positions.is_empty() {
        let para = Paragraph::new("  No open positions.")
            .style(c.dim_style())
            .block(
                Block::default()
                    .title(" Positions ")
                    .borders(Borders::ALL)
                    .border_style(c.border_fg_style()),
            );
        frame.render_widget(para, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let header = Row::new(vec![
        Cell::from("Symbol").style(c.header_style()),
        Cell::from("Qty").style(c.header_style()),
        Cell::from("Avg Cost").style(c.header_style()),
        Cell::from("Cur Price").style(c.header_style()),
        Cell::from("Mkt Value").style(c.header_style()),
        Cell::from("Unrealized P&L").style(c.header_style()),
        Cell::from("%").style(c.header_style()),
    ]);

    let rows: Vec<Row> = app
        .positions
        .iter()
        .map(|p| {
            let cur_price = app
                .quotes
                .get(&p.symbol)
                .and_then(|q| q.ap.or(q.bp))
                .map(|v| format!("${:.2}", v))
                .unwrap_or_else(|| format!("${}", fmt_dollar(&p.current_price)));

            let pnl = p.unrealized_pl.trim().to_string();
            let pnl_pct = fmt_pct(&p.unrealized_plpc);
            let pnl_style = c.pnl_style(&pnl);

            Row::new(vec![
                Cell::from(p.symbol.clone()).style(c.bold_style()),
                Cell::from(p.qty.clone()),
                Cell::from(format!("${}", fmt_dollar(&p.avg_entry_price))),
                Cell::from(cur_price),
                Cell::from(format!("${}", fmt_dollar(&p.market_value))),
                Cell::from(format!("${}", fmt_dollar(&pnl))).style(pnl_style),
                Cell::from(pnl_pct).style(pnl_style),
            ])
        })
        .collect();

    let block = Block::default()
        .title(format!(" Positions ({}) ", app.positions.len()))
        .borders(Borders::ALL)
        .border_style(c.border_fg_style());

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(13),
            Constraint::Length(16),
            Constraint::Length(9),
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(c.selected_style())
    .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, chunks[0], &mut app.positions_state);

    // Footer totals
    let total_value: f64 = app
        .positions
        .iter()
        .filter_map(|p| p.market_value.parse::<f64>().ok())
        .sum();
    let total_pnl: f64 = app
        .positions
        .iter()
        .filter_map(|p| p.unrealized_pl.parse::<f64>().ok())
        .sum();
    let pnl_style = if total_pnl >= 0.0 {
        c.positive_style()
    } else {
        c.negative_style()
    };

    let footer = Line::from(vec![
        Span::styled("  Total Long: ", c.dim_style()),
        Span::styled(format!("${:.2}", total_value), c.bold_style()),
        Span::styled("    Total Unrealized: ", c.dim_style()),
        Span::styled(format!("${:.2}", total_pnl), pnl_style),
    ]);

    let footer_para = Paragraph::new(footer).block(Block::default().borders(Borders::NONE));
    frame.render_widget(footer_para, chunks[1]);
}

fn fmt_dollar(s: &str) -> String {
    if let Ok(v) = s.parse::<f64>() {
        format!("{:.2}", v)
    } else {
        s.to_string()
    }
}

fn fmt_pct(s: &str) -> String {
    if let Ok(v) = s.parse::<f64>() {
        format!("{:+.2}%", v * 100.0)
    } else {
        s.to_string()
    }
}
