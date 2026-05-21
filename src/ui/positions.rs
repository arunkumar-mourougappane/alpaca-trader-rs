use ratatui::{
    layout::Constraint,
    style::Style,
    widgets::{Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::{App, PositionSortCol, SortDir};
use crate::ui::formatting::{format_pct_ratio, format_price, header_cell};

/// Return a header label with an ▲/▼ sort indicator appended when `active` is true.
fn sorted_header(label: &str, active: bool, dir: SortDir) -> String {
    if active {
        let arrow = if dir == SortDir::Asc { " ▲" } else { " ▼" };
        format!("{label}{arrow}")
    } else {
        label.to_string()
    }
}

pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    let c = app.current_theme.colors();

    if app.positions.is_empty() {
        let para = Paragraph::new("  No open positions.")
            .style(c.dim_style())
            .block(c.bordered_block(" Positions "));
        frame.render_widget(para, area);
        return;
    }

    let sort_col = app.positions_sort.col;
    let sort_dir = app.positions_sort.dir;

    let active = |col: PositionSortCol| sort_col == col;
    let h_symbol = sorted_header("Symbol", active(PositionSortCol::Symbol), sort_dir);
    let h_qty = sorted_header("Qty", active(PositionSortCol::Qty), sort_dir);
    let h_avg = sorted_header("Avg Cost", active(PositionSortCol::AvgCost), sort_dir);
    let h_mkt = sorted_header("Mkt Value", active(PositionSortCol::MarketValue), sort_dir);
    let h_pnl = sorted_header(
        "Unrealized P&L",
        active(PositionSortCol::UnrealizedPl),
        sort_dir,
    );
    let h_pct = sorted_header("%", active(PositionSortCol::Pct), sort_dir);
    let header = Row::new(vec![
        header_cell(&h_symbol, &c),
        header_cell(&h_qty, &c),
        header_cell(&h_avg, &c),
        header_cell("Cur Price", &c),
        header_cell(&h_mkt, &c),
        header_cell(&h_pnl, &c),
        header_cell(&h_pct, &c),
    ]);

    // Sort a clone of the positions slice.
    let mut sorted: Vec<_> = app.positions.iter().collect();
    match sort_col {
        PositionSortCol::None => {}
        PositionSortCol::Symbol => sorted.sort_by(|a, b| a.symbol.cmp(&b.symbol)),
        PositionSortCol::Qty => sorted.sort_by(|a, b| {
            let av = a.qty.parse::<f64>().unwrap_or(0.0);
            let bv = b.qty.parse::<f64>().unwrap_or(0.0);
            av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
        }),
        PositionSortCol::AvgCost => sorted.sort_by(|a, b| {
            let av = a.avg_entry_price.parse::<f64>().unwrap_or(0.0);
            let bv = b.avg_entry_price.parse::<f64>().unwrap_or(0.0);
            av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
        }),
        PositionSortCol::MarketValue => sorted.sort_by(|a, b| {
            let av = a.market_value.parse::<f64>().unwrap_or(0.0);
            let bv = b.market_value.parse::<f64>().unwrap_or(0.0);
            av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
        }),
        PositionSortCol::UnrealizedPl => sorted.sort_by(|a, b| {
            let av = a.unrealized_pl.trim().parse::<f64>().unwrap_or(0.0);
            let bv = b.unrealized_pl.trim().parse::<f64>().unwrap_or(0.0);
            av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
        }),
        PositionSortCol::Pct => sorted.sort_by(|a, b| {
            let av = a.unrealized_plpc.parse::<f64>().unwrap_or(0.0);
            let bv = b.unrealized_plpc.parse::<f64>().unwrap_or(0.0);
            av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
        }),
    }
    if sort_dir == SortDir::Desc {
        sorted.reverse();
    }

    let mut rows: Vec<Row> = sorted
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

    // ── Sort indicator tests ───────────────────────────────────────────────────

    #[test]
    fn positions_sort_by_symbol_asc_shows_indicator() {
        let mut app = make_test_app();
        app.positions.push(make_position("MSFT", "50.00"));
        app.positions.push(make_position("AAPL", "100.00"));
        app.positions_sort.col = crate::app::PositionSortCol::Symbol;
        app.positions_sort.dir = crate::app::SortDir::Asc;
        let output = render_positions_to_string(&mut app);
        // Header should contain the ▲ indicator next to Symbol
        assert!(
            output.contains("Symbol ▲") || output.contains("Symbol▲"),
            "expected ascending sort indicator on Symbol, got: {output}"
        );
    }

    #[test]
    fn positions_sort_by_symbol_desc_shows_indicator() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "100.00"));
        app.positions.push(make_position("MSFT", "50.00"));
        app.positions_sort.col = crate::app::PositionSortCol::Symbol;
        app.positions_sort.dir = crate::app::SortDir::Desc;
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("Symbol ▼") || output.contains("Symbol▼"),
            "expected descending sort indicator on Symbol, got: {output}"
        );
    }

    #[test]
    fn positions_sorted_by_symbol_asc_orders_rows_alphabetically() {
        let mut app = make_test_app();
        // Push in reverse-alphabetical order
        app.positions.push(make_position("TSLA", "100.00"));
        app.positions.push(make_position("AAPL", "50.00"));
        app.positions_sort.col = crate::app::PositionSortCol::Symbol;
        app.positions_sort.dir = crate::app::SortDir::Asc;
        let output = render_positions_to_string(&mut app);
        let aapl_pos = output.find("AAPL").expect("AAPL should be in output");
        let tsla_pos = output.find("TSLA").expect("TSLA should be in output");
        assert!(
            aapl_pos < tsla_pos,
            "AAPL should appear before TSLA when sorted ascending by symbol"
        );
    }

    #[test]
    fn positions_sorted_by_symbol_desc_reverses_order() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "50.00"));
        app.positions.push(make_position("TSLA", "100.00"));
        app.positions_sort.col = crate::app::PositionSortCol::Symbol;
        app.positions_sort.dir = crate::app::SortDir::Desc;
        let output = render_positions_to_string(&mut app);
        let aapl_pos = output.find("AAPL").expect("AAPL should be in output");
        let tsla_pos = output.find("TSLA").expect("TSLA should be in output");
        assert!(
            tsla_pos < aapl_pos,
            "TSLA should appear before AAPL when sorted descending by symbol"
        );
    }

    #[test]
    fn positions_no_sort_shows_no_indicator() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "100.00"));
        // Default sort is None
        let output = render_positions_to_string(&mut app);
        assert!(
            !output.contains('▲') && !output.contains('▼'),
            "no sort indicator expected when sort col is None, got: {output}"
        );
    }

    #[test]
    fn positions_sorted_by_unrealized_pl_asc() {
        let mut app = make_test_app();
        app.positions.push(make_position("TSLA", "200.00"));
        app.positions.push(make_position("AAPL", "50.00"));
        app.positions_sort.col = crate::app::PositionSortCol::UnrealizedPl;
        app.positions_sort.dir = crate::app::SortDir::Asc;
        let output = render_positions_to_string(&mut app);
        let aapl_pos = output.find("AAPL").expect("AAPL should be in output");
        let tsla_pos = output.find("TSLA").expect("TSLA should be in output");
        assert!(
            aapl_pos < tsla_pos,
            "AAPL (lower P&L) should appear before TSLA when sorted ascending by P&L"
        );
    }

    // ── Additional sort-column coverage ───────────────────────────────────────

    fn make_position_with_qty(symbol: &str, qty: &str) -> Position {
        Position {
            symbol: symbol.into(),
            qty: qty.into(),
            avg_entry_price: "100.00".into(),
            current_price: "110.00".into(),
            market_value: "1100.00".into(),
            unrealized_pl: "100.00".into(),
            unrealized_plpc: "0.10".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }
    }

    fn make_position_full(
        symbol: &str,
        qty: &str,
        avg: &str,
        mkt: &str,
        pnl: &str,
        pct: &str,
    ) -> Position {
        Position {
            symbol: symbol.into(),
            qty: qty.into(),
            avg_entry_price: avg.into(),
            current_price: "110.00".into(),
            market_value: mkt.into(),
            unrealized_pl: pnl.into(),
            unrealized_plpc: pct.into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }
    }

    #[test]
    fn positions_sort_by_qty_asc_shows_indicator() {
        let mut app = make_test_app();
        app.positions.push(make_position_with_qty("AAPL", "5"));
        app.positions_sort.col = crate::app::PositionSortCol::Qty;
        app.positions_sort.dir = crate::app::SortDir::Asc;
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("Qty ▲") || output.contains("Qty▲"),
            "expected Qty ▲ header, got: {output}"
        );
    }

    #[test]
    fn positions_sort_by_qty_desc_orders_rows() {
        let mut app = make_test_app();
        app.positions.push(make_position_with_qty("AAPL", "5"));
        app.positions.push(make_position_with_qty("TSLA", "20"));
        app.positions_sort.col = crate::app::PositionSortCol::Qty;
        app.positions_sort.dir = crate::app::SortDir::Desc;
        let output = render_positions_to_string(&mut app);
        // Descending: TSLA (20) should appear before AAPL (5)
        let aapl_pos = output.find("AAPL").expect("AAPL should be present");
        let tsla_pos = output.find("TSLA").expect("TSLA should be present");
        assert!(
            tsla_pos < aapl_pos,
            "TSLA (qty=20) should precede AAPL (qty=5) in desc order"
        );
    }

    #[test]
    fn positions_sort_by_avg_cost_asc_shows_indicator() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "50.00"));
        app.positions_sort.col = crate::app::PositionSortCol::AvgCost;
        app.positions_sort.dir = crate::app::SortDir::Asc;
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("Avg Cost ▲") || output.contains("Avg Cost▲"),
            "expected Avg Cost ▲ header, got: {output}"
        );
    }

    #[test]
    fn positions_sort_by_avg_cost_orders_rows() {
        let mut app = make_test_app();
        app.positions.push(make_position_full(
            "CHEAP", "10", "50.00", "500.00", "10.00", "0.02",
        ));
        app.positions.push(make_position_full(
            "PRICEY", "10", "300.00", "3000.00", "100.00", "0.05",
        ));
        app.positions_sort.col = crate::app::PositionSortCol::AvgCost;
        app.positions_sort.dir = crate::app::SortDir::Asc;
        let output = render_positions_to_string(&mut app);
        let cheap_pos = output.find("CHEAP").expect("CHEAP should be present");
        let pricey_pos = output.find("PRICEY").expect("PRICEY should be present");
        assert!(
            cheap_pos < pricey_pos,
            "CHEAP should appear before PRICEY sorted asc by avg cost"
        );
    }

    #[test]
    fn positions_sort_by_market_value_asc_shows_indicator() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "50.00"));
        app.positions_sort.col = crate::app::PositionSortCol::MarketValue;
        app.positions_sort.dir = crate::app::SortDir::Asc;
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("Mkt Value ▲") || output.contains("Mkt Value▲"),
            "expected Mkt Value ▲ header, got: {output}"
        );
    }

    #[test]
    fn positions_sort_by_market_value_desc_orders_rows() {
        let mut app = make_test_app();
        app.positions.push(make_position_full(
            "SMALL", "10", "100.00", "500.00", "10.00", "0.02",
        ));
        app.positions.push(make_position_full(
            "BIG", "10", "100.00", "5000.00", "100.00", "0.10",
        ));
        app.positions_sort.col = crate::app::PositionSortCol::MarketValue;
        app.positions_sort.dir = crate::app::SortDir::Desc;
        let output = render_positions_to_string(&mut app);
        let big_pos = output.find("BIG").expect("BIG should be present");
        let small_pos = output.find("SMALL").expect("SMALL should be present");
        assert!(
            big_pos < small_pos,
            "BIG should appear before SMALL in desc market-value order"
        );
    }

    #[test]
    fn positions_sort_by_pct_asc_shows_indicator() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "50.00"));
        app.positions_sort.col = crate::app::PositionSortCol::Pct;
        app.positions_sort.dir = crate::app::SortDir::Asc;
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("% ▲") || output.contains("%▲"),
            "expected % ▲ header, got: {output}"
        );
    }

    #[test]
    fn positions_sort_by_pct_orders_rows() {
        let mut app = make_test_app();
        app.positions.push(make_position_full(
            "LOSER", "10", "100.00", "900.00", "-100.00", "-0.10",
        ));
        app.positions.push(make_position_full(
            "WINNER", "10", "100.00", "1200.00", "200.00", "0.20",
        ));
        app.positions_sort.col = crate::app::PositionSortCol::Pct;
        app.positions_sort.dir = crate::app::SortDir::Asc;
        let output = render_positions_to_string(&mut app);
        let loser_pos = output.find("LOSER").expect("LOSER should be present");
        let winner_pos = output.find("WINNER").expect("WINNER should be present");
        assert!(
            loser_pos < winner_pos,
            "LOSER (-10%) should appear before WINNER (+20%) sorted asc"
        );
    }

    #[test]
    fn positions_sort_by_unrealized_pl_desc_shows_indicator() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "50.00"));
        app.positions_sort.col = crate::app::PositionSortCol::UnrealizedPl;
        app.positions_sort.dir = crate::app::SortDir::Desc;
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("Unrealized P&L ▼") || output.contains("Unrealized P&L▼"),
            "expected Unrealized P&L ▼ header, got: {output}"
        );
    }

    #[test]
    fn positions_sort_by_pct_desc_shows_indicator() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "50.00"));
        app.positions_sort.col = crate::app::PositionSortCol::Pct;
        app.positions_sort.dir = crate::app::SortDir::Desc;
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("% ▼") || output.contains("%▼"),
            "expected % ▼ header, got: {output}"
        );
    }

    #[test]
    fn positions_live_quote_overrides_current_price() {
        use crate::types::Quote;
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL", "100.00"));
        // Inject a live ask-price quote
        app.quotes.insert(
            "AAPL".into(),
            Quote {
                ap: Some(175.99),
                bp: None,
                ..Default::default()
            },
        );
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("175.99"),
            "expected live ask price in cur-price column, got: {output}"
        );
    }

    #[test]
    fn positions_live_quote_bid_used_when_no_ask() {
        use crate::types::Quote;
        let mut app = make_test_app();
        app.positions.push(make_position("MSFT", "50.00"));
        app.quotes.insert(
            "MSFT".into(),
            Quote {
                ap: None,
                bp: Some(299.50),
                ..Default::default()
            },
        );
        let output = render_positions_to_string(&mut app);
        assert!(
            output.contains("299.50"),
            "expected live bid price in cur-price column, got: {output}"
        );
    }
}
