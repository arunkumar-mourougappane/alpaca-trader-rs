pub mod account;
pub mod charts;
pub mod dashboard;
pub mod formatting;
pub mod modals;
pub mod orders;
pub mod positions;
pub mod theme;
pub mod watchlist;

#[cfg(test)]
pub mod test_helpers;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

use crate::app::{App, HitAreas, Tab};

pub fn render(frame: &mut Frame, app: &mut App) {
    // Reset hit areas at the start of each frame so stale rects are never used.
    app.hit_areas = HitAreas::default();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(1), // tab bar
            Constraint::Min(0),    // content
            Constraint::Length(1), // status bar
        ])
        .split(frame.area());

    app.hit_areas.tab_bar = chunks[1];

    dashboard::render_header(frame, chunks[0], app);
    dashboard::render_tabs(frame, chunks[1], app);

    match app.active_tab {
        Tab::Account => account::render(frame, chunks[2], app),
        Tab::Watchlist => {
            // Data rows begin after top border (1) + header row (1)
            app.hit_areas.list_data_start_y = chunks[2].y + 2;
            watchlist::render(frame, chunks[2], app);
        }
        Tab::Positions => {
            app.hit_areas.list_data_start_y = chunks[2].y + 2;
            positions::render(frame, chunks[2], app);
        }
        Tab::Orders => {
            // Orders has a 1-row sub-tab bar, then the table block.
            // Data rows begin after sub-tab bar (1) + top border (1) + header row (1).
            app.hit_areas.list_data_start_y = chunks[2].y + 3;
            orders::render(frame, chunks[2], app);
        }
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

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, Terminal};

    use crate::app::test_helpers::make_test_app;
    use crate::app::{Modal, Tab};

    fn render_app_to_string(app: &mut crate::app::App) -> String {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| super::render(frame, app)).unwrap();
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
    fn render_account_tab_does_not_panic() {
        let mut app = make_test_app();
        app.active_tab = Tab::Account;
        render_app_to_string(&mut app);
    }

    #[test]
    fn render_watchlist_tab_does_not_panic() {
        let mut app = make_test_app();
        app.active_tab = Tab::Watchlist;
        render_app_to_string(&mut app);
    }

    #[test]
    fn render_positions_tab_does_not_panic() {
        let mut app = make_test_app();
        app.active_tab = Tab::Positions;
        render_app_to_string(&mut app);
    }

    #[test]
    fn render_orders_tab_does_not_panic() {
        let mut app = make_test_app();
        app.active_tab = Tab::Orders;
        render_app_to_string(&mut app);
    }

    #[test]
    fn render_with_help_modal_does_not_panic() {
        let mut app = make_test_app();
        app.modal = Some(Modal::Help);
        render_app_to_string(&mut app);
    }

    #[test]
    fn popup_area_is_centered_within_parent() {
        use ratatui::layout::Rect;
        let area = Rect::new(0, 0, 100, 50);
        let result = super::popup_area(area, 60, 40);
        assert_eq!(result.width, 60);
        assert_eq!(result.height, 20);
        assert_eq!(result.x, 20);
        assert_eq!(result.y, 15);
    }
}
