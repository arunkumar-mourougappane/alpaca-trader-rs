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
        Cell::from("Change").style(theme::style_header()),
        Cell::from("Ask").style(theme::style_header()),
        Cell::from("Bid").style(theme::style_header()),
    ]);

    let rows: Vec<Row> = filtered
        .iter()
        .map(|asset| {
            let quote = app.quotes.get(&asset.symbol);
            let price = quote
                .and_then(|q| q.ap.or(q.bp))
                .map(|p| format!("${:.2}", p))
                .unwrap_or_else(|| "—".into());
            let ask = quote
                .and_then(|q| q.ap)
                .map(|p| format!("${:.2}", p))
                .unwrap_or_else(|| "—".into());
            let bid = quote
                .and_then(|q| q.bp)
                .map(|p| format!("${:.2}", p))
                .unwrap_or_else(|| "—".into());

            Row::new(vec![
                Cell::from(asset.symbol.clone()).style(theme::style_bold()),
                Cell::from(asset.name.clone()),
                Cell::from(price),
                Cell::from("—"), // Change requires prev close (Phase 2)
                Cell::from(ask),
                Cell::from(bid),
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
            Constraint::Min(28),
            Constraint::Length(10),
            Constraint::Length(9),
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
