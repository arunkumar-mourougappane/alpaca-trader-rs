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

pub fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.current_theme.colors();
    let env_label = app.config.env_label();
    let env_color = if env_label == "PAPER" {
        c.accent
    } else {
        c.negative
    };

    let (market_status, market_color) = app
        .clock
        .as_ref()
        .map(|cl| {
            let state = cl.market_state();
            let color = match &state {
                MarketState::Open => c.positive,
                MarketState::PreMarket => c.neutral,
                MarketState::AfterHours => Color::Magenta,
                MarketState::Closed => c.dim,
            };
            (state.as_str(), color)
        })
        .unwrap_or(("—", c.dim));

    let now = Local::now().format("%H:%M:%S ET  %Y-%m-%d").to_string();

    // Right-side indicator: spinner while fetching, or "Updated HH:MM:SS" when idle.
    let fetch_indicator = if app.pending_requests > 0 {
        format!("  {} Fetching…", app.spinner_frame())
    } else if let Some(updated_at) = app.last_updated {
        format!("  Updated {}", updated_at.format("%H:%M:%S"))
    } else {
        String::new()
    };

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
        Span::styled("Market: ", c.dim_style()),
        Span::styled(
            market_status,
            Style::default()
                .fg(market_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("   {}", now), c.dim_style()),
        Span::styled(fetch_indicator, c.dim_style()),
    ];

    if app.config.dry_run {
        spans.insert(
            1,
            Span::styled(
                " [DRY-RUN]",
                Style::default().fg(c.neutral).add_modifier(Modifier::BOLD),
            ),
        );
    }

    if !app.market_stream_ok || !app.account_stream_ok {
        let which = match (app.market_stream_ok, app.account_stream_ok) {
            (false, false) => " ⚠ STREAM",
            (false, true) => " ⚠ MARKET",
            (true, false) => " ⚠ ACCOUNT",
            _ => unreachable!(),
        };
        spans.push(Span::styled(
            which,
            Style::default().fg(c.neutral).add_modifier(Modifier::BOLD),
        ));
    }

    let line = Line::from(spans);

    let paragraph = Paragraph::new(line)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}

pub fn render_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.current_theme.colors();
    let titles = vec!["1:Account", "2:Watchlist", "3:Positions", "4:Orders"];
    let tabs = Tabs::new(titles)
        .select(app.active_tab.index())
        .highlight_style(
            Style::default()
                .fg(c.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider("|");
    frame.render_widget(tabs, area);
}

pub fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.current_theme.colors();
    let panel_hints = match app.active_tab {
        Tab::Account => " r:Refresh  T:Theme  A:About  ?:Help  q:Quit",
        Tab::Watchlist => {
            " j/k:Navigate  Enter:Detail  o:Order  a:Add  d:Remove  c:Copy  /:Search  T:Theme  A:About  ?:Help  q:Quit"
        }
        Tab::Positions => {
            " j/k:Navigate  Enter:Detail  o:Order  c:Copy  s:Sort  S:SortDir  T:Theme  A:About  ?:Help  q:Quit"
        }
        Tab::Orders => " j/k:Navigate  o:New  c:Cancel  f:Filter  F:ClearFilter  s:Sort  S:SortDir  1-3:Filter  T:Theme  A:About  ?:Help  q:Quit",
    };

    let status = if app.current_status_text().is_empty() {
        panel_hints.to_string()
    } else {
        format!("  {}  │{}", app.current_status_text(), panel_hints)
    };

    let para = Paragraph::new(status).style(c.dim_style());
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

    fn render_header_to_string(app: &App) -> String {
        let backend = TestBackend::new(120, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_header(frame, frame.area(), app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let width = buffer.area().width as usize;
        let height = buffer.area().height as usize;
        let mut out = String::with_capacity(width * height);
        for row in 0..height {
            for col in 0..width {
                let sym = buffer
                    .cell(ratatui::layout::Position {
                        x: col as u16,
                        y: row as u16,
                    })
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default();
                out.push_str(&sym);
            }
        }
        out
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

    #[test]
    fn header_shows_dry_run_badge_when_enabled() {
        let mut app = make_test_app();
        app.config.dry_run = true;
        let output = render_header_to_string(&app);
        assert!(
            output.contains("[DRY-RUN]"),
            "header should show [DRY-RUN] badge when dry_run is true; got: {output:?}"
        );
    }

    #[test]
    fn header_hides_dry_run_badge_when_disabled() {
        let app = make_test_app(); // dry_run: false by default
        let output = render_header_to_string(&app);
        assert!(
            !output.contains("[DRY-RUN]"),
            "header must not show [DRY-RUN] badge when dry_run is false"
        );
    }

    #[test]
    fn status_bar_empty_queue_shows_only_hints() {
        let mut app = make_test_app();
        app.active_tab = Tab::Account;
        // queue starts empty → only hints should appear
        let output = render_status_to_string(&app);
        assert!(output.contains("q:Quit"), "should show hints");
        assert!(
            !output.contains("│"),
            "should not show separator when no status message"
        );
    }

    #[test]
    fn status_bar_with_queue_message_shows_message_and_separator() {
        use crate::app::StatusMessage;
        let mut app = make_test_app();
        app.active_tab = Tab::Account;
        app.push_status(StatusMessage::persistent("Refreshing…"));
        let output = render_status_to_string(&app);
        assert!(output.contains("Refreshing…"), "should show status message");
        assert!(
            output.contains("│"),
            "should show separator between message and hints"
        );
    }

    #[test]
    fn status_bar_shows_front_of_queue() {
        use crate::app::StatusMessage;
        let mut app = make_test_app();
        app.push_status(StatusMessage::persistent("First"));
        app.push_status(StatusMessage::persistent("Second"));
        let output = render_status_to_string(&app);
        assert!(
            output.contains("First"),
            "should show first (front) message"
        );
        assert!(
            !output.contains("Second"),
            "should not show queued second message"
        );
    }

    #[test]
    fn header_shows_fetching_spinner_when_pending() {
        let mut app = make_test_app();
        app.pending_requests = 1;
        let output = render_header_to_string(&app);
        assert!(
            output.contains("Fetching"),
            "header should show 'Fetching…' while requests are in-flight; got: {output:?}"
        );
    }

    #[test]
    fn header_shows_updated_time_when_idle_with_last_updated() {
        let mut app = make_test_app();
        app.pending_requests = 0;
        app.last_updated = Some(chrono::Local::now());
        let output = render_header_to_string(&app);
        assert!(
            output.contains("Updated"),
            "header should show 'Updated HH:MM:SS' when idle with last_updated set; got: {output:?}"
        );
    }

    #[test]
    fn header_shows_no_fetch_indicator_when_idle_and_no_last_updated() {
        let app = make_test_app();
        assert_eq!(app.pending_requests, 0);
        assert!(app.last_updated.is_none());
        let output = render_header_to_string(&app);
        assert!(
            !output.contains("Fetching"),
            "header must not show spinner when idle"
        );
        assert!(
            !output.contains("Updated"),
            "header must not show 'Updated' when last_updated is None"
        );
    }
}
