use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Sparkline},
    Frame,
};

use crate::app::App;
use crate::types::Position;
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
}
