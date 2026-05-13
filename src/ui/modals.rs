use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Sparkline, Table},
    Frame,
};

use crate::app::{App, ConfirmAction, Modal, OrderEntryState, OrderField};
use crate::ui::{popup_area, theme};

pub fn render(frame: &mut Frame, area: Rect, modal: &Modal, app: &mut App) {
    match modal {
        Modal::Help => render_help(frame, area),
        Modal::OrderEntry(state) => render_order_entry(frame, area, state, app),
        Modal::SymbolDetail(symbol) => render_symbol_detail(frame, area, symbol, app),
        Modal::Confirm {
            message,
            action,
            confirmed,
        } => render_confirm(frame, area, message, action, *confirmed, app),
        Modal::AddSymbol { input, .. } => render_add_symbol(frame, area, input),
    }
}

fn render_help(frame: &mut Frame, area: Rect) {
    let popup = popup_area(area, 50, 70);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Keyboard Shortcuts ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme::BRAND_CYAN));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let rows = vec![
        ("NAVIGATION", ""),
        ("1/2/3/4 or Tab", "Switch panels"),
        ("j / k  or ↑/↓", "Move cursor"),
        ("g / G", "Top / Bottom"),
        ("Enter", "Open detail"),
        ("Esc", "Close / Cancel"),
        ("", ""),
        ("ACTIONS", ""),
        ("o", "New order (pre-fills symbol)"),
        ("c", "Cancel selected order"),
        ("a", "Add symbol to watchlist"),
        ("d", "Remove symbol from watchlist"),
        ("r", "Force refresh"),
        ("/", "Search / filter watchlist"),
        ("", ""),
        ("GLOBAL", ""),
        ("q / Ctrl-C", "Quit"),
        ("?", "This help screen"),
    ];

    let header = Row::new(vec![
        Cell::from("Key").style(theme::style_header()),
        Cell::from("Action").style(theme::style_header()),
    ]);

    let table_rows: Vec<Row> = rows
        .iter()
        .map(|(k, v)| {
            if v.is_empty() {
                Row::new(vec![
                    Cell::from(*k).style(
                        Style::default()
                            .fg(theme::BRAND_CYAN)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Cell::from(""),
                ])
            } else {
                Row::new(vec![
                    Cell::from(*k).style(Style::default().fg(theme::DIM)),
                    Cell::from(*v),
                ])
            }
        })
        .collect();

    let table =
        Table::new(table_rows, [Constraint::Length(18), Constraint::Min(20)]).header(header);

    frame.render_widget(table, inner);

    // Footer hint
    let footer_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    let footer = Paragraph::new("  Press any key to close")
        .alignment(Alignment::Center)
        .style(Style::default().fg(theme::DIM));
    frame.render_widget(footer, footer_area);
}

fn render_order_entry(frame: &mut Frame, area: Rect, state: &OrderEntryState, app: &mut App) {
    let popup = popup_area(area, 45, 65);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" New Order ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme::BRAND_CYAN));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // [0]  Symbol
            Constraint::Length(1), // [1]  blank
            Constraint::Length(1), // [2]  Side
            Constraint::Length(1), // [3]  Type
            Constraint::Length(1), // [4]  Qty
            Constraint::Length(1), // [5]  Price
            Constraint::Length(1), // [6]  TimeInForce
            Constraint::Length(1), // [7]  blank
            Constraint::Length(1), // [8]  Est Total
            Constraint::Length(1), // [9]  Buying Power
            Constraint::Length(1), // [10] blank
            Constraint::Length(1), // [11] Market-closed warning
            Constraint::Length(1), // [12] Submit / Cancel
            Constraint::Length(1), // [13] hint
        ])
        .split(inner);

    let market_open = app.clock.as_ref().map(|c| c.is_open).unwrap_or(true);

    // Populate hit areas for mouse click handling
    app.hit_areas.modal_fields = vec![
        (OrderField::Symbol, chunks[0]),
        (OrderField::Side, chunks[2]),
        (OrderField::OrderType, chunks[3]),
        (OrderField::Qty, chunks[4]),
        (OrderField::Price, chunks[5]),
        (OrderField::TimeInForce, chunks[6]),
    ];
    app.hit_areas.modal_submit = Some(chunks[12]);

    let focused = |field: &OrderField| *field == state.focused_field;

    let field_style = |field: &OrderField| {
        if focused(field) {
            Style::default()
                .fg(theme::BRAND_CYAN)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        }
    };

    // Symbol
    frame.render_widget(
        field_line(
            "Symbol",
            &format!(
                "{}{}",
                state.symbol,
                if focused(&OrderField::Symbol) {
                    "▋"
                } else {
                    ""
                }
            ),
            field_style(&OrderField::Symbol),
        ),
        chunks[0],
    );

    // Side
    let side_line = Line::from(vec![
        Span::styled("  Side    ", Style::default().fg(theme::DIM)),
        radio(state.side == crate::app::OrderSide::Buy, "BUY"),
        Span::raw("  "),
        radio(state.side == crate::app::OrderSide::Sell, "SELL"),
        Span::raw("  "),
        radio(state.side == crate::app::OrderSide::SellShort, "SELL SHORT"),
    ]);
    frame.render_widget(Paragraph::new(side_line), chunks[2]);

    // Type
    let type_line = Line::from(vec![
        Span::styled("  Type    ", Style::default().fg(theme::DIM)),
        radio(!state.market_order, "LIMIT"),
        Span::raw("  "),
        radio(state.market_order, "MARKET"),
    ]);
    frame.render_widget(Paragraph::new(type_line), chunks[3]);

    // Qty
    frame.render_widget(
        field_line(
            "Qty   ",
            &format!(
                "{}{}",
                state.qty_input,
                if focused(&OrderField::Qty) { "▋" } else { "" }
            ),
            field_style(&OrderField::Qty),
        ),
        chunks[4],
    );

    // Price (only shown for limit)
    if !state.market_order {
        frame.render_widget(
            field_line(
                "Price ",
                &format!(
                    "{}{}",
                    state.price_input,
                    if focused(&OrderField::Price) {
                        "▋"
                    } else {
                        ""
                    }
                ),
                field_style(&OrderField::Price),
            ),
            chunks[5],
        );
    } else {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  Price ", Style::default().fg(theme::DIM)),
                Span::styled("N/A (Market order)", Style::default().fg(theme::DIM)),
            ])),
            chunks[5],
        );
    }

    // TimeInForce
    let tif_line = Line::from(vec![
        Span::styled("  TIF     ", Style::default().fg(theme::DIM)),
        radio(!state.gtc_order, "DAY"),
        Span::raw("  "),
        radio(state.gtc_order, "GTC"),
    ]);
    frame.render_widget(Paragraph::new(tif_line), chunks[6]);

    // Est Total
    let est_total = estimate_total(state);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Est. Total  ", Style::default().fg(theme::DIM)),
            Span::styled(est_total, theme::style_bold()),
        ])),
        chunks[8],
    );

    // Buying power
    let bp = app
        .account
        .as_ref()
        .map(|a| format!("${:.2}", a.buying_power.parse::<f64>().unwrap_or(0.0)))
        .unwrap_or_else(|| "—".into());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Buying Power  ", Style::default().fg(theme::DIM)),
            Span::styled(bp, theme::style_bold()),
        ])),
        chunks[9],
    );

    // Market-closed warning
    if !market_open && !state.gtc_order {
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                "  ⚠ Market closed — switch to GTC or wait",
                Style::default()
                    .fg(theme::YELLOW)
                    .add_modifier(Modifier::BOLD),
            )])),
            chunks[11],
        );
    }

    // Submit button — dimmed when market is closed and order is DAY
    let market_closed_day = !market_open && !state.gtc_order;
    let submit_style = if focused(&OrderField::Submit) && !market_closed_day {
        Style::default()
            .fg(theme::BRAND_CYAN)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else if market_closed_day {
        Style::default().fg(theme::DIM)
    } else {
        Style::default()
    };
    let buttons = Line::from(vec![
        Span::styled("  [ Submit Order ]", submit_style),
        Span::raw("  "),
        Span::styled("[ Esc: Cancel ]", Style::default().fg(theme::DIM)),
    ]);
    frame.render_widget(Paragraph::new(buttons), chunks[12]);

    // Hint
    frame.render_widget(
        Paragraph::new("  Tab:Next  ←/→:Toggle  Enter:Advance  Esc:Close")
            .style(Style::default().fg(theme::DIM)),
        chunks[13],
    );
}

fn render_symbol_detail(frame: &mut Frame, area: Rect, symbol: &str, app: &App) {
    let popup = popup_area(area, 55, 88);
    frame.render_widget(Clear, popup);

    let asset = app
        .watchlist
        .as_ref()
        .and_then(|w| w.assets.iter().find(|a| a.symbol == symbol));

    let name = asset.map(|a| a.name.as_str()).unwrap_or(symbol);

    let in_watchlist = app
        .watchlist
        .as_ref()
        .map(|w| w.assets.iter().any(|a| a.symbol == symbol))
        .unwrap_or(false);

    let block = Block::default()
        .title(format!(" {} — {} ", symbol, name))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme::BRAND_CYAN));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    // ── Data ─────────────────────────────────────────────────────────────────
    let quote = app.quotes.get(symbol);
    let snapshot = app.snapshots.get(symbol);
    let daily = snapshot.and_then(|s| s.daily_bar.as_ref());
    let prev = snapshot.and_then(|s| s.prev_daily_bar.as_ref());

    let price: Option<f64> = quote
        .and_then(|q| match (q.ap, q.bp) {
            (Some(a), Some(b)) => Some((a + b) / 2.0),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            _ => None,
        })
        .or_else(|| daily.map(|b| b.c));

    let change_pct: Option<f64> = price.zip(prev.map(|b| b.c)).map(|(p, pc)| {
        if pc != 0.0 {
            (p - pc) / pc * 100.0
        } else {
            0.0
        }
    });

    let price_str = price
        .map(|p| format!("${:.2}", p))
        .unwrap_or_else(|| "—".into());
    let change_str = change_pct
        .map(|c| format!("{:+.2}%", c))
        .unwrap_or_else(|| "—".into());
    let value_style = change_pct
        .map(|c| {
            if c >= 0.0 {
                Style::default().fg(theme::GREEN)
            } else {
                Style::default().fg(theme::RED)
            }
        })
        .unwrap_or_else(theme::style_bold);

    let open_str = daily
        .map(|b| format!("${:.2}", b.o))
        .unwrap_or_else(|| "—".into());
    let high_str = daily
        .map(|b| format!("${:.2}", b.h))
        .unwrap_or_else(|| "—".into());
    let low_str = daily
        .map(|b| format!("${:.2}", b.l))
        .unwrap_or_else(|| "—".into());
    let vol_str = daily
        .map(|b| crate::ui::watchlist::format_volume(b.v))
        .unwrap_or_else(|| "—".into());

    let wl_label = if in_watchlist {
        "w:− Watchlist"
    } else {
        "w:+ Watchlist"
    };

    // ── Layout ───────────────────────────────────────────────────────────────
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // blank
            Constraint::Length(1), // price + change%
            Constraint::Length(1), // open + high
            Constraint::Length(1), // low + volume
            Constraint::Length(1), // blank
            Constraint::Length(1), // "── Intraday ──" label
            Constraint::Length(3), // sparkline
            Constraint::Length(1), // blank
            Constraint::Length(1), // exchange + class
            Constraint::Length(1), // tradable + shortable
            Constraint::Length(1), // fractional + etb
            Constraint::Min(0),    // filler
            Constraint::Length(1), // footer
        ])
        .split(inner);

    // ── OHLCV rows ───────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Price    ", Style::default().fg(theme::DIM)),
            Span::styled(price_str, value_style),
            Span::raw("   "),
            Span::styled("Change    ", Style::default().fg(theme::DIM)),
            Span::styled(change_str, value_style),
        ])),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Open     ", Style::default().fg(theme::DIM)),
            Span::styled(open_str, theme::style_bold()),
            Span::raw("   "),
            Span::styled("High      ", Style::default().fg(theme::DIM)),
            Span::styled(high_str, Style::default().fg(theme::GREEN)),
        ])),
        chunks[2],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Low      ", Style::default().fg(theme::DIM)),
            Span::styled(low_str, Style::default().fg(theme::RED)),
            Span::raw("   "),
            Span::styled("Volume    ", Style::default().fg(theme::DIM)),
            Span::styled(vol_str, theme::style_bold()),
        ])),
        chunks[3],
    );

    // ── Intraday sparkline ────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "  ── Intraday ──",
            Style::default().fg(theme::DIM),
        )])),
        chunks[5],
    );

    match app.intraday_bars.get(symbol) {
        None => {
            // Command dispatched but response not yet received
            frame.render_widget(
                Paragraph::new("  Loading…").style(Style::default().fg(theme::DIM)),
                chunks[6],
            );
        }
        Some(bars) if bars.is_empty() => {
            // Fetched but no bars (market closed, pre-market, or error)
            frame.render_widget(
                Paragraph::new("  No intraday data available")
                    .style(Style::default().fg(theme::DIM)),
                chunks[6],
            );
        }
        Some(bars) => {
            frame.render_widget(
                Sparkline::default()
                    .data(bars)
                    .style(Style::default().fg(theme::BRAND_CYAN)),
                chunks[6],
            );
        }
    }

    // ── Asset flags ──────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Exchange ", Style::default().fg(theme::DIM)),
            Span::styled(
                asset
                    .map(|a| a.exchange.as_str())
                    .unwrap_or("—")
                    .to_string(),
                theme::style_bold(),
            ),
            Span::raw("   "),
            Span::styled("Class     ", Style::default().fg(theme::DIM)),
            Span::styled(
                asset
                    .map(|a| a.asset_class.as_str())
                    .unwrap_or("—")
                    .to_string(),
                theme::style_bold(),
            ),
        ])),
        chunks[8],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Tradable ", Style::default().fg(theme::DIM)),
            Span::styled(
                flag(asset.map(|a| a.tradable).unwrap_or(false)),
                Style::default().fg(theme::GREEN),
            ),
            Span::raw("   "),
            Span::styled("Shortable ", Style::default().fg(theme::DIM)),
            Span::styled(
                flag(asset.map(|a| a.shortable).unwrap_or(false)),
                Style::default().fg(theme::GREEN),
            ),
        ])),
        chunks[9],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Fractional ", Style::default().fg(theme::DIM)),
            Span::styled(
                flag(asset.map(|a| a.fractionable).unwrap_or(false)),
                Style::default().fg(theme::GREEN),
            ),
            Span::raw(" "),
            Span::styled("ETB       ", Style::default().fg(theme::DIM)),
            Span::styled(
                flag(asset.map(|a| a.easy_to_borrow).unwrap_or(false)),
                Style::default().fg(theme::GREEN),
            ),
        ])),
        chunks[10],
    );

    // ── Footer ───────────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!("  o:Buy  s:Sell  {}  Esc:Close", wl_label),
            Style::default().fg(theme::DIM),
        )])),
        chunks[12],
    );
}

fn render_confirm(
    frame: &mut Frame,
    area: Rect,
    message: &str,
    _action: &ConfirmAction,
    confirmed: bool,
    app: &mut App,
) {
    let popup = popup_area(area, 40, 25);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme::BRAND_RED));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    // Store button row for mouse hit-testing (left = Yes, right = No)
    app.hit_areas.modal_confirm_buttons = Some(chunks[2]);

    frame.render_widget(
        Paragraph::new(format!("  {}", message)).style(theme::style_bold()),
        chunks[0],
    );

    let yes_style = if confirmed {
        Style::default()
            .fg(theme::GREEN)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default().fg(theme::GREEN)
    };
    let no_style = if !confirmed {
        Style::default()
            .fg(theme::RED)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default().fg(theme::RED)
    };

    let buttons = Line::from(vec![
        Span::styled("  [ y: Yes ]", yes_style),
        Span::raw("  "),
        Span::styled("[ n: No ]", no_style),
    ]);
    frame.render_widget(Paragraph::new(buttons), chunks[2]);
}

fn render_add_symbol(frame: &mut Frame, area: Rect, input: &str) {
    let popup = popup_area(area, 35, 20);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Add Symbol ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme::BRAND_CYAN));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new("  Enter ticker symbol:").style(Style::default().fg(theme::DIM)),
        chunks[0],
    );

    let input_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(input.to_string(), theme::style_bold()),
        Span::styled("▋", Style::default().fg(theme::BRAND_CYAN)),
    ]);
    frame.render_widget(Paragraph::new(input_line), chunks[1]);

    frame.render_widget(
        Paragraph::new("  Enter:Add  Esc:Cancel").style(Style::default().fg(theme::DIM)),
        chunks[2],
    );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn field_line<'a>(label: &'a str, value: &'a str, style: Style) -> Paragraph<'a> {
    Paragraph::new(Line::from(vec![
        Span::styled(format!("  {:<8}", label), Style::default().fg(theme::DIM)),
        Span::styled(value.to_string(), style),
    ]))
}

fn radio(selected: bool, label: &str) -> Span<'static> {
    let marker = if selected { "● " } else { "○ " };
    let style = if selected {
        Style::default()
            .fg(theme::BRAND_CYAN)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::DIM)
    };
    Span::styled(format!("{}{}", marker, label), style)
}

fn flag(v: bool) -> &'static str {
    if v {
        "✓"
    } else {
        "✗"
    }
}

fn estimate_total(state: &OrderEntryState) -> String {
    let qty: f64 = state.qty_input.parse().unwrap_or(0.0);
    let price: f64 = state.price_input.parse().unwrap_or(0.0);
    if qty > 0.0 && price > 0.0 {
        format!("${:.2}", qty * price)
    } else {
        "—".into()
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, Terminal};

    use super::*;
    use crate::app::test_helpers::{make_test_app, make_watchlist};

    fn render_symbol_detail_to_string(app: &mut App, symbol: &str) -> String {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_symbol_detail(frame, area, symbol, app);
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
    fn render_symbol_detail_shows_loading_when_no_bars_key() {
        let mut app = make_test_app();
        // No entry in intraday_bars → "Loading…"
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("Loading"),
            "should show Loading when intraday_bars has no entry for symbol"
        );
    }

    #[test]
    fn render_symbol_detail_shows_no_data_when_bars_empty() {
        let mut app = make_test_app();
        app.intraday_bars.insert("AAPL".into(), vec![]);
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("No intraday data"),
            "should show 'No intraday data available' when bars vec is empty"
        );
    }

    #[test]
    fn render_symbol_detail_renders_sparkline_with_bars() {
        let mut app = make_test_app();
        app.intraday_bars
            .insert("AAPL".into(), vec![15000, 15050, 15100, 15080, 15120]);
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        // The sparkline renders something other than "Loading" or "No intraday data"
        assert!(
            !output.contains("Loading"),
            "should not show Loading when bars are present"
        );
        assert!(
            !output.contains("No intraday data"),
            "should not show no-data message when bars are present"
        );
    }

    #[test]
    fn render_symbol_detail_shows_ohlcv_labels() {
        let mut app = make_test_app();
        let output = render_symbol_detail_to_string(&mut app, "TSLA");
        assert!(output.contains("Price"), "should show Price label");
        assert!(output.contains("Open"), "should show Open label");
        assert!(output.contains("High"), "should show High label");
        assert!(output.contains("Low"), "should show Low label");
        assert!(output.contains("Volume"), "should show Volume label");
    }

    #[test]
    fn render_symbol_detail_shows_footer_actions() {
        let mut app = make_test_app();
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        assert!(output.contains("o:Buy"), "footer should contain buy action");
        assert!(
            output.contains("s:Sell"),
            "footer should contain sell action"
        );
        assert!(
            output.contains("Esc:Close"),
            "footer should contain close hint"
        );
    }

    #[test]
    fn render_symbol_detail_shows_watchlist_label_not_in_watchlist() {
        let mut app = make_test_app();
        // AAPL is not in watchlist
        app.watchlist = Some(make_watchlist(&["TSLA"]));
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("w:+"),
            "footer should show 'w:+ Watchlist' when symbol not in watchlist"
        );
    }

    #[test]
    fn render_symbol_detail_shows_watchlist_label_in_watchlist() {
        let mut app = make_test_app();
        app.watchlist = Some(make_watchlist(&["AAPL", "TSLA"]));
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("w:\u{2212}") || output.contains("w:-"),
            "footer should show 'w:− Watchlist' when symbol is in watchlist"
        );
    }

    #[test]
    fn render_symbol_detail_uses_asset_name_in_title() {
        let mut app = make_test_app();
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        // The watchlist asset name is "AAPL Corp" per make_asset
        assert!(
            output.contains("AAPL Corp"),
            "title should include asset name from watchlist"
        );
    }

    #[test]
    fn render_symbol_detail_falls_back_to_symbol_as_name() {
        let mut app = make_test_app();
        // No watchlist entry → symbol is used as name
        let output = render_symbol_detail_to_string(&mut app, "NVDA");
        assert!(
            output.contains("NVDA"),
            "title should contain symbol when no asset info available"
        );
    }

    #[test]
    fn render_symbol_detail_with_quote_shows_price() {
        use crate::types::Quote;
        let mut app = make_test_app();
        app.quotes.insert(
            "AAPL".into(),
            Quote {
                symbol: "AAPL".into(),
                ap: Some(185.50),
                bp: Some(185.40),
                ..Default::default()
            },
        );
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        // Midpoint of 185.50 + 185.40 = 185.45
        assert!(
            output.contains("185.45"),
            "should display midpoint price from quote"
        );
    }

    fn render_order_entry_to_string(app: &mut App, state: crate::app::OrderEntryState) -> String {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                let modal = Modal::OrderEntry(state);
                render(frame, area, &modal, app);
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
    fn render_order_entry_buy_shows_buy_selected() {
        use crate::app::{OrderEntryState, OrderSide};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new("AAPL".into());
        state.side = OrderSide::Buy;
        let output = render_order_entry_to_string(&mut app, state);
        assert!(output.contains("BUY"), "order entry should show BUY option");
        assert!(
            output.contains("SELL"),
            "order entry should show SELL option"
        );
        assert!(
            output.contains("SELL SHORT"),
            "order entry should show SELL SHORT option"
        );
    }

    #[test]
    fn render_order_entry_sell_short_shows_sell_short_selected() {
        use crate::app::{OrderEntryState, OrderSide};
        let mut app = make_test_app();
        let mut state = OrderEntryState::new("TSLA".into());
        state.side = OrderSide::SellShort;
        let output = render_order_entry_to_string(&mut app, state);
        assert!(
            output.contains("SELL SHORT"),
            "order entry with SellShort should display SELL SHORT option"
        );
    }
}
