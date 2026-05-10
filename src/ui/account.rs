use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Sparkline},
    Frame,
};

use crate::app::App;
use crate::ui::theme;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(4)])
        .split(area);

    render_summary(frame, chunks[0], app);
    render_sparkline(frame, chunks[1], app);
}

fn render_summary(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Account ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER_COLOR));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(acc) = &app.account {
        let equity = format!("${}", format_dollars(&acc.equity));
        let buying_power = format!("${}", format_dollars(&acc.buying_power));
        let cash = format!("${}", format_dollars(&acc.cash));
        let long_val = format!("${}", format_dollars(&acc.long_market_value));

        let lines = vec![
            Line::from(vec![
                label("  Portfolio Value  "),
                value(&equity),
                spacer(),
                label("  Account Status  "),
                value(&acc.status),
            ]),
            Line::from(vec![
                label("  Buying Power    "),
                value(&buying_power),
                spacer(),
                label("  Currency        "),
                value(&acc.currency),
            ]),
            Line::from(vec![
                label("  Cash            "),
                value(&cash),
                spacer(),
                label("  Day Trades      "),
                value(&acc.daytrade_count.to_string()),
            ]),
            Line::from(vec![
                label("  Long Mkt Value  "),
                value(&long_val),
                spacer(),
                label("  PDT Flag        "),
                value(if acc.pattern_day_trader { "YES" } else { "NO" }),
            ]),
        ];

        let para = Paragraph::new(lines);
        frame.render_widget(para, inner);
    } else {
        let para = Paragraph::new("  Loading account data…").style(Style::default().fg(theme::DIM));
        frame.render_widget(para, inner);
    }
}

fn render_sparkline(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Today's Equity Curve ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER_COLOR));

    if app.equity_history.is_empty() {
        let para = Paragraph::new("  Collecting data…")
            .style(Style::default().fg(theme::DIM))
            .block(block);
        frame.render_widget(para, area);
        return;
    }

    let sparkline = Sparkline::default()
        .block(block)
        .data(&app.equity_history)
        .style(Style::default().fg(theme::BRAND_CYAN));

    frame.render_widget(sparkline, area);
}

fn label(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::default().fg(theme::DIM))
}

fn value(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), theme::style_bold())
}

fn spacer() -> Span<'static> {
    Span::raw("   ")
}

fn format_dollars(s: &str) -> String {
    if let Ok(v) = s.parse::<f64>() {
        format!("{:.2}", v)
    } else {
        s.to_string()
    }
}
