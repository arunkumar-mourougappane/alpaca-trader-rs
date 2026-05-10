use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table},
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
    let popup = popup_area(area, 45, 60);
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
            Constraint::Length(1), // Symbol
            Constraint::Length(1), // blank
            Constraint::Length(1), // Side
            Constraint::Length(1), // Type
            Constraint::Length(1), // Qty
            Constraint::Length(1), // Price
            Constraint::Length(1), // blank
            Constraint::Length(1), // Est Total
            Constraint::Length(1), // Buying Power
            Constraint::Length(1), // blank
            Constraint::Length(1), // Submit / Cancel
            Constraint::Length(1), // hint
        ])
        .split(inner);

    // Populate hit areas for mouse click handling
    app.hit_areas.modal_fields = vec![
        (OrderField::Symbol, chunks[0]),
        (OrderField::Side, chunks[2]),
        (OrderField::OrderType, chunks[3]),
        (OrderField::Qty, chunks[4]),
        (OrderField::Price, chunks[5]),
    ];
    app.hit_areas.modal_submit = Some(chunks[10]);

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
        radio(state.side_buy, "BUY"),
        Span::raw("  "),
        radio(!state.side_buy, "SELL"),
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

    // Est Total
    let est_total = estimate_total(state);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Est. Total  ", Style::default().fg(theme::DIM)),
            Span::styled(est_total, theme::style_bold()),
        ])),
        chunks[7],
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
        chunks[8],
    );

    // Buttons
    let submit_style = if focused(&OrderField::Submit) {
        Style::default()
            .fg(theme::BRAND_CYAN)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default()
    };
    let buttons = Line::from(vec![
        Span::styled("  [ Submit Order ]", submit_style),
        Span::raw("  "),
        Span::styled("[ Esc: Cancel ]", Style::default().fg(theme::DIM)),
    ]);
    frame.render_widget(Paragraph::new(buttons), chunks[10]);

    // Hint
    frame.render_widget(
        Paragraph::new("  Tab:Next  ←/→:Toggle  Enter:Advance  Esc:Close")
            .style(Style::default().fg(theme::DIM)),
        chunks[11],
    );
}

fn render_symbol_detail(frame: &mut Frame, area: Rect, symbol: &str, app: &App) {
    let popup = popup_area(area, 42, 55);
    frame.render_widget(Clear, popup);

    let name = app
        .watchlist
        .as_ref()
        .and_then(|w| w.assets.iter().find(|a| a.symbol == symbol))
        .map(|a| a.name.as_str())
        .unwrap_or(symbol);

    let block = Block::default()
        .title(format!(" {} — {} ", symbol, name))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme::BRAND_CYAN));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let quote = app.quotes.get(symbol);
    let ask = quote
        .and_then(|q| q.ap)
        .map(|p| format!("${:.2}", p))
        .unwrap_or_else(|| "—".into());
    let bid = quote
        .and_then(|q| q.bp)
        .map(|p| format!("${:.2}", p))
        .unwrap_or_else(|| "—".into());

    let asset = app
        .watchlist
        .as_ref()
        .and_then(|w| w.assets.iter().find(|a| a.symbol == symbol));

    let lines = vec![
        Line::from(vec![
            Span::styled("  Ask Price  ", Style::default().fg(theme::DIM)),
            Span::styled(ask, theme::style_bold()),
        ]),
        Line::from(vec![
            Span::styled("  Bid Price  ", Style::default().fg(theme::DIM)),
            Span::styled(bid, theme::style_bold()),
        ]),
        Line::from(vec![]),
        Line::from(vec![
            Span::styled("  Exchange   ", Style::default().fg(theme::DIM)),
            Span::styled(
                asset
                    .map(|a| a.exchange.as_str())
                    .unwrap_or("—")
                    .to_string(),
                theme::style_bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Class      ", Style::default().fg(theme::DIM)),
            Span::styled(
                asset
                    .map(|a| a.asset_class.as_str())
                    .unwrap_or("—")
                    .to_string(),
                theme::style_bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Tradable   ", Style::default().fg(theme::DIM)),
            Span::styled(
                flag(asset.map(|a| a.tradable).unwrap_or(false)),
                Style::default().fg(theme::GREEN),
            ),
            Span::styled("  Shortable  ", Style::default().fg(theme::DIM)),
            Span::styled(
                flag(asset.map(|a| a.shortable).unwrap_or(false)),
                Style::default().fg(theme::GREEN),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Fractional ", Style::default().fg(theme::DIM)),
            Span::styled(
                flag(asset.map(|a| a.fractionable).unwrap_or(false)),
                Style::default().fg(theme::GREEN),
            ),
            Span::styled("  ETB        ", Style::default().fg(theme::DIM)),
            Span::styled(
                flag(asset.map(|a| a.easy_to_borrow).unwrap_or(false)),
                Style::default().fg(theme::GREEN),
            ),
        ]),
        Line::from(vec![]),
        Line::from(vec![Span::styled(
            "  o:Buy  s:Sell  Esc:Close",
            Style::default().fg(theme::DIM),
        )]),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
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
