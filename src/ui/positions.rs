use ratatui::{
    layout::Constraint,
    style::Style,
    widgets::{Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::App;
use crate::ui::formatting::{format_pct_ratio, format_price, header_cell};

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    let c = app.current_theme.colors();

    if app.positions.is_empty() {
        let para = Paragraph::new("  No open positions.")
            .style(c.dim_style())
            .block(c.bordered_block(" Positions "));
        frame.render_widget(para, area);
        return;
    }

    let header = Row::new(vec![
        header_cell("Symbol", &c),
        header_cell("Qty", &c),
        header_cell("Avg Cost", &c),
        header_cell("Cur Price", &c),
        header_cell("Mkt Value", &c),
        header_cell("Unrealized P&L", &c),
        header_cell("%", &c),
    ]);

    let mut rows: Vec<Row> = app
        .positions
        .iter()
        .map(|p| {
            let cur_price = app
                .quotes
                .get(&p.symbol)
                .and_then(|q| q.ap.or(q.bp))
                .map(|v| format!("${:.2}", v))
                .unwrap_or_else(|| format_price(&p.current_price));

            let pnl = p.unrealized_pl.trim().to_string();
            let pnl_pct = format_pct_ratio(&p.unrealized_plpc);
            let pnl_style = c.pnl_style(&pnl);

            Row::new(vec![
                Cell::from(p.symbol.clone()).style(c.bold_style()),
                Cell::from(p.qty.clone()),
                Cell::from(format_price(&p.avg_entry_price)),
                Cell::from(cur_price),
                Cell::from(format_price(&p.market_value)),
                Cell::from(format_price(&pnl)).style(pnl_style),
                Cell::from(pnl_pct).style(pnl_style),
            ])
        })
        .collect();

    // ── Totals footer row ─────────────────────────────────────────────────
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
    // cost basis = market_value - unrealized_pl; avoid division by zero
    let total_cost = total_value - total_pnl;
    let total_pct = if total_cost != 0.0 {
        total_pnl / total_cost * 100.0
    } else {
        0.0
    };
    let footer_pnl_style = if total_pnl >= 0.0 {
        c.positive_style()
    } else {
        c.negative_style()
    };

    rows.push(
        Row::new(vec![
            Cell::from("TOTAL").style(c.bold_style()),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(format!("${:.2}", total_value)).style(c.bold_style()),
            Cell::from({
                let sign = if total_pnl >= 0.0 { "+" } else { "-" };
                format!("{}${:.2}", sign, total_pnl.abs())
            })
            .style(footer_pnl_style),
            Cell::from(format!("{:+.2}%", total_pct)).style(footer_pnl_style),
        ])
        .style(Style::default()),
    );

    let title = format!(" Positions ({}) ", app.positions.len());
    let block = c.bordered_block(&title);

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

    frame.render_stateful_widget(table, area, &mut app.positions_state);
}

#[cfg(test)]
mod tests {
    use crate::app::test_helpers::make_test_app;
    use crate::types::Position;
    use crate::ui::test_helpers::render_to_string;

    fn make_position(symbol: &str, pnl: &str) -> Position {
        Position {
            symbol: symbol.into(),
            qty: "10".into(),
            avg_entry_price: "100.00".into(),
            current_price: "110.00".into(),
            market_value: "1100.00".into(),
            unrealized_pl: pnl.into(),
            unrealized_plpc: "0.10".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }
    }

    fn render_positions_to_string(app: &mut crate::app::App) -> String {
        render_to_string(120, 20, |frame| {
            super::render(frame, frame.area(), app);
        })
    }

    #[test]
    fn positions_empty_shows_no_positions_message() {
        let mut app = make_test_app();
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("No open positions"),
            "expected no-positions message, got: {output}"
        );
    }

    #[test]
    fn positions_shows_header_columns() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "100.00"));
        let output = render_positions_to_string(&mut app);
        assert!(output.contains("Symbol"), "expected Symbol header");
        assert!(output.contains("Qty"), "expected Qty header");
        assert!(output.contains("Avg Cost"), "expected Avg Cost header");
    }

    #[test]
    fn positions_shows_symbol_and_qty() {
        let mut app = make_test_app();
        app.positions.push(make_position("TSLA", "250.00"));
        let output = render_positions_to_string(&mut app);
        assert!(output.contains("TSLA"), "expected TSLA symbol in row");
        assert!(output.contains("10"), "expected qty in row");
    }

    #[test]
    fn positions_shows_footer_total_row() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "100.00"));
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("TOTAL"),
            "expected TOTAL footer row in table, got: {output}"
        );
    }

    #[test]
    fn positions_footer_total_market_value() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "100.00")); // market_value = 1100.00
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("1100.00"),
            "expected total market value in footer row, got: {output}"
        );
    }

    #[test]
    fn positions_footer_total_pnl_sum() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "100.00"));
        app.positions.push(make_position("TSLA", "-30.00"));
        let output = render_positions_to_string(&mut app);
        // total PnL = +70.00 → +$70.00
        assert!(
            output.contains("+$70.00"),
            "expected summed PnL in footer row, got: {output}"
        );
    }

    #[test]
    fn positions_footer_negative_total_pnl() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "-50.00"));
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("-$50.00"),
            "expected negative total PnL in footer, got: {output}"
        );
    }

    #[test]
    fn positions_footer_pct_calculated() {
        let mut app = make_test_app();
        // market_value=1100, unrealized_pl=100 → cost=1000 → pct=+10.00%
        app.positions.push(make_position("AAPL", "100.00"));
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("+10.00%"),
            "expected +10.00% in footer row, got: {output}"
        );
    }

    #[test]
    fn positions_negative_pnl_renders() {
        let mut app = make_test_app();
        app.positions.push(make_position("NVDA", "-50.00"));
        let output = render_positions_to_string(&mut app);
        assert!(output.contains("NVDA"), "expected NVDA symbol");
    }

    #[test]
    fn positions_multiple_rows() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "100.00"));
        app.positions.push(make_position("MSFT", "-30.00"));
        let output = render_positions_to_string(&mut app);
        assert!(output.contains("AAPL"), "expected AAPL");
        assert!(output.contains("MSFT"), "expected MSFT");
    }

    #[test]
    fn positions_count_in_title() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "50.00"));
        app.positions.push(make_position("GOOG", "75.00"));
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("Positions (2)"),
            "expected 'Positions (2)' in title, got: {output}"
        );
    }

    #[test]
    fn positions_render_uses_theme_colors() {
        use crate::ui::theme::Theme;
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "100.00"));
        app.current_theme = Theme::HighContrast;
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("AAPL"),
            "should render with high-contrast theme"
        );
    }

    #[test]
    fn positions_footer_zero_cost_basis_pct_is_zero() {
        // When total_cost (market_value - unrealized_pl) == 0, pct should display 0.00%
        let mut app = make_test_app();
        // market_value == unrealized_pl → cost basis = 0, avoid division by zero
        app.positions.push(Position {
            symbol: "ZERO".into(),
            qty: "1".into(),
            avg_entry_price: "0.00".into(),
            current_price: "100.00".into(),
            market_value: "100.00".into(),
            unrealized_pl: "100.00".into(),
            unrealized_plpc: "0.0".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        });
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("+0.00%"),
            "expected +0.00% when cost basis is zero, got: {output}"
        );
    }

    #[test]
    fn positions_fmt_dollar_invalid_passthrough() {
        assert_eq!(
            crate::ui::formatting::format_dollar("not-a-number"),
            "not-a-number"
        );
    }

    #[test]
    fn positions_fmt_dollar_valid() {
        assert_eq!(crate::ui::formatting::format_dollar("123.456"), "123.46");
    }

    #[test]
    fn positions_fmt_pct_valid() {
        assert_eq!(crate::ui::formatting::format_pct_ratio("0.05"), "+5.00%");
    }

    #[test]
    fn positions_fmt_pct_negative() {
        assert_eq!(crate::ui::formatting::format_pct_ratio("-0.025"), "-2.50%");
    }

    #[test]
    fn positions_fmt_pct_invalid() {
        assert_eq!(crate::ui::formatting::format_pct_ratio("n/a"), "n/a");
    }
}
