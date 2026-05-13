use chrono::Local;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use crate::app::{App, Tab};
use crate::types::MarketState;
use crate::ui::theme;

pub fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let env_label = app.config.env_label();
    let env_color = if env_label == "PAPER" {
        theme::BRAND_CYAN
    } else {
        theme::BRAND_RED
    };

    let (market_status, market_color) = app
        .clock
        .as_ref()
        .map(|c| {
            let state = c.market_state();
            let color = match &state {
                MarketState::Open => theme::GREEN,
                MarketState::PreMarket => theme::YELLOW,
                MarketState::AfterHours => Color::Magenta,
                MarketState::Closed => theme::DIM,
            };
            (state.as_str(), color)
        })
        .unwrap_or(("—", theme::DIM));

    let now = Local::now().format("%H:%M:%S ET  %Y-%m-%d").to_string();

    let mut spans = vec![
        Span::styled(
            format!(" [{}] ", env_label),
            Style::default().fg(env_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "alpaca-trader-rs",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled("Market: ", Style::default().fg(theme::DIM)),
        Span::styled(
            market_status,
            Style::default()
                .fg(market_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("   {}", now), Style::default().fg(theme::DIM)),
    ];

    if !app.market_stream_ok || !app.account_stream_ok {
        let which = match (app.market_stream_ok, app.account_stream_ok) {
            (false, false) => " ⚠ STREAM",
            (false, true) => " ⚠ MARKET",
            (true, false) => " ⚠ ACCOUNT",
            _ => unreachable!(),
        };
        spans.push(Span::styled(
            which,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let line = Line::from(spans);

    let paragraph = Paragraph::new(line)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}

pub fn render_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles = vec!["1:Account", "2:Watchlist", "3:Positions", "4:Orders"];
    let tabs = Tabs::new(titles)
        .select(app.active_tab.index())
        .highlight_style(
            Style::default()
                .fg(theme::BRAND_CYAN)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider("|");
    frame.render_widget(tabs, area);
}

pub fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    let panel_hints = match app.active_tab {
        Tab::Account => " r:Refresh  A:About  ?:Help  q:Quit",
        Tab::Watchlist => {
            " j/k:Navigate  Enter:Detail  o:Order  a:Add  d:Remove  /:Search  A:About  ?:Help  q:Quit"
        }
        Tab::Positions => " j/k:Navigate  Enter:Detail  o:Close  s:Short  A:About  ?:Help  q:Quit",
        Tab::Orders => " j/k:Navigate  o:New  c:Cancel  1-3:Filter  A:About  ?:Help  q:Quit",
    };

    let status = if app.status_msg.is_empty() {
        panel_hints.to_string()
    } else {
        format!("  {}  │{}", app.status_msg.text, panel_hints)
    };

    let para = Paragraph::new(status).style(Style::default().fg(theme::DIM));
    frame.render_widget(para, area);
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, Terminal};

    use super::*;
    use crate::app::test_helpers::make_test_app;

    fn render_status_to_string(app: &App) -> String {
        let backend = TestBackend::new(120, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_status(frame, frame.area(), app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area().width as usize)
            .map(|col| {
                buffer
                    .cell(ratatui::layout::Position {
                        x: col as u16,
                        y: 0,
                    })
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect()
    }

    #[test]
    fn status_bar_account_tab_shows_about_hint() {
        let mut app = make_test_app();
        app.active_tab = Tab::Account;
        let output = render_status_to_string(&app);
        assert!(
            output.contains("A:About"),
            "Account status bar should show A:About"
        );
    }

    #[test]
    fn status_bar_watchlist_tab_shows_about_hint() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        let output = render_status_to_string(&app);
        assert!(
            output.contains("A:About"),
            "Watchlist status bar should show A:About"
        );
    }

    #[test]
    fn status_bar_positions_tab_shows_about_hint() {
        let mut app = make_test_app();
        app.active_tab = Tab::Positions;
        let output = render_status_to_string(&app);
        assert!(
            output.contains("A:About"),
            "Positions status bar should show A:About"
        );
    }

    #[test]
    fn status_bar_orders_tab_shows_about_hint() {
        let mut app = make_test_app();
        app.active_tab = Tab::Orders;
        let output = render_status_to_string(&app);
        assert!(
            output.contains("A:About"),
            "Orders status bar should show A:About"
        );
    }
}
