use crossterm::event::KeyCode;

use crate::app::App;

pub(crate) fn handle_search_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.searching = false;
        }
        KeyCode::Backspace => {
            app.search_query.pop();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.watchlist_state.select(Some(0));
        }
        _ => {}
    }
}
