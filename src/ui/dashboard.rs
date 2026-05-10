use chrono::Local;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use crate::app::{App, Tab};
use crate::ui::theme;

pub fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let env_label = app.config.env_label();
    let env_color = if env_label == "PAPER" {
        theme::BRAND_CYAN
    } else {
        theme::BRAND_RED
    };

    let market_status = app
        .clock
        .as_ref()
        .map(|c| if c.is_open { "OPEN" } else { "CLOSED" })
        .unwrap_or("—");

    let now = Local::now().format("%H:%M:%S ET  %Y-%m-%d").to_string();

    let line = Line::from(vec![
        Span::styled(
            format!(" [{}] ", env_label),
            Style::default().fg(env_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "alpaca-trader-rs",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(
            format!("Market: {}   {}", market_status, now),
            Style::default().fg(theme::DIM),
        ),
    ]);

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
        Tab::Account => " r:Refresh  ?:Help  q:Quit",
        Tab::Watchlist => {
            " j/k:Navigate  Enter:Detail  o:Order  a:Add  d:Remove  /:Search  ?:Help  q:Quit"
        }
        Tab::Positions => " j/k:Navigate  Enter:Detail  o:Close  ?:Help  q:Quit",
        Tab::Orders => " j/k:Navigate  o:New  c:Cancel  1-3:Filter  ?:Help  q:Quit",
    };

    let status = if app.status_msg.is_empty() {
        panel_hints.to_string()
    } else {
        format!("  {}  │{}", app.status_msg, panel_hints)
    };

    let para = Paragraph::new(status).style(Style::default().fg(theme::DIM));
    frame.render_widget(para, area);
}
