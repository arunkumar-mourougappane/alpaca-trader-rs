pub mod account;
pub mod dashboard;
pub mod modals;
pub mod orders;
pub mod positions;
pub mod theme;
pub mod watchlist;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

use crate::app::{App, Tab};

pub fn render(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(1), // tab bar
            Constraint::Min(0),    // content
            Constraint::Length(1), // status bar
        ])
        .split(frame.area());

    dashboard::render_header(frame, chunks[0], app);
    dashboard::render_tabs(frame, chunks[1], app);

    match app.active_tab {
        Tab::Account => account::render(frame, chunks[2], app),
        Tab::Watchlist => watchlist::render(frame, chunks[2], app),
        Tab::Positions => positions::render(frame, chunks[2], app),
        Tab::Orders => orders::render(frame, chunks[2], app),
    }

    dashboard::render_status(frame, chunks[3], app);

    // Modals rendered last (on top)
    if let Some(modal) = app.modal.clone() {
        modals::render(frame, frame.area(), &modal, app);
    }
}

/// Returns a centered rectangle with the given percentage dimensions.
pub fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
