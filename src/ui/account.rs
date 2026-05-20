use ratatui::{
    layout::{Constraint, Direction, Layout, Position as TermPos, Rect},
    style::Style,
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Clear, Dataset, GraphType, Paragraph},
    Frame,
};

use crate::app::App;
use crate::types::Position;
use crate::ui::charts;
use crate::ui::formatting::format_dollar;
use crate::ui::theme::ThemeColors;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(4)])
        .split(area);

    app.hit_areas.equity_chart_area = chunks[1];
    render_summary(frame, chunks[0], app);
    render_equity_chart(frame, chunks[1], app);
}

fn render_summary(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.current_theme.colors();
    let block = c.bordered_block(" Account ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(acc) = &app.account {
        let buying_power = format!("${}", format_dollar(&acc.buying_power));
        let cash = format!("${}", format_dollar(&acc.cash));
        let long_val = format!("${}", format_dollar(&acc.long_market_value));

        // Day P&L
        let day_pl_str = match compute_day_pl(&acc.equity, &acc.last_equity) {
            Some((pl, pct)) => format_day_pl(pl, pct),
            None => "—".into(),
        };
        let day_pl_style = c.pnl_style(&day_pl_str);

        // Open P&L
        let open_pl = compute_open_pl(&app.positions);
        let open_pl_str = format_pl_amount(open_pl);
        let open_pl_style = c.pnl_style(&open_pl_str);

        // Account number (show dash if empty)
        let account_num = if acc.account_number.is_empty() {
            "—".into()
        } else {
            acc.account_number.clone()
        };

        let lines = vec![
            Line::from(vec![
                label_t("  Portfolio Value  ", &c),
                value(acc.equity.as_str()),
                spacer(),
                label_t("  Day P&L    ", &c),
                Span::styled(day_pl_str, day_pl_style),
            ]),
            Line::from(vec![
                label_t("  Buying Power    ", &c),
                value(&buying_power),
                spacer(),
                label_t("  Open P&L   ", &c),
                Span::styled(open_pl_str, open_pl_style),
            ]),
            Line::from(vec![
                label_t("  Cash            ", &c),
                value(&cash),
                spacer(),
                label_t("  Account #  ", &c),
                value(&account_num),
            ]),
            Line::from(vec![
                label_t("  Long Mkt Value  ", &c),
                value(&long_val),
                spacer(),
                label_t("  Status     ", &c),
                value(&acc.status),
            ]),
        ];

        let para = Paragraph::new(lines);
        frame.render_widget(para, inner);
    } else {
        let para = Paragraph::new("  Loading account data…").style(c.dim_style());
        frame.render_widget(para, inner);
    }
}

fn render_equity_chart(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.current_theme.colors();
    let block = c.bordered_block(" Today's Equity Curve  ←/→ to inspect ");

    if app.equity_history.is_empty() {
        let para = Paragraph::new("  Collecting data…")
            .style(c.dim_style())
            .block(block);
        frame.render_widget(para, area);
        return;
    }

    let data_points = charts::price_points(&app.equity_history);
    let n = data_points.len() as f64;
    let [y_min, y_max] = charts::y_bounds(&data_points);
    let line_color = charts::trend_color(&data_points, &c);

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

    // Draw crosshair + floating tooltip if a cursor index is set.
    if let Some(cursor_idx) = app.equity_chart_cursor {
        let cursor_idx = cursor_idx.min(app.equity_history.len().saturating_sub(1));
        render_chart_crosshair(
            frame,
            area,
            &data_points,
            cursor_idx,
            &c,
            y_min,
            y_max,
            &app.equity_history,
        );
    }
}

/// Draw a vertical crosshair line and a floating price/time tooltip at `cursor_idx`.
fn render_chart_crosshair(
    frame: &mut Frame,
    area: Rect,
    data_points: &[(f64, f64)],
    cursor_idx: usize,
    c: &crate::ui::theme::ThemeColors,
    y_min: f64,
    y_max: f64,
    equity_history: &[u64],
) {
    // The Chart widget uses a 1-cell border + a y-axis label column on the left
    // and an x-axis label row at the bottom.  Approximate inner plot area:
    //   left:   area.x + 1 (border) + ~8 chars for y-axis labels
    //   right:  area.x + area.width - 2 (border)
    //   top:    area.y + 1 (border)
    //   bottom: area.y + area.height - 2 (border + x-axis label row)
    let plot_x = area.x + 9;
    let plot_w = area.width.saturating_sub(11); // 9 left + 2 right border
    let plot_y = area.y + 1;
    let plot_h = area.height.saturating_sub(3); // 1 top + 2 bottom (border + axis row)

    if plot_w == 0 || plot_h == 0 || data_points.is_empty() {
        return;
    }

    let n = data_points.len();
    // Map cursor index → terminal column within the plot area.
    let col = if n <= 1 {
        plot_x
    } else {
        plot_x + ((cursor_idx as f64 / (n - 1) as f64) * (plot_w - 1) as f64).round() as u16
    };
    let col = col.min(plot_x + plot_w - 1);

    // Draw a vertical crosshair line.
    let crosshair_style = Style::default().fg(c.accent);
    for row in plot_y..plot_y + plot_h {
        if let Some(cell) = frame.buffer_mut().cell_mut(TermPos { x: col, y: row }) {
            cell.set_symbol("│").set_style(crosshair_style);
        }
    }

    // Build tooltip text: "$12,345.67  14:37"
    let price_dollars = equity_history[cursor_idx] as f64 / 100.0;
    let time_str = index_to_time(cursor_idx, n);
    let label = format!(" ${:.2}  {} ", price_dollars, time_str);

    // Also draw the cursor dot on the crosshair at the price's y position.
    let price_row = if (y_max - y_min).abs() > f64::EPSILON {
        let frac = (data_points[cursor_idx].1 - y_min) / (y_max - y_min);
        // y grows downward in the terminal; high price = low row index
        let row_offset = ((1.0 - frac) * (plot_h.saturating_sub(1)) as f64).round() as u16;
        plot_y + row_offset.min(plot_h.saturating_sub(1))
    } else {
        plot_y + plot_h / 2
    };

    if let Some(cell) = frame.buffer_mut().cell_mut(TermPos {
        x: col,
        y: price_row,
    }) {
        cell.set_symbol("●")
            .set_style(Style::default().fg(c.accent));
    }

    // Render floating tooltip popup.
    let popup_w = label.len() as u16;
    let popup_h = 3u16;

    // Position above the price point; fall back to below if not enough room.
    let popup_y = if price_row >= area.y + popup_h {
        price_row - popup_h
    } else {
        price_row + 1
    };
    // Horizontally centred on crosshair column, clamped to screen.
    let popup_x = col
        .saturating_sub(popup_w / 2)
        .min(area.x + area.width.saturating_sub(popup_w));
    let popup_x = popup_x.max(area.x);

    let popup_rect = Rect::new(popup_x, popup_y, popup_w, popup_h);

    frame.render_widget(Clear, popup_rect);
    frame.render_widget(
        Paragraph::new(label)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(c.border_fg_style()),
            )
            .style(Style::default().fg(c.accent)),
        popup_rect,
    );
}

/// Convert a data-point index into an approximate market-hours time string (`HH:MM`).
///
/// Market session runs 09:30–16:00 (390 minutes). With `n` samples, each step
/// is `390 / (n - 1)` minutes from open.
fn index_to_time(idx: usize, n: usize) -> String {
    if n <= 1 {
        return "09:30".to_string();
    }
    let minutes_from_open = (idx as f64 / (n - 1) as f64 * 390.0).round() as u32;
    let total = 9 * 60 + 30 + minutes_from_open;
    format!("{:02}:{:02}", total / 60, total % 60)
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

fn label_t(s: &str, c: &ThemeColors) -> Span<'static> {
    Span::styled(s.to_string(), c.dim_style())
}

fn value(s: &str) -> Span<'static> {
    Span::styled(
        s.to_string(),
        ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD),
    )
}

fn spacer() -> Span<'static> {
    Span::raw("   ")
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

    // ── format_dollar (from crate::ui::formatting) ────────────────────────────

    #[test]
    fn format_dollars_valid_float() {
        assert_eq!(crate::ui::formatting::format_dollar("1000.5"), "1000.50");
        assert_eq!(crate::ui::formatting::format_dollar("0"), "0.00");
        assert_eq!(
            crate::ui::formatting::format_dollar("125432.18"),
            "125432.18"
        );
    }

    #[test]
    fn format_dollars_non_numeric_passthrough() {
        assert_eq!(crate::ui::formatting::format_dollar("N/A"), "N/A");
        assert_eq!(crate::ui::formatting::format_dollar(""), "");
    }

    // ── span helpers ──────────────────────────────────────────────────────────

    #[test]
    fn label_span_contains_text() {
        let c = crate::ui::theme::Theme::Default.colors();
        let span = label_t("  Portfolio Value  ", &c);
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
    fn render_equity_chart_single_data_point_does_not_panic() {
        // n=1 → x-axis bound = (1-1).max(0) = 0; must not panic or produce garbage
        let output = render_equity_chart_to_string(vec![12_500_000u64]);
        // With one point there is nothing to draw as a line, but the chart
        // frame and axes should still render without crashing.
        assert!(!output.is_empty());
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

    // ── index_to_time ─────────────────────────────────────────────────────────

    #[test]
    fn index_to_time_first_point_is_open() {
        assert_eq!(index_to_time(0, 10), "09:30");
    }

    #[test]
    fn index_to_time_last_point_is_close() {
        // 390 minutes after 09:30 = 16:00
        assert_eq!(index_to_time(9, 10), "16:00");
    }

    #[test]
    fn index_to_time_midpoint() {
        // Midpoint (idx 4 of 9 steps) ≈ 195 min after 09:30 → 12:45
        let t = index_to_time(4, 9);
        assert!(!t.is_empty());
        // Just confirm it parses as HH:MM
        let parts: Vec<&str> = t.split(':').collect();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn index_to_time_single_point() {
        assert_eq!(index_to_time(0, 1), "09:30");
    }

    // ── cursor key handling ───────────────────────────────────────────────────

    fn key(code: crossterm::event::KeyCode) -> crossterm::event::KeyEvent {
        crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
    }

    #[test]
    fn account_right_key_sets_cursor() {
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = vec![100_000; 10];
        app.active_tab = crate::app::Tab::Account;
        crate::update::update(
            &mut app,
            crate::events::Event::Input(key(crossterm::event::KeyCode::Right)),
        );
        assert_eq!(app.equity_chart_cursor, Some(1));
    }

    #[test]
    fn account_left_key_starts_from_end() {
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = vec![100_000; 10];
        app.active_tab = crate::app::Tab::Account;
        // With no cursor, left starts from n-1=9 then subtracts 1 → 8
        crate::update::update(
            &mut app,
            crate::events::Event::Input(key(crossterm::event::KeyCode::Left)),
        );
        assert_eq!(app.equity_chart_cursor, Some(8));
    }

    #[test]
    fn account_left_key_clamps_at_zero() {
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = vec![100_000; 10];
        app.equity_chart_cursor = Some(0);
        app.active_tab = crate::app::Tab::Account;
        crate::update::update(
            &mut app,
            crate::events::Event::Input(key(crossterm::event::KeyCode::Left)),
        );
        assert_eq!(app.equity_chart_cursor, Some(0));
    }

    #[test]
    fn account_right_key_clamps_at_end() {
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = vec![100_000; 3];
        app.equity_chart_cursor = Some(2);
        app.active_tab = crate::app::Tab::Account;
        crate::update::update(
            &mut app,
            crate::events::Event::Input(key(crossterm::event::KeyCode::Right)),
        );
        assert_eq!(app.equity_chart_cursor, Some(2));
    }

    #[test]
    fn account_esc_clears_cursor() {
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = vec![100_000; 5];
        app.equity_chart_cursor = Some(3);
        app.active_tab = crate::app::Tab::Account;
        crate::update::update(
            &mut app,
            crate::events::Event::Input(key(crossterm::event::KeyCode::Esc)),
        );
        assert!(app.equity_chart_cursor.is_none());
    }

    #[test]
    fn switching_tab_clears_cursor() {
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = vec![100_000; 5];
        app.equity_chart_cursor = Some(2);
        app.active_tab = crate::app::Tab::Account;
        crate::update::update(
            &mut app,
            crate::events::Event::Input(key(crossterm::event::KeyCode::Char('2'))),
        );
        assert!(app.equity_chart_cursor.is_none());
    }

    #[test]
    fn render_equity_chart_with_cursor_does_not_panic() {
        use ratatui::{backend::TestBackend, Terminal};
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = (0..20).map(|i| 12_500_000u64 + i * 1_000).collect();
        app.equity_chart_cursor = Some(10);
        terminal
            .draw(|frame| {
                render_equity_chart(frame, frame.area(), &app);
            })
            .unwrap();
        // If we get here the render did not panic
    }

    #[test]
    fn render_equity_chart_hint_in_title() {
        let history: Vec<u64> = (0..5).map(|i| 10_000_000u64 + i * 500).collect();
        let output = render_equity_chart_to_string(history);
        assert!(
            output.contains("←") || output.contains("→"),
            "title should contain navigation hint arrows, got:\n{}",
            output
        );
    }

    // ── mouse click → cursor ──────────────────────────────────────────────────

    fn mouse_click(col: u16, row: u16) -> crossterm::event::MouseEvent {
        crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    #[test]
    fn mouse_click_inside_chart_sets_cursor() {
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = vec![100_000; 10];
        app.active_tab = crate::app::Tab::Account;
        // Set the chart area hit rect to a 60-wide, 15-tall area at (0,0)
        // plot_x=9, plot_w=49; clicking col=9 (first plot col) → idx 0
        app.hit_areas.equity_chart_area = ratatui::layout::Rect::new(0, 0, 60, 15);
        crate::update::update(&mut app, crate::events::Event::Mouse(mouse_click(9, 5)));
        assert_eq!(app.equity_chart_cursor, Some(0));
    }

    #[test]
    fn mouse_click_last_column_sets_last_cursor() {
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = vec![100_000; 10];
        app.active_tab = crate::app::Tab::Account;
        // chart area 60-wide → plot_x=9, plot_w=49, last plot col = 9+49-1 = 57
        app.hit_areas.equity_chart_area = ratatui::layout::Rect::new(0, 0, 60, 15);
        crate::update::update(&mut app, crate::events::Event::Mouse(mouse_click(57, 5)));
        assert_eq!(app.equity_chart_cursor, Some(9));
    }

    #[test]
    fn mouse_click_outside_chart_area_does_not_change_cursor() {
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = vec![100_000; 10];
        app.equity_chart_cursor = Some(3);
        app.active_tab = crate::app::Tab::Account;
        app.hit_areas.equity_chart_area = ratatui::layout::Rect::new(0, 5, 60, 15);
        // Click is above the chart area (row 0 < area.y 5)
        crate::update::update(&mut app, crate::events::Event::Mouse(mouse_click(20, 0)));
        assert_eq!(app.equity_chart_cursor, Some(3));
    }

    #[test]
    fn mouse_click_non_account_tab_does_not_set_cursor() {
        let mut app = crate::app::test_helpers::make_test_app();
        app.equity_history = vec![100_000; 10];
        app.active_tab = crate::app::Tab::Watchlist;
        app.hit_areas.equity_chart_area = ratatui::layout::Rect::new(0, 0, 60, 15);
        crate::update::update(&mut app, crate::events::Event::Mouse(mouse_click(20, 5)));
        assert!(app.equity_chart_cursor.is_none());
    }
}
