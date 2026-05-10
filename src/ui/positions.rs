use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::App;
use crate::ui::theme;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.positions.is_empty() {
        let para = Paragraph::new("  No open positions.")
            .style(Style::default().fg(theme::DIM))
            .block(
                Block::default()
                    .title(" Positions ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::BORDER_COLOR)),
            );
        frame.render_widget(para, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let header = Row::new(vec![
        Cell::from("Symbol").style(theme::style_header()),
        Cell::from("Qty").style(theme::style_header()),
        Cell::from("Avg Cost").style(theme::style_header()),
        Cell::from("Cur Price").style(theme::style_header()),
        Cell::from("Mkt Value").style(theme::style_header()),
        Cell::from("Unrealized P&L").style(theme::style_header()),
        Cell::from("%").style(theme::style_header()),
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
            let pnl_style = theme::pnl_style(&pnl);

            Row::new(vec![
                Cell::from(p.symbol.clone()).style(theme::style_bold()),
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
        .border_style(Style::default().fg(theme::BORDER_COLOR));

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
    .row_highlight_style(theme::style_selected())
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
        theme::style_positive()
    } else {
        theme::style_negative()
    };

    let footer = Line::from(vec![
        Span::styled("  Total Long: ", Style::default().fg(theme::DIM)),
        Span::styled(format!("${:.2}", total_value), theme::style_bold()),
        Span::styled("    Total Unrealized: ", Style::default().fg(theme::DIM)),
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
