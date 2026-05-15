use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph},
    Frame,
};

use crate::app::App;
use crate::types::Position;
use crate::ui::{charts, theme};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(4)])
        .split(area);

    render_summary(frame, chunks[0], app);
    render_equity_chart(frame, chunks[1], app);
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

        // Day P&L
        let day_pl_str = match compute_day_pl(&acc.equity, &acc.last_equity) {
            Some((pl, pct)) => format_day_pl(pl, pct),
            None => "—".into(),
        };
        let day_pl_style = theme::pnl_style(&day_pl_str);

        // Open P&L
        let open_pl = compute_open_pl(&app.positions);
        let open_pl_str = format_pl_amount(open_pl);
        let open_pl_style = theme::pnl_style(&open_pl_str);

        // Account number (show dash if empty)
        let account_num = if acc.account_number.is_empty() {
            "—".into()
        } else {
            acc.account_number.clone()
        };

        let lines = vec![
            Line::from(vec![
                label("  Portfolio Value  "),
                value(&equity),
                spacer(),
                label("  Day P&L    "),
                Span::styled(day_pl_str, day_pl_style),
            ]),
            Line::from(vec![
                label("  Buying Power    "),
                value(&buying_power),
                spacer(),
                label("  Open P&L   "),
                Span::styled(open_pl_str, open_pl_style),
            ]),
            Line::from(vec![
                label("  Cash            "),
                value(&cash),
                spacer(),
                label("  Account #  "),
                value(&account_num),
            ]),
            Line::from(vec![
                label("  Long Mkt Value  "),
                value(&long_val),
                spacer(),
                label("  Status     "),
                value(&acc.status),
            ]),
        ];

        let para = Paragraph::new(lines);
        frame.render_widget(para, inner);
    } else {
        let para = Paragraph::new("  Loading account data…").style(Style::default().fg(theme::DIM));
        frame.render_widget(para, inner);
    }
}

fn render_equity_chart(frame: &mut Frame, area: Rect, app: &App) {
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

    let data_points = charts::price_points(&app.equity_history);
    let n = data_points.len() as f64;
    let [y_min, y_max] = charts::y_bounds(&data_points);
    let line_color = charts::trend_color(&data_points);

    let dataset = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(line_color))
        .data(&data_points);

    let chart = Chart::new(vec![dataset])
        .block(block)
        .x_axis(
            Axis::default()
                .bounds([0.0, (n - 1.0).max(0.0)])
                .labels(["09:30", "16:00"]),
        )
        .y_axis(Axis::default().bounds([y_min, y_max]));

    frame.render_widget(chart, area);
}

/// Compute Day P&L from equity and last_equity strings.
/// Returns `None` if either string is non-numeric or last_equity is zero.
pub fn compute_day_pl(equity: &str, last_equity: &str) -> Option<(f64, f64)> {
    let eq = equity.parse::<f64>().ok()?;
    let last = last_equity.parse::<f64>().ok()?;
    if last == 0.0 {
        return None;
    }
    let pl = eq - last;
    let pct = pl / last * 100.0;
    Some((pl, pct))
}

/// Compute total Open P&L as the sum of `unrealized_pl` across all positions.
pub fn compute_open_pl(positions: &[Position]) -> f64 {
    positions
        .iter()
        .filter_map(|p| p.unrealized_pl.parse::<f64>().ok())
        .sum()
}

/// Format a P&L dollar amount with sign prefix: `"+$843.22"` or `"-$843.22"`.
pub fn format_pl_amount(pl: f64) -> String {
    if pl >= 0.0 {
        format!("+${:.2}", pl)
    } else {
        format!("-${:.2}", pl.abs())
    }
}

/// Format Day P&L with both dollar and percentage: `"+$843.22 (+0.68%)"`.
fn format_day_pl(pl: f64, pct: f64) -> String {
    let sign = if pl >= 0.0 { "+" } else { "-" };
    format!("{}${:.2} ({}{:.2}%)", sign, pl.abs(), sign, pct.abs())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Position;

    fn make_position(unrealized_pl: &str) -> Position {
        Position {
            symbol: "AAPL".into(),
            qty: "10".into(),
            avg_entry_price: "100.00".into(),
            current_price: "110.00".into(),
            market_value: "1100.00".into(),
            unrealized_pl: unrealized_pl.into(),
            unrealized_plpc: "0.1".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }
    }

    // ── format_pl_amount ──────────────────────────────────────────────────────

    #[test]
    fn format_pl_positive() {
        assert_eq!(format_pl_amount(843.22), "+$843.22");
    }

    #[test]
    fn format_pl_negative() {
        assert_eq!(format_pl_amount(-123.45), "-$123.45");
    }

    #[test]
    fn format_pl_zero() {
        assert_eq!(format_pl_amount(0.0), "+$0.00");
    }

    // ── format_day_pl ─────────────────────────────────────────────────────────

    #[test]
    fn format_day_pl_positive() {
        assert_eq!(format_day_pl(843.22, 0.68), "+$843.22 (+0.68%)");
    }

    #[test]
    fn format_day_pl_negative() {
        assert_eq!(format_day_pl(-500.0, -1.0), "-$500.00 (-1.00%)");
    }

    // ── format_dollars ────────────────────────────────────────────────────────

    #[test]
    fn format_dollars_valid_float() {
        assert_eq!(format_dollars("1000.5"), "1000.50");
        assert_eq!(format_dollars("0"), "0.00");
        assert_eq!(format_dollars("125432.18"), "125432.18");
    }

    #[test]
    fn format_dollars_non_numeric_passthrough() {
        assert_eq!(format_dollars("N/A"), "N/A");
        assert_eq!(format_dollars(""), "");
    }

    // ── span helpers ──────────────────────────────────────────────────────────

    #[test]
    fn label_span_contains_text() {
        let span = label("  Portfolio Value  ");
        assert_eq!(span.content, "  Portfolio Value  ");
    }

    #[test]
    fn value_span_contains_text() {
        let span = value("$1,000.00");
        assert_eq!(span.content, "$1,000.00");
    }

    #[test]
    fn spacer_span_is_three_spaces() {
        let span = spacer();
        assert_eq!(span.content, "   ");
    }

    // ── compute_day_pl ────────────────────────────────────────────────────────

    #[test]
    fn compute_day_pl_positive() {
        let (pl, pct) = compute_day_pl("125432.18", "124588.96").unwrap();
        assert!((pl - 843.22).abs() < 0.01, "pl={pl}");
        assert!((pct - 0.6767).abs() < 0.01, "pct={pct}");
    }

    #[test]
    fn compute_day_pl_negative() {
        let (pl, pct) = compute_day_pl("99000.00", "100000.00").unwrap();
        assert!((pl - (-1000.0)).abs() < 0.01);
        assert!((pct - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn compute_day_pl_zero_last_equity_returns_none() {
        assert!(compute_day_pl("100000.00", "0").is_none());
    }

    #[test]
    fn compute_day_pl_unparseable_returns_none() {
        assert!(compute_day_pl("N/A", "100000").is_none());
        assert!(compute_day_pl("100000", "N/A").is_none());
    }

    // ── compute_open_pl ───────────────────────────────────────────────────────

    #[test]
    fn compute_open_pl_sums_all_positions() {
        let positions = vec![
            make_position("500.00"),
            make_position("704.50"),
            make_position("-200.00"),
        ];
        let result = compute_open_pl(&positions);
        assert!((result - 1004.50).abs() < 0.01, "result={result}");
    }

    #[test]
    fn compute_open_pl_empty_is_zero() {
        assert_eq!(compute_open_pl(&[]), 0.0);
    }

    #[test]
    fn compute_open_pl_skips_unparseable() {
        let positions = vec![make_position("100.00"), make_position("N/A")];
        let result = compute_open_pl(&positions);
        assert!((result - 100.0).abs() < 0.01);
    }

    // ── render_equity_chart ───────────────────────────────────────────────────

    fn render_equity_chart_to_string(equity_history: Vec<u64>) -> String {
        use ratatui::{backend::TestBackend, Terminal};
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = equity_history;
        terminal
            .draw(|frame| {
                render_equity_chart(frame, frame.area(), &app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let width = buffer.area().width as usize;
        let height = buffer.area().height as usize;
        (0..height)
            .map(|row| {
                (0..width)
                    .map(|col| {
                        buffer
                            .cell(ratatui::layout::Position {
                                x: col as u16,
                                y: row as u16,
                            })
                            .map(|c| c.symbol().to_string())
                            .unwrap_or_default()
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn render_equity_chart_empty_shows_collecting() {
        let output = render_equity_chart_to_string(vec![]);
        assert!(
            output.contains("Collecting data"),
            "should show collecting message when history is empty"
        );
    }

    #[test]
    fn render_equity_chart_with_data_contains_braille_chars() {
        // Simulate 20 data points with visible variation ($125,000 → $125,200)
        let history: Vec<u64> = (0..20).map(|i| 12_500_000u64 + i * 1_000).collect();
        let output = render_equity_chart_to_string(history);
        let has_braille = output
            .chars()
            .any(|c| ('\u{2800}'..='\u{28FF}').contains(&c));
        assert!(
            has_braille,
            "expected braille characters in line chart output, got:\n{}",
            output
        );
    }

    #[test]
    fn render_equity_chart_shows_time_labels() {
        let history: Vec<u64> = (0..10).map(|i| 10_000_000u64 + i * 500).collect();
        let output = render_equity_chart_to_string(history);
        assert!(
            output.contains("09:30") && output.contains("16:00"),
            "should show time labels on x-axis, got:\n{}",
            output
        );
    }
}
