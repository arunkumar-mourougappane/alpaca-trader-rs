use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, BorderType, Borders, Cell, Chart, Clear, Dataset, GraphType, Paragraph, Row,
        Table,
    },
    Frame,
};

use crate::app::{App, ConfirmAction, Modal, OrderEntryState, OrderField};
use crate::ui::{charts, popup_area};

pub fn render(frame: &mut Frame, area: Rect, modal: &Modal, app: &mut App) {
    // Register the popup bounding box so the mouse handler can dismiss the modal
    // when the user clicks outside it.
    app.hit_areas.modal_popup_area = Some(match modal {
        Modal::Help => popup_area(area, 50, 70),
        Modal::About => popup_area(area, 50, 60),
        Modal::OrderEntry(_) => popup_area(area, 45, 65),
        Modal::SymbolDetail(_) => popup_area(area, 55, 88),
        Modal::Confirm { .. } => popup_area(area, 40, 25),
        Modal::ConfirmRemoveWatchlist { .. } => popup_area(area, 42, 22),
        Modal::AddSymbol { .. } => popup_area(area, 35, 20),
        Modal::GlobalSearch { .. } => popup_area(area, 35, 20),
        Modal::PositionDetail { .. } => popup_area(area, 60, 90),
    });

    match modal {
        Modal::Help => render_help(frame, area, app),
        Modal::About => render_about(frame, area, app),
        Modal::OrderEntry(state) => render_order_entry(frame, area, state, app),
        Modal::SymbolDetail(symbol) => render_symbol_detail(frame, area, symbol, app),
        Modal::Confirm {
            message,
            action,
            confirmed,
        } => render_confirm(frame, area, message, action, *confirmed, app),
        Modal::ConfirmRemoveWatchlist { symbol, .. } => {
            render_confirm_remove_watchlist(frame, area, symbol, app)
        }
        Modal::AddSymbol { input, .. } => render_add_symbol(frame, area, input, app),
        Modal::GlobalSearch { query } => render_global_search(frame, area, query, app),
        Modal::PositionDetail { symbol } => render_position_detail(frame, area, symbol, app),
    }
}

fn render_help(frame: &mut Frame, area: Rect, app: &App) {
    let popup = popup_area(area, 50, 70);
    frame.render_widget(Clear, popup);

    let c = app.current_theme.colors();

    let block = Block::default()
        .title(" Keyboard Shortcuts ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(c.accent_style());

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let rows = vec![
        ("NAVIGATION", ""),
        ("1/2/3/4 or Tab", "Switch panels"),
        ("j / k  or ↑/↓", "Move cursor"),
        ("g / G", "Top / Bottom"),
        (
            "Enter",
            "Open detail (position: detail view / other: symbol chart)",
        ),
        ("Esc", "Close / Cancel"),
        ("", ""),
        ("ACTIONS", ""),
        ("o", "New order (Watchlist/Orders) / Sell position"),
        ("c", "Copy symbol (Watchlist/Positions) / Cancel order"),
        ("a", "Add symbol to watchlist"),
        ("d", "Remove symbol from watchlist"),
        ("r", "Force refresh"),
        ("/", "Search / filter watchlist"),
        ("s", "Cycle sort column (Positions / Orders)"),
        ("S", "Toggle sort direction ▲/▼ (Positions / Orders)"),
        ("f", "Filter orders by symbol prefix"),
        ("F", "Clear orders symbol filter"),
        ("", ""),
        ("GLOBAL", ""),
        ("T", "Cycle theme (Default → Dark → High-contrast)"),
        ("q / Ctrl-C", "Quit"),
        ("?", "This help screen"),
        ("A", "About this app"),
        ("Ctrl-F / /", "Global symbol search"),
    ];

    let header = Row::new(vec![
        Cell::from("Key").style(c.header_style()),
        Cell::from("Action").style(c.header_style()),
    ]);

    let table_rows: Vec<Row> = rows
        .iter()
        .map(|(k, v)| {
            if v.is_empty() {
                Row::new(vec![
                    Cell::from(*k)
                        .style(Style::default().fg(c.accent).add_modifier(Modifier::BOLD)),
                    Cell::from(""),
                ])
            } else {
                Row::new(vec![Cell::from(*k).style(c.dim_style()), Cell::from(*v)])
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
        .style(c.dim_style());
    frame.render_widget(footer, footer_area);
}

fn render_about(frame: &mut Frame, area: Rect, app: &App) {
    let popup = popup_area(area, 50, 60);
    frame.render_widget(Clear, popup);

    let c = app.current_theme.colors();

    let block = Block::default()
        .title(" About alpaca-trader-rs ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(c.accent_style());

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  alpaca-trader-rs",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  v{}", env!("CARGO_PKG_VERSION")),
                c.accent_style(),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Alpaca Markets TUI trading terminal",
            c.dim_style(),
        )),
        Line::from(Span::styled(
            "  and async REST client library.",
            c.dim_style(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ── Author ─────────────────────────────",
            c.accent_style(),
        )),
        Line::from("  Arunkumar Mourougappane"),
        Line::from(Span::styled("  amouroug.dev@gmail.com", c.dim_style())),
        Line::from(Span::styled(
            "  github.com/arunkumar-mourougappane",
            c.dim_style(),
        )),
        Line::from(Span::styled("  anengineersrant.com", c.dim_style())),
        Line::from(""),
        Line::from(Span::styled(
            "  ── Project ────────────────────────────",
            c.accent_style(),
        )),
        Line::from(Span::styled(
            "  github.com/arunkumar-mourougappane/",
            c.dim_style(),
        )),
        Line::from(Span::styled("    alpaca-trader-rs", c.dim_style())),
        Line::from(Span::styled("  docs.rs/alpaca-trader-rs", c.dim_style())),
        Line::from(""),
        Line::from(Span::styled(
            "  ── License ────────────────────────────",
            c.accent_style(),
        )),
        Line::from(Span::styled(
            format!("  {}", env!("CARGO_PKG_LICENSE")),
            c.dim_style(),
        )),
        Line::from(""),
    ];

    let content_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, content_area);

    let footer_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    let footer = Paragraph::new("  Press any key to close")
        .alignment(Alignment::Center)
        .style(c.dim_style());
    frame.render_widget(footer, footer_area);
}

fn render_order_entry(frame: &mut Frame, area: Rect, state: &OrderEntryState, app: &mut App) {
    let popup = popup_area(area, 45, 65);
    frame.render_widget(Clear, popup);

    let c = app.current_theme.colors();

    let block = Block::default()
        .title(" New Order ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(c.accent_style());

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
            Style::default().fg(c.accent).add_modifier(Modifier::BOLD)
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
            c.dim_style(),
        ),
        chunks[0],
    );

    // Side
    let side_line = Line::from(vec![
        Span::styled("  Side    ", c.dim_style()),
        radio(state.side == crate::app::OrderSide::Buy, "BUY", &c),
        Span::raw("  "),
        radio(state.side == crate::app::OrderSide::Sell, "SELL", &c),
        Span::raw("  "),
        radio(
            state.side == crate::app::OrderSide::SellShort,
            "SELL SHORT",
            &c,
        ),
    ]);
    frame.render_widget(Paragraph::new(side_line), chunks[2]);

    // Type
    let type_line = Line::from(vec![
        Span::styled("  Type    ", c.dim_style()),
        radio(!state.market_order, "LIMIT", &c),
        Span::raw("  "),
        radio(state.market_order, "MARKET", &c),
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
            c.dim_style(),
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
                c.dim_style(),
            ),
            chunks[5],
        );
    } else {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  Price ", c.dim_style()),
                Span::styled("N/A (Market order)", c.dim_style()),
            ])),
            chunks[5],
        );
    }

    // TimeInForce
    let tif_line = Line::from(vec![
        Span::styled("  TIF     ", c.dim_style()),
        radio(!state.gtc_order, "DAY", &c),
        Span::raw("  "),
        radio(state.gtc_order, "GTC", &c),
    ]);
    frame.render_widget(Paragraph::new(tif_line), chunks[6]);

    // Est Total
    let est_total = estimate_total(state);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Est. Total  ", c.dim_style()),
            Span::styled(est_total, c.bold_style()),
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
            Span::styled("  Buying Power  ", c.dim_style()),
            Span::styled(bp, c.bold_style()),
        ])),
        chunks[9],
    );

    // Market-closed warning
    if !market_open && !state.gtc_order {
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                "  ⚠ Market closed — switch to GTC or wait",
                Style::default().fg(c.neutral).add_modifier(Modifier::BOLD),
            )])),
            chunks[11],
        );
    }

    // Submit button — dimmed when market is closed and order is DAY
    let market_closed_day = !market_open && !state.gtc_order;
    let submit_style = if focused(&OrderField::Submit) && !market_closed_day {
        Style::default()
            .fg(c.accent)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else if market_closed_day {
        c.dim_style()
    } else {
        Style::default()
    };
    let buttons = Line::from(vec![
        Span::styled("  [ Submit Order ]", submit_style),
        Span::raw("  "),
        Span::styled("[ Esc: Cancel ]", c.dim_style()),
    ]);
    frame.render_widget(Paragraph::new(buttons), chunks[12]);

    // Hint
    frame.render_widget(
        Paragraph::new("  Tab:Next  ←/→:Toggle  Enter:Advance  Esc:Close").style(c.dim_style()),
        chunks[13],
    );
}

fn render_symbol_detail(frame: &mut Frame, area: Rect, symbol: &str, app: &App) {
    let popup = popup_area(area, 55, 88);
    frame.render_widget(Clear, popup);

    let c = app.current_theme.colors();

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
        .border_style(c.accent_style());

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
        .map(|pct| {
            if pct >= 0.0 {
                c.positive_style()
            } else {
                c.negative_style()
            }
        })
        .unwrap_or_else(|| c.bold_style());

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
            Constraint::Length(5), // line chart
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
            Span::styled("  Price    ", c.dim_style()),
            Span::styled(price_str, value_style),
            Span::raw("   "),
            Span::styled("Change    ", c.dim_style()),
            Span::styled(change_str, value_style),
        ])),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Open     ", c.dim_style()),
            Span::styled(open_str, c.bold_style()),
            Span::raw("   "),
            Span::styled("High      ", c.dim_style()),
            Span::styled(high_str, c.positive_style()),
        ])),
        chunks[2],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Low      ", c.dim_style()),
            Span::styled(low_str, c.negative_style()),
            Span::raw("   "),
            Span::styled("Volume    ", c.dim_style()),
            Span::styled(vol_str, c.bold_style()),
        ])),
        chunks[3],
    );

    // ── Intraday line chart ───────────────────────────────────────────────────
    // When a crosshair is active, replace the static "── Intraday ──" label
    // with a price/time tooltip for the highlighted bar.
    let crosshair = app.symbol_detail_crosshair;
    let intraday_label: Line =
        if let (Some(ci), Some(bars)) = (crosshair, app.intraday_bars.get(symbol)) {
            if let Some(&price_cents) = bars.get(ci) {
                let price = price_cents as f64 / 100.0;
                let time = charts::bar_time_label(ci);
                Line::from(vec![
                    Span::styled("  ", c.dim_style()),
                    Span::styled(time, c.accent_style()),
                    Span::styled("  $", c.dim_style()),
                    Span::styled(format!("{:.2}", price), c.bold_style()),
                    Span::styled("  ←→ to move  Esc to clear", c.dim_style()),
                ])
            } else {
                Line::from(vec![Span::styled("  ── Intraday ──", c.dim_style())])
            }
        } else {
            Line::from(vec![Span::styled("  ── Intraday ──", c.dim_style())])
        };
    frame.render_widget(Paragraph::new(intraday_label), chunks[5]);

    match app.intraday_bars.get(symbol) {
        None => {
            // Command dispatched but response not yet received
            frame.render_widget(Paragraph::new("  Loading…").style(c.dim_style()), chunks[6]);
        }
        Some(bars) if bars.is_empty() => {
            // Fetched but no bars (market closed, pre-market, or error)
            frame.render_widget(
                Paragraph::new("  No intraday data available").style(c.dim_style()),
                chunks[6],
            );
        }
        Some(bars) => {
            let data_points = charts::price_points(bars);
            let n = data_points.len() as f64;
            let [y_min, y_max] = charts::y_bounds(&data_points);
            let line_color = charts::trend_color(&data_points, &c);

            let dataset = Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(line_color))
                .data(&data_points);

            // When a crosshair is active, add a vertical line Dataset at that index.
            let crosshair_pts: Vec<(f64, f64)>;
            let mut datasets = vec![dataset];
            if let Some(ci) = crosshair {
                if ci < bars.len() {
                    let x = ci as f64;
                    crosshair_pts = (0..=16)
                        .map(|j| {
                            let y = y_min + (y_max - y_min) * j as f64 / 16.0;
                            (x, y)
                        })
                        .collect();
                    datasets.push(
                        Dataset::default()
                            .marker(symbols::Marker::Braille)
                            .graph_type(GraphType::Scatter)
                            .style(Style::default().fg(c.accent))
                            .data(&crosshair_pts),
                    );
                }
            }

            let chart = Chart::new(datasets)
                .x_axis(
                    Axis::default()
                        .bounds([0.0, (n - 1.0).max(0.0)])
                        .labels(["09:30", "16:00"]),
                )
                .y_axis(Axis::default().bounds([y_min, y_max]));

            frame.render_widget(chart, chunks[6]);
        }
    }

    // ── Asset flags ──────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Exchange ", c.dim_style()),
            Span::styled(
                asset
                    .map(|a| a.exchange.as_str())
                    .unwrap_or("—")
                    .to_string(),
                c.bold_style(),
            ),
            Span::raw("   "),
            Span::styled("Class     ", c.dim_style()),
            Span::styled(
                asset
                    .map(|a| a.asset_class.as_str())
                    .unwrap_or("—")
                    .to_string(),
                c.bold_style(),
            ),
        ])),
        chunks[8],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Tradable ", c.dim_style()),
            Span::styled(
                flag(asset.map(|a| a.tradable).unwrap_or(false)),
                c.positive_style(),
            ),
            Span::raw("   "),
            Span::styled("Shortable ", c.dim_style()),
            Span::styled(
                flag(asset.map(|a| a.shortable).unwrap_or(false)),
                c.positive_style(),
            ),
        ])),
        chunks[9],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Fractional ", c.dim_style()),
            Span::styled(
                flag(asset.map(|a| a.fractionable).unwrap_or(false)),
                c.positive_style(),
            ),
            Span::raw(" "),
            Span::styled("ETB       ", c.dim_style()),
            Span::styled(
                flag(asset.map(|a| a.easy_to_borrow).unwrap_or(false)),
                c.positive_style(),
            ),
        ])),
        chunks[10],
    );

    // ── Footer ───────────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!("  o:Buy  s:Sell  {}  ←→:Chart  Esc:Close", wl_label),
            c.dim_style(),
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

    let c = app.current_theme.colors();

    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(c.negative_style());

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
        Paragraph::new(format!("  {}", message)).style(c.bold_style()),
        chunks[0],
    );

    let yes_style = if confirmed {
        Style::default()
            .fg(c.positive)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        c.positive_style()
    };
    let no_style = if !confirmed {
        Style::default()
            .fg(c.negative)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        c.negative_style()
    };

    let buttons = Line::from(vec![
        Span::styled("  [ y: Yes ]", yes_style),
        Span::raw("  "),
        Span::styled("[ n: No ]", no_style),
    ]);
    frame.render_widget(Paragraph::new(buttons), chunks[2]);
}

/// Renders the dedicated watchlist-removal confirmation dialog:
///
/// ```text
/// ┌─ Remove from Watchlist ─────────┐
/// │                                  │
/// │  Remove AAPL from watchlist?    │
/// │                                  │
/// │    [y / Enter] Yes  [n / Esc] No │
/// └──────────────────────────────────┘
/// ```
fn render_confirm_remove_watchlist(frame: &mut Frame, area: Rect, symbol: &str, app: &mut App) {
    let popup = popup_area(area, 42, 22);
    frame.render_widget(Clear, popup);

    let c = app.current_theme.colors();

    let block = Block::default()
        .title(" Remove from Watchlist ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(c.negative_style());

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

    // Store button row for mouse hit-testing
    app.hit_areas.modal_confirm_buttons = Some(chunks[2]);

    frame.render_widget(
        Paragraph::new(format!("  Remove {} from watchlist?", symbol)).style(c.bold_style()),
        chunks[0],
    );

    let buttons = Line::from(vec![
        Span::styled("  [y / Enter] Yes", c.positive_style()),
        Span::raw("  "),
        Span::styled("[n / Esc] No", c.negative_style()),
    ]);
    frame.render_widget(Paragraph::new(buttons), chunks[2]);
}

fn render_add_symbol(frame: &mut Frame, area: Rect, input: &str, app: &App) {
    let popup = popup_area(area, 35, 20);
    frame.render_widget(Clear, popup);

    let c = app.current_theme.colors();

    let block = Block::default()
        .title(" Add Symbol ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(c.accent_style());

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
        Paragraph::new("  Enter ticker symbol:").style(c.dim_style()),
        chunks[0],
    );

    let input_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(input.to_string(), c.bold_style()),
        Span::styled("▋", c.accent_style()),
    ]);
    frame.render_widget(Paragraph::new(input_line), chunks[1]);

    frame.render_widget(
        Paragraph::new("  Enter:Add  Esc:Cancel").style(c.dim_style()),
        chunks[2],
    );
}

fn render_global_search(frame: &mut Frame, area: Rect, query: &str, app: &App) {
    let popup = popup_area(area, 35, 20);
    frame.render_widget(Clear, popup);

    let c = app.current_theme.colors();

    let block = Block::default()
        .title(" Global Symbol Search ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(c.accent_style());

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
        Paragraph::new("  Enter ticker symbol:").style(c.dim_style()),
        chunks[0],
    );

    let input_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(query.to_string(), c.bold_style()),
        Span::styled("▋", c.accent_style()),
    ]);
    frame.render_widget(Paragraph::new(input_line), chunks[1]);

    frame.render_widget(
        Paragraph::new("  Enter:Open  Esc:Cancel").style(c.dim_style()),
        chunks[2],
    );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn field_line(label: &str, value: &str, style: Style, dim_style: Style) -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled(format!("  {:<8}", label), dim_style),
        Span::styled(value.to_string(), style),
    ]))
}

fn radio(selected: bool, label: &str, c: &crate::ui::theme::ThemeColors) -> Span<'static> {
    let marker = if selected { "● " } else { "○ " };
    let style = if selected {
        Style::default().fg(c.accent).add_modifier(Modifier::BOLD)
    } else {
        c.dim_style()
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

fn render_position_detail(frame: &mut Frame, area: Rect, symbol: &str, app: &App) {
    let popup = popup_area(area, 60, 90);
    frame.render_widget(Clear, popup);

    let c = app.current_theme.colors();

    let asset = app
        .watchlist
        .as_ref()
        .and_then(|w| w.assets.iter().find(|a| a.symbol == symbol));
    let name = asset.map(|a| a.name.as_str()).unwrap_or(symbol);

    let block = Block::default()
        .title(format!(" {} — {} ", symbol, name))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(c.accent_style());
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    // ── Outer vertical split: chart (top 50%) + detail row (bottom 50%) ───────
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // label
            Constraint::Percentage(50), // intraday chart
            Constraint::Min(0),         // position summary + orders
            Constraint::Length(1),      // footer
        ])
        .split(inner);

    // ── Chart label ──────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "  ── Intraday ──",
            c.dim_style(),
        )])),
        outer[0],
    );

    // ── Intraday chart ────────────────────────────────────────────────────────
    match app.intraday_bars.get(symbol) {
        None => {
            frame.render_widget(Paragraph::new("  Loading…").style(c.dim_style()), outer[1]);
        }
        Some(bars) if bars.is_empty() => {
            frame.render_widget(
                Paragraph::new("  No intraday data available").style(c.dim_style()),
                outer[1],
            );
        }
        Some(bars) => {
            let data_points = charts::price_points(bars);
            let n = data_points.len() as f64;
            let [y_min, y_max] = charts::y_bounds(&data_points);
            let line_color = charts::trend_color(&data_points, &c);

            let dataset = Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(line_color))
                .data(&data_points);

            let chart = Chart::new(vec![dataset])
                .x_axis(
                    Axis::default()
                        .bounds([0.0, (n - 1.0).max(0.0)])
                        .labels(["09:30", "16:00"]),
                )
                .y_axis(Axis::default().bounds([y_min, y_max]));

            frame.render_widget(chart, outer[1]);
        }
    }

    // ── Bottom split: position summary (left) + open orders (right) ──────────
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer[2]);

    // ── Position summary ─────────────────────────────────────────────────────
    let pos = app.positions.iter().find(|p| p.symbol == symbol);
    let summary_lines: Vec<Line> = if let Some(p) = pos {
        let pl: f64 = p.unrealized_pl.parse().unwrap_or(0.0);
        let pl_style = if pl >= 0.0 {
            c.positive_style()
        } else {
            c.negative_style()
        };
        let plpc: f64 = p.unrealized_plpc.parse().unwrap_or(0.0);
        vec![
            Line::from(vec![
                Span::styled("  Qty        ", c.dim_style()),
                Span::styled(p.qty.clone(), c.bold_style()),
            ]),
            Line::from(vec![
                Span::styled("  Avg Cost   ", c.dim_style()),
                Span::styled(format!("${}", p.avg_entry_price), c.bold_style()),
            ]),
            Line::from(vec![
                Span::styled("  Cur Price  ", c.dim_style()),
                Span::styled(format!("${}", p.current_price), c.bold_style()),
            ]),
            Line::from(vec![
                Span::styled("  Mkt Value  ", c.dim_style()),
                Span::styled(format!("${}", p.market_value), c.bold_style()),
            ]),
            Line::from(vec![
                Span::styled("  Unreal P/L ", c.dim_style()),
                Span::styled(format!("${:.2}", pl), pl_style),
            ]),
            Line::from(vec![
                Span::styled("  P/L %      ", c.dim_style()),
                Span::styled(format!("{:+.2}%", plpc * 100.0), pl_style),
            ]),
            Line::from(vec![
                Span::styled("  Side       ", c.dim_style()),
                Span::styled(p.side.clone(), c.bold_style()),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "  No position data",
            c.dim_style(),
        ))]
    };

    let summary_block = Block::default()
        .title(" Position ")
        .borders(Borders::ALL)
        .border_style(c.dim_style());
    let summary_inner = summary_block.inner(bottom[0]);
    frame.render_widget(summary_block, bottom[0]);
    frame.render_widget(Paragraph::new(summary_lines), summary_inner);

    // ── Related open orders ───────────────────────────────────────────────────
    let open_orders: Vec<&crate::types::Order> = app
        .orders
        .iter()
        .filter(|o| {
            o.symbol == symbol
                && matches!(
                    o.status.as_str(),
                    "new" | "pending_new" | "accepted" | "held" | "partially_filled"
                )
        })
        .collect();

    let orders_block = Block::default()
        .title(" Open Orders ")
        .borders(Borders::ALL)
        .border_style(c.dim_style());
    let orders_inner = orders_block.inner(bottom[1]);
    frame.render_widget(orders_block, bottom[1]);

    if open_orders.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("  No open orders", c.dim_style())),
            orders_inner,
        );
    } else {
        let rows: Vec<Row> = open_orders
            .iter()
            .map(|o| {
                let qty = o.qty.as_deref().unwrap_or("—");
                let price = o
                    .limit_price
                    .as_deref()
                    .map(|p| format!("${p}"))
                    .unwrap_or_else(|| "mkt".into());
                Row::new(vec![
                    Cell::from(o.side.clone()),
                    Cell::from(qty.to_string()),
                    Cell::from(price),
                    Cell::from(o.status.clone()),
                ])
            })
            .collect();

        let header = Row::new(vec![
            Cell::from(Span::styled("Side", c.dim_style())),
            Cell::from(Span::styled("Qty", c.dim_style())),
            Cell::from(Span::styled("Price", c.dim_style())),
            Cell::from(Span::styled("Status", c.dim_style())),
        ]);

        let table = Table::new(
            rows,
            [
                Constraint::Length(5),
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Min(6),
            ],
        )
        .header(header);

        frame.render_widget(table, orders_inner);
    }

    // ── Footer ────────────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "  o:Order  Esc:Close",
            c.dim_style(),
        )])),
        outer[3],
    );
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
    fn render_symbol_detail_renders_line_chart_with_bars() {
        let mut app = make_test_app();
        app.intraday_bars
            .insert("AAPL".into(), vec![15000, 15050, 15100, 15080, 15120]);
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        // The line chart renders something other than "Loading" or "No intraday data"
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

    // ── SymbolDetail crosshair rendering ──────────────────────────────────────

    #[test]
    fn render_symbol_detail_footer_shows_arrow_hint() {
        let mut app = make_test_app();
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("←→:Chart"),
            "footer should contain crosshair hint; got:\n{}",
            output
        );
    }

    #[test]
    fn render_symbol_detail_shows_static_intraday_label_without_crosshair() {
        let mut app = make_test_app();
        app.intraday_bars
            .insert("AAPL".into(), vec![15000, 15100, 15200]);
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("Intraday"),
            "should show static Intraday label when no crosshair; got:\n{}",
            output
        );
    }

    #[test]
    fn render_symbol_detail_shows_time_and_price_when_crosshair_active() {
        let mut app = make_test_app();
        // 3 bars: 09:30, 09:31, 09:32; bar at index 1 = $151.00
        app.intraday_bars
            .insert("AAPL".into(), vec![15100, 15100, 15200]);
        app.symbol_detail_crosshair = Some(1);
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("09:31"),
            "tooltip should show time for bar 1 (09:31); got:\n{}",
            output
        );
        assert!(
            output.contains("151.00"),
            "tooltip should show price $151.00 for bar 1; got:\n{}",
            output
        );
    }

    #[test]
    fn render_symbol_detail_crosshair_at_index_zero_shows_market_open() {
        let mut app = make_test_app();
        app.intraday_bars.insert("AAPL".into(), vec![17000, 17100]);
        app.symbol_detail_crosshair = Some(0);
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("09:30"),
            "crosshair at 0 should show 09:30; got:\n{}",
            output
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

    fn render_about_to_string() -> String {
        let app = make_test_app();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_about(frame, area, &app);
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
    fn render_about_shows_app_name() {
        let output = render_about_to_string();
        assert!(
            output.contains("alpaca-trader-rs"),
            "About modal should display the app name"
        );
    }

    #[test]
    fn render_about_shows_version() {
        let output = render_about_to_string();
        let expected = format!("v{}", env!("CARGO_PKG_VERSION"));
        assert!(
            output.contains(&expected),
            "About modal should display the version"
        );
    }

    #[test]
    fn render_about_shows_author() {
        let output = render_about_to_string();
        assert!(
            output.contains("Arunkumar"),
            "About modal should display the author name"
        );
    }

    #[test]
    fn render_about_shows_license() {
        let output = render_about_to_string();
        assert!(
            output.contains("MIT"),
            "About modal should display the license"
        );
    }

    #[test]
    fn render_about_shows_close_hint() {
        let output = render_about_to_string();
        assert!(
            output.contains("Press any key to close"),
            "About modal should display close hint"
        );
    }

    #[test]
    fn render_dispatch_about_modal() {
        // Exercises the `Modal::About => render_about(frame, area)` arm in `render()`
        let mut app = make_test_app();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render(frame, area, &Modal::About, &mut app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let output: String = (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
            .join("\n");
        assert!(
            output.contains("alpaca-trader-rs"),
            "render() with Modal::About should display app name"
        );
    }

    fn render_confirm_remove_watchlist_to_string(symbol: &str) -> String {
        let mut app = make_test_app();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_confirm_remove_watchlist(frame, area, symbol, &mut app);
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
    fn render_confirm_remove_watchlist_shows_title() {
        let output = render_confirm_remove_watchlist_to_string("AAPL");
        assert!(
            output.contains("Remove from Watchlist"),
            "modal title should say 'Remove from Watchlist', got: {output}"
        );
    }

    #[test]
    fn render_confirm_remove_watchlist_shows_symbol() {
        let output = render_confirm_remove_watchlist_to_string("TSLA");
        assert!(
            output.contains("TSLA"),
            "modal should display the symbol being removed, got: {output}"
        );
    }

    #[test]
    fn render_confirm_remove_watchlist_shows_yes_button() {
        let output = render_confirm_remove_watchlist_to_string("AAPL");
        assert!(
            output.contains("Yes"),
            "modal should show Yes button, got: {output}"
        );
    }

    #[test]
    fn render_confirm_remove_watchlist_shows_no_button() {
        let output = render_confirm_remove_watchlist_to_string("AAPL");
        assert!(
            output.contains("No"),
            "modal should show No button, got: {output}"
        );
    }

    #[test]
    fn render_dispatch_confirm_remove_watchlist_modal() {
        // Exercises the Modal::ConfirmRemoveWatchlist arm in render()
        let mut app = make_test_app();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render(
                    frame,
                    area,
                    &Modal::ConfirmRemoveWatchlist {
                        symbol: "NVDA".into(),
                        watchlist_id: "wl-1".into(),
                    },
                    &mut app,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let output: String = (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
            .join("\n");
        assert!(
            output.contains("NVDA"),
            "render() with Modal::ConfirmRemoveWatchlist should display the symbol"
        );
    }

    fn render_global_search_to_string(app: &mut App, query: &str) -> String {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                let modal = Modal::GlobalSearch {
                    query: query.to_string(),
                };
                render(frame, area, &modal, app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
    fn render_global_search_shows_title() {
        let mut app = make_test_app();
        let output = render_global_search_to_string(&mut app, "");
        assert!(
            output.contains("Global Symbol Search"),
            "modal should display 'Global Symbol Search' title"
        );
    }

    #[test]
    fn render_global_search_shows_query_text() {
        let mut app = make_test_app();
        let output = render_global_search_to_string(&mut app, "AAPL");
        assert!(
            output.contains("AAPL"),
            "modal should display the current query"
        );
    }

    #[test]
    fn render_global_search_shows_instructions() {
        let mut app = make_test_app();
        let output = render_global_search_to_string(&mut app, "");
        assert!(
            output.contains("Enter ticker symbol"),
            "modal should display entry prompt"
        );
        assert!(
            output.contains("Enter"),
            "modal footer should mention Enter key"
        );
        assert!(
            output.contains("Esc"),
            "modal footer should mention Esc key"
        );
    }

    // ── render_help ───────────────────────────────────────────────────────────

    fn render_help_to_string() -> String {
        let app = make_test_app();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_help(frame, area, &app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
    fn render_help_shows_title() {
        let output = render_help_to_string();
        assert!(
            output.contains("Keyboard Shortcuts"),
            "help modal should show 'Keyboard Shortcuts' title"
        );
    }

    #[test]
    fn render_help_shows_navigation_section() {
        let output = render_help_to_string();
        assert!(
            output.contains("NAVIGATION"),
            "help modal should show NAVIGATION section"
        );
    }

    #[test]
    fn render_help_shows_actions_section() {
        let output = render_help_to_string();
        assert!(
            output.contains("ACTIONS"),
            "help modal should show ACTIONS section"
        );
    }

    #[test]
    fn render_help_shows_global_section() {
        let output = render_help_to_string();
        assert!(
            output.contains("GLOBAL"),
            "help modal should show GLOBAL section"
        );
    }

    #[test]
    fn render_help_shows_close_hint() {
        let output = render_help_to_string();
        assert!(
            output.contains("Press any key to close"),
            "help modal should show close hint"
        );
    }

    #[test]
    fn render_help_shows_global_search_shortcut() {
        let output = render_help_to_string();
        assert!(
            output.contains("Ctrl-F"),
            "help modal should list the Ctrl-F global search shortcut"
        );
    }

    #[test]
    fn render_dispatch_help_modal() {
        let mut app = make_test_app();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render(frame, area, &Modal::Help, &mut app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let output: String = (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
            .join("\n");
        assert!(
            output.contains("Keyboard Shortcuts"),
            "render() with Modal::Help should show shortcuts title"
        );
    }

    // ── render_confirm ────────────────────────────────────────────────────────

    fn render_confirm_to_string(message: &str, confirmed: bool) -> String {
        let mut app = make_test_app();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_confirm(
                    frame,
                    area,
                    message,
                    &ConfirmAction::CancelOrder("id".into()),
                    confirmed,
                    &mut app,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
    fn render_confirm_shows_title() {
        let output = render_confirm_to_string("Cancel order?", false);
        assert!(
            output.contains("Confirm"),
            "confirm modal should show 'Confirm' title"
        );
    }

    #[test]
    fn render_confirm_shows_message() {
        let output = render_confirm_to_string("Cancel order?", false);
        assert!(
            output.contains("Cancel order"),
            "confirm modal should display the message"
        );
    }

    #[test]
    fn render_confirm_shows_yes_and_no_buttons() {
        let output = render_confirm_to_string("Are you sure?", true);
        assert!(
            output.contains("Yes"),
            "confirm modal should show Yes button"
        );
        assert!(output.contains("No"), "confirm modal should show No button");
    }

    #[test]
    fn render_confirm_not_confirmed_shows_no_highlighted() {
        // confirmed=false means NO is highlighted (reversed style)
        let output = render_confirm_to_string("Are you sure?", false);
        assert!(
            output.contains("No"),
            "No button should be present when confirmed=false"
        );
    }

    #[test]
    fn render_confirm_confirmed_shows_yes_highlighted() {
        // confirmed=true means YES is highlighted (reversed style)
        let output = render_confirm_to_string("Proceed?", true);
        assert!(
            output.contains("Yes"),
            "Yes button should be present when confirmed=true"
        );
    }

    #[test]
    fn render_dispatch_confirm_modal() {
        let mut app = make_test_app();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render(
                    frame,
                    area,
                    &Modal::Confirm {
                        message: "Delete?".into(),
                        action: ConfirmAction::CancelOrder("id".into()),
                        confirmed: false,
                    },
                    &mut app,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let output: String = (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
            .join("\n");
        assert!(
            output.contains("Confirm"),
            "render() with Modal::Confirm should show Confirm title"
        );
    }

    // ── render_add_symbol ─────────────────────────────────────────────────────

    fn render_add_symbol_to_string(input: &str) -> String {
        let app = make_test_app();
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_add_symbol(frame, area, input, &app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
    fn render_add_symbol_shows_title() {
        let output = render_add_symbol_to_string("");
        assert!(
            output.contains("Add Symbol"),
            "add-symbol modal should show 'Add Symbol' title"
        );
    }

    #[test]
    fn render_add_symbol_shows_input() {
        let output = render_add_symbol_to_string("MSFT");
        assert!(
            output.contains("MSFT"),
            "add-symbol modal should display the current input"
        );
    }

    #[test]
    fn render_add_symbol_shows_hints() {
        let output = render_add_symbol_to_string("");
        assert!(
            output.contains("Enter"),
            "add-symbol modal should show Enter hint"
        );
        assert!(
            output.contains("Esc"),
            "add-symbol modal should show Esc hint"
        );
    }

    #[test]
    fn render_dispatch_add_symbol_modal() {
        let mut app = make_test_app();
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render(
                    frame,
                    area,
                    &Modal::AddSymbol {
                        input: "GOOG".into(),
                        watchlist_id: "wl-1".into(),
                    },
                    &mut app,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let output: String = (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
            .join("\n");
        assert!(
            output.contains("GOOG"),
            "render() with Modal::AddSymbol should display the input symbol"
        );
    }

    // ── render_order_entry additional branches ────────────────────────────────

    #[test]
    fn render_order_entry_market_order_shows_na_price() {
        use crate::app::OrderEntryState;
        let mut app = make_test_app();
        let mut state = OrderEntryState::new("AAPL".into());
        state.market_order = true;
        let output = render_order_entry_to_string(&mut app, state);
        assert!(
            output.contains("N/A"),
            "market order should show N/A for price field"
        );
    }

    #[test]
    fn render_order_entry_limit_order_shows_price_field() {
        use crate::app::OrderEntryState;
        let mut app = make_test_app();
        let mut state = OrderEntryState::new("AAPL".into());
        state.market_order = false;
        let output = render_order_entry_to_string(&mut app, state);
        assert!(
            output.contains("Price"),
            "limit order should show Price field"
        );
    }

    #[test]
    fn render_order_entry_market_closed_day_shows_warning() {
        use crate::app::OrderEntryState;
        use crate::types::MarketClock;
        let mut app = make_test_app();
        app.clock = Some(MarketClock {
            is_open: false,
            next_open: "2026-01-01T09:30:00Z".into(),
            next_close: "2026-01-01T16:00:00Z".into(),
            timestamp: "2026-01-01T08:00:00Z".into(),
        });
        let mut state = OrderEntryState::new("AAPL".into());
        state.gtc_order = false; // DAY order + market closed → warning
        let output = render_order_entry_to_string(&mut app, state);
        assert!(
            output.contains("Market closed"),
            "should show market-closed warning when market is closed and order is DAY"
        );
    }

    #[test]
    fn render_order_entry_market_closed_gtc_no_warning() {
        use crate::app::OrderEntryState;
        use crate::types::MarketClock;
        let mut app = make_test_app();
        app.clock = Some(MarketClock {
            is_open: false,
            next_open: "2026-01-01T09:30:00Z".into(),
            next_close: "2026-01-01T16:00:00Z".into(),
            timestamp: "2026-01-01T08:00:00Z".into(),
        });
        let mut state = OrderEntryState::new("AAPL".into());
        state.gtc_order = true; // GTC order → no warning even when market closed
        let output = render_order_entry_to_string(&mut app, state);
        assert!(
            !output.contains("Market closed"),
            "GTC order should not show market-closed warning"
        );
    }

    #[test]
    fn render_order_entry_shows_buying_power_from_account() {
        use crate::app::OrderEntryState;
        use crate::types::AccountInfo;
        let mut app = make_test_app();
        app.account = Some(AccountInfo {
            buying_power: "50000.00".into(),
            ..Default::default()
        });
        let state = OrderEntryState::new("AAPL".into());
        let output = render_order_entry_to_string(&mut app, state);
        assert!(
            output.contains("50000"),
            "order entry should display buying power from account"
        );
    }

    #[test]
    fn render_order_entry_shows_dash_when_no_account() {
        use crate::app::OrderEntryState;
        let mut app = make_test_app();
        app.account = None;
        let state = OrderEntryState::new("TSLA".into());
        let output = render_order_entry_to_string(&mut app, state);
        assert!(
            output.contains("Buying Power"),
            "order entry should show Buying Power label even without account"
        );
    }

    #[test]
    fn render_order_entry_shows_estimated_total_when_filled() {
        use crate::app::OrderEntryState;
        let mut app = make_test_app();
        let mut state = OrderEntryState::new("AAPL".into());
        state.qty_input = "10".into();
        state.price_input = "150.00".into();
        state.market_order = false;
        let output = render_order_entry_to_string(&mut app, state);
        assert!(
            output.contains("1500.00"),
            "order entry should display estimated total (qty × price)"
        );
    }

    #[test]
    fn render_dispatch_order_entry_modal() {
        use crate::app::OrderEntryState;
        let mut app = make_test_app();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                let state = OrderEntryState::new("NVDA".into());
                render(frame, area, &Modal::OrderEntry(state), &mut app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let output: String = (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
            .join("\n");
        assert!(
            output.contains("New Order"),
            "render() with Modal::OrderEntry should show 'New Order' title"
        );
    }

    // ── render_symbol_detail with snapshot data ───────────────────────────────

    #[test]
    fn render_symbol_detail_shows_ohlcv_values_from_snapshot() {
        use crate::types::{Snapshot, SnapshotBar};
        let mut app = make_test_app();
        app.snapshots.insert(
            "AAPL".into(),
            Snapshot {
                daily_bar: Some(SnapshotBar {
                    o: 180.0,
                    h: 195.0,
                    l: 178.0,
                    c: 190.0,
                    v: 1_500_000.0,
                }),
                prev_daily_bar: Some(SnapshotBar {
                    o: 179.0,
                    h: 182.0,
                    l: 177.0,
                    c: 185.0,
                    v: 1_200_000.0,
                }),
                ..Default::default()
            },
        );
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("180"),
            "should display open price from snapshot"
        );
        assert!(
            output.contains("195"),
            "should display high price from snapshot"
        );
        assert!(
            output.contains("178"),
            "should display low price from snapshot"
        );
    }

    #[test]
    fn render_symbol_detail_shows_positive_change_from_snapshot() {
        use crate::types::{Snapshot, SnapshotBar};
        let mut app = make_test_app();
        app.snapshots.insert(
            "AAPL".into(),
            Snapshot {
                daily_bar: Some(SnapshotBar {
                    o: 180.0,
                    h: 195.0,
                    l: 178.0,
                    c: 190.0,
                    v: 1_000_000.0,
                }),
                prev_daily_bar: Some(SnapshotBar {
                    c: 185.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let output = render_symbol_detail_to_string(&mut app, "AAPL");
        // 190/185 → +2.70%
        assert!(
            output.contains("+"),
            "positive change should be shown with + sign"
        );
    }

    #[test]
    fn render_symbol_detail_shows_negative_change_from_snapshot() {
        use crate::types::{Snapshot, SnapshotBar};
        let mut app = make_test_app();
        app.snapshots.insert(
            "MSFT".into(),
            Snapshot {
                daily_bar: Some(SnapshotBar {
                    o: 390.0,
                    h: 395.0,
                    l: 382.0,
                    c: 385.0,
                    v: 900_000.0,
                }),
                prev_daily_bar: Some(SnapshotBar {
                    c: 400.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let output = render_symbol_detail_to_string(&mut app, "MSFT");
        // 385/400 → -3.75%
        assert!(
            output.contains("-"),
            "negative change should be shown with - sign"
        );
    }

    #[test]
    fn render_dispatch_symbol_detail_modal() {
        let mut app = make_test_app();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render(frame, area, &Modal::SymbolDetail("AAPL".into()), &mut app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let output: String = (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
            .join("\n");
        assert!(
            output.contains("AAPL"),
            "render() with Modal::SymbolDetail should display the symbol"
        );
    }

    // ── Helper unit tests ─────────────────────────────────────────────────────

    #[test]
    fn estimate_total_returns_dash_when_qty_zero() {
        use crate::app::OrderEntryState;
        let mut state = OrderEntryState::new("X".into());
        state.qty_input = "0".into();
        state.price_input = "100.0".into();
        assert_eq!(estimate_total(&state), "—");
    }

    #[test]
    fn estimate_total_returns_dash_when_price_zero() {
        use crate::app::OrderEntryState;
        let mut state = OrderEntryState::new("X".into());
        state.qty_input = "5".into();
        state.price_input = "0".into();
        assert_eq!(estimate_total(&state), "—");
    }

    #[test]
    fn estimate_total_computes_correctly() {
        use crate::app::OrderEntryState;
        let mut state = OrderEntryState::new("X".into());
        state.qty_input = "3".into();
        state.price_input = "25.50".into();
        assert_eq!(estimate_total(&state), "$76.50");
    }

    #[test]
    fn estimate_total_returns_dash_when_unparseable() {
        use crate::app::OrderEntryState;
        let mut state = OrderEntryState::new("X".into());
        state.qty_input = "abc".into();
        state.price_input = "100".into();
        assert_eq!(estimate_total(&state), "—");
    }

    #[test]
    fn flag_returns_checkmark_for_true() {
        assert_eq!(flag(true), "✓");
    }

    #[test]
    fn flag_returns_cross_for_false() {
        assert_eq!(flag(false), "✗");
    }

    // ── render_position_detail tests ──────────────────────────────────────────

    fn make_position(symbol: &str) -> crate::types::Position {
        crate::types::Position {
            symbol: symbol.into(),
            qty: "10".into(),
            avg_entry_price: "100.00".into(),
            current_price: "115.00".into(),
            market_value: "1150.00".into(),
            unrealized_pl: "150.00".into(),
            unrealized_plpc: "0.15".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }
    }

    fn render_position_detail_to_string(app: &mut App, symbol: &str) -> String {
        let backend = TestBackend::new(160, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_position_detail(frame, area, symbol, app);
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
    fn render_position_detail_shows_symbol_in_title() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        let output = render_position_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("AAPL"),
            "expected AAPL in title, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_shows_asset_name_when_available() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        app.watchlist = Some(make_watchlist(&["AAPL"]));
        let output = render_position_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("AAPL Corp"),
            "expected asset name in title, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_shows_loading_when_no_bars() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        // no entry in intraday_bars → "Loading…"
        let output = render_position_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("Loading"),
            "expected Loading, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_shows_no_data_when_bars_empty() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        app.intraday_bars.insert("AAPL".into(), vec![]);
        let output = render_position_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("No intraday data"),
            "expected no-data message, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_renders_chart_with_bars() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        app.intraday_bars
            .insert("AAPL".into(), vec![15000, 15100, 15050]);
        let output = render_position_detail_to_string(&mut app, "AAPL");
        // chart x-axis labels
        assert!(
            output.contains("09:30") || output.contains("16:00"),
            "expected chart time labels, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_shows_position_summary() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        let output = render_position_detail_to_string(&mut app, "AAPL");
        assert!(output.contains("Qty"), "expected Qty label, got: {output}");
        assert!(
            output.contains("Avg Cost"),
            "expected Avg Cost label, got: {output}"
        );
        assert!(
            output.contains("Mkt Value"),
            "expected Mkt Value label, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_shows_no_position_data_when_missing() {
        // Symbol has no matching entry in app.positions
        let mut app = make_test_app();
        let output = render_position_detail_to_string(&mut app, "NVDA");
        assert!(
            output.contains("No position data"),
            "expected no-position message, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_shows_no_open_orders_when_empty() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        let output = render_position_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("No open orders"),
            "expected no-orders message, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_shows_open_orders_table() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        app.orders.push(crate::types::Order {
            id: "ord-1".into(),
            symbol: "AAPL".into(),
            side: "buy".into(),
            qty: Some("5".into()),
            notional: None,
            order_type: "limit".into(),
            limit_price: Some("110.00".into()),
            status: "new".into(),
            submitted_at: None,
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        let output = render_position_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("buy"),
            "expected order side 'buy', got: {output}"
        );
        assert!(
            output.contains("Side"),
            "expected Side column header, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_filters_non_open_orders() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        // filled orders should NOT appear in the open-orders pane
        app.orders.push(crate::types::Order {
            id: "ord-filled".into(),
            symbol: "AAPL".into(),
            side: "buy".into(),
            qty: Some("5".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: "filled".into(),
            submitted_at: None,
            filled_at: None,
            filled_qty: "5".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        let output = render_position_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("No open orders"),
            "filled orders should not appear in open-orders pane, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_shows_footer_actions() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        let output = render_position_detail_to_string(&mut app, "AAPL");
        assert!(output.contains("o:Order"), "expected footer, got: {output}");
        assert!(
            output.contains("Esc:Close"),
            "expected Esc:Close in footer, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_negative_pl_renders_value() {
        let mut app = make_test_app();
        app.positions.push(crate::types::Position {
            symbol: "AAPL".into(),
            qty: "10".into(),
            avg_entry_price: "150.00".into(),
            current_price: "130.00".into(),
            market_value: "1300.00".into(),
            unrealized_pl: "-200.00".into(),
            unrealized_plpc: "-0.133".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        });
        let output = render_position_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("-200.00"),
            "expected negative P/L value in output, got: {output}"
        );
    }

    #[test]
    fn render_position_detail_market_order_shows_mkt() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        app.orders.push(crate::types::Order {
            id: "ord-mkt".into(),
            symbol: "AAPL".into(),
            side: "buy".into(),
            qty: Some("3".into()),
            notional: None,
            order_type: "market".into(),
            limit_price: None,
            status: "new".into(),
            submitted_at: None,
            filled_at: None,
            filled_qty: "0".into(),
            filled_avg_price: None,
            time_in_force: "day".into(),
        });
        let output = render_position_detail_to_string(&mut app, "AAPL");
        assert!(
            output.contains("mkt"),
            "expected 'mkt' for market order with no limit price, got: {output}"
        );
    }

    #[test]
    fn render_dispatch_position_detail_modal() {
        let mut app = make_test_app();
        app.positions.push(make_position("TSLA"));
        let backend = TestBackend::new(160, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render(
                    frame,
                    area,
                    &Modal::PositionDetail {
                        symbol: "TSLA".into(),
                    },
                    &mut app,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let output: String = (0..buffer.area().height as usize)
            .map(|row| {
                (0..buffer.area().width as usize)
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
            .join("\n");
        assert!(
            output.contains("TSLA"),
            "render() with Modal::PositionDetail should display the symbol"
        );
        assert!(
            app.hit_areas.modal_popup_area.is_some(),
            "render() should set modal_popup_area for PositionDetail"
        );
    }
}
