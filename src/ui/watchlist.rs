use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::App;
use crate::ui::theme;

/// Format a trading volume number into a compact human-readable string.
///
/// - ≥ 1 000 000  → `"28.7M"`
/// - ≥ 1 000      → `"1.2K"`
/// - otherwise    → the raw integer as a string
pub fn format_volume(v: f64) -> String {
    if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("{:.1}K", v / 1_000.0)
    } else {
        format!("{}", v as u64)
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let wl = match app.watchlist.clone() {
        Some(w) => w,
        None => {
            let para = Paragraph::new("  Loading watchlist…")
                .style(Style::default().fg(theme::DIM))
                .block(Block::default().title(" Watchlist ").borders(Borders::ALL));
            frame.render_widget(para, area);
            return;
        }
    };

    let (table_area, search_area) = if app.searching {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let query = app.search_query.to_lowercase();
    let filtered: Vec<_> = wl
        .assets
        .iter()
        .filter(|a| {
            if app.searching && !query.is_empty() {
                a.symbol.to_lowercase().contains(&query) || a.name.to_lowercase().contains(&query)
            } else {
                true
            }
        })
        .collect();

    let header = Row::new(vec![
        Cell::from("Symbol").style(theme::style_header()),
        Cell::from("Name").style(theme::style_header()),
        Cell::from("Price").style(theme::style_header()),
        Cell::from("Change%").style(theme::style_header()),
        Cell::from("Volume").style(theme::style_header()),
    ]);

    let rows: Vec<Row> = filtered
        .iter()
        .map(|asset| {
            let quote = app.quotes.get(&asset.symbol);
            let snapshot = app.snapshots.get(&asset.symbol);

            // Current price: prefer real-time ask/bid quote, fall back to
            // snapshot latest quote, then latest trade (works when market closed).
            let current_price = quote.and_then(|q| q.ap.or(q.bp)).or_else(|| {
                snapshot.and_then(|s| {
                    s.latest_quote
                        .as_ref()
                        .and_then(|lq| lq.ap.or(lq.bp))
                        .or_else(|| s.latest_trade.as_ref().map(|lt| lt.p))
                })
            });

            let price_text = current_price
                .map(|p| format!("${:.2}", p))
                .unwrap_or_else(|| "—".into());

            // Change% = (current - prev_close) / prev_close * 100
            let (change_text, change_style) = {
                let prev_close = snapshot
                    .and_then(|s| s.prev_daily_bar.as_ref())
                    .map(|b| b.c);
                match (current_price, prev_close) {
                    (Some(cur), Some(prev)) if prev != 0.0 => {
                        let pct = (cur - prev) / prev * 100.0;
                        let text = format!("{:+.2}%", pct);
                        let style = if pct >= 0.0 {
                            theme::style_positive()
                        } else {
                            theme::style_negative()
                        };
                        (text, style)
                    }
                    _ => ("—".into(), Style::default()),
                }
            };

            // Price cell style: green if up vs prev close, red if down
            let price_style = {
                let prev_close = snapshot
                    .and_then(|s| s.prev_daily_bar.as_ref())
                    .map(|b| b.c);
                match (current_price, prev_close) {
                    (Some(cur), Some(prev)) if prev != 0.0 => {
                        if cur >= prev {
                            theme::style_positive()
                        } else {
                            theme::style_negative()
                        }
                    }
                    _ => Style::default(),
                }
            };

            // Volume from today's daily bar
            let volume_text = snapshot
                .and_then(|s| s.daily_bar.as_ref())
                .map(|b| format_volume(b.v))
                .unwrap_or_else(|| "—".into());

            Row::new(vec![
                Cell::from(asset.symbol.clone()).style(theme::style_bold()),
                Cell::from(asset.name.clone()),
                Cell::from(price_text).style(price_style),
                Cell::from(change_text).style(change_style),
                Cell::from(volume_text),
            ])
        })
        .collect();

    let title = format!(" Watchlist: {} ({}) ", wl.name, filtered.len());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER_COLOR));

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(24),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(theme::style_selected())
    .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, table_area, &mut app.watchlist_state);

    if let Some(sa) = search_area {
        let search_line = Line::from(vec![
            Span::styled(" Search: ", Style::default().fg(theme::DIM)),
            Span::styled(app.search_query.clone(), theme::style_bold()),
            Span::styled("▋", Style::default().fg(theme::BRAND_CYAN)),
        ]);
        let search_box = Paragraph::new(search_line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::BRAND_CYAN)),
        );
        frame.render_widget(search_box, sa);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, Terminal};

    use super::format_volume;
    use crate::app::test_helpers::{make_test_app, make_watchlist};
    use crate::types::{Snapshot, SnapshotBar, SnapshotQuote, SnapshotTrade};

    // ── format_volume ─────────────────────────────────────────────────────────

    #[test]
    fn format_volume_millions() {
        assert_eq!(format_volume(28_700_000.0), "28.7M");
        assert_eq!(format_volume(1_000_000.0), "1.0M");
    }

    #[test]
    fn format_volume_thousands() {
        assert_eq!(format_volume(1_234.0), "1.2K");
        assert_eq!(format_volume(1_000.0), "1.0K");
    }

    #[test]
    fn format_volume_small() {
        assert_eq!(format_volume(999.0), "999");
        assert_eq!(format_volume(0.0), "0");
    }

    // ── price fallback render tests ───────────────────────────────────────────

    fn render_watchlist_to_string(app: &mut crate::app::App) -> String {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                super::render(frame, frame.area(), app);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut out = String::new();
        for row in 0..buf.area.height {
            for col in 0..buf.area.width {
                out.push_str(buf[(col, row)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn watchlist_shows_dash_when_no_quote_no_snapshot() {
        let mut app = make_test_app();
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        // No quotes, no snapshots — price and change% must show "—"
        let output = render_watchlist_to_string(&mut app);
        assert!(output.contains('—'), "expected em-dash when no price data");
    }

    #[test]
    fn watchlist_shows_price_from_snapshot_latest_trade() {
        let mut app = make_test_app();
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        // Only snapshot latestTrade, no real-time quote
        app.snapshots.insert(
            "AAPL".to_string(),
            Snapshot {
                latest_trade: Some(SnapshotTrade { p: 150.75 }),
                latest_quote: None,
                daily_bar: Some(SnapshotBar {
                    c: 150.75,
                    v: 1_000_000.0,
                    ..Default::default()
                }),
                prev_daily_bar: Some(SnapshotBar {
                    c: 148.0,
                    v: 900_000.0,
                    ..Default::default()
                }),
            },
        );
        let output = render_watchlist_to_string(&mut app);
        assert!(
            output.contains("$150.75"),
            "expected price from latestTrade"
        );
    }

    #[test]
    fn watchlist_shows_price_from_snapshot_latest_quote_ask() {
        let mut app = make_test_app();
        app.watchlist = Some(make_watchlist(&["TSLA"]));
        // Only snapshot latestQuote (ask), no real-time quote
        app.snapshots.insert(
            "TSLA".to_string(),
            Snapshot {
                latest_trade: Some(SnapshotTrade { p: 200.0 }),
                latest_quote: Some(SnapshotQuote {
                    ap: Some(200.50),
                    bp: Some(200.25),
                }),
                daily_bar: None,
                prev_daily_bar: Some(SnapshotBar {
                    c: 195.0,
                    v: 500_000.0,
                    ..Default::default()
                }),
            },
        );
        let output = render_watchlist_to_string(&mut app);
        // Ask price from latestQuote.ap preferred over latestTrade.p
        assert!(
            output.contains("$200.50"),
            "expected ask price from latestQuote"
        );
    }

    #[test]
    fn watchlist_shows_change_pct_from_snapshot() {
        let mut app = make_test_app();
        app.watchlist = Some(make_watchlist(&["NVDA"]));
        app.snapshots.insert(
            "NVDA".to_string(),
            Snapshot {
                latest_trade: Some(SnapshotTrade { p: 110.0 }),
                latest_quote: None,
                daily_bar: None,
                prev_daily_bar: Some(SnapshotBar {
                    c: 100.0,
                    v: 0.0,
                    ..Default::default()
                }),
            },
        );
        let output = render_watchlist_to_string(&mut app);
        // (110 - 100) / 100 * 100 = +10.00%
        assert!(output.contains("+10.00%"), "expected +10.00% change");
    }
}
