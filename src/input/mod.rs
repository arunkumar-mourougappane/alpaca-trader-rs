pub(crate) mod modal;
pub(crate) mod mouse;
pub(crate) mod orders;
pub(crate) mod positions;
pub(crate) mod search;
pub(crate) mod validation;
pub(crate) mod watchlist;

pub(crate) use modal::handle_modal_key;
pub(crate) use mouse::handle_mouse;
pub(crate) use orders::handle_orders_key;
pub(crate) use positions::handle_positions_key;
pub(crate) use search::handle_search_key;
pub(crate) use validation::validate;
pub(crate) use watchlist::handle_watchlist_key;

use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use ratatui::widgets::{ListState, TableState};
use tokio::sync::mpsc::error::TrySendError;

use crate::app::{App, StatusMessage};
use crate::commands::Command;

/// Timeout window for the `gg` (jump-to-top) vim key sequence.
pub(crate) const GG_TIMEOUT: Duration = Duration::from_millis(500);

/// Abstraction over ratatui selection states so [`handle_nav_key`] can work
/// with both [`ListState`] and [`TableState`].
pub(crate) trait SelectionState {
    fn selected(&self) -> Option<usize>;
    fn select(&mut self, index: Option<usize>);
}

impl SelectionState for ListState {
    fn selected(&self) -> Option<usize> {
        ListState::selected(self)
    }
    fn select(&mut self, index: Option<usize>) {
        ListState::select(self, index);
    }
}

impl SelectionState for TableState {
    fn selected(&self) -> Option<usize> {
        TableState::selected(self)
    }
    fn select(&mut self, index: Option<usize>) {
        TableState::select(self, index);
    }
}

/// Handle vim-style navigation keys (`j`/`k`/`Up`/`Down`/`g`/`G`) for any list or table.
///
/// Mutates `state` (any [`SelectionState`] such as [`ListState`] or [`TableState`]) and
/// `pending_g` (the timestamp of the last `g` press used for `gg` detection).  Any
/// non-navigation key resets `pending_g` so the `gg` sequence is cancelled.
pub(crate) fn handle_nav_key(
    key: KeyCode,
    len: usize,
    state: &mut impl SelectionState,
    pending_g: &mut Option<Instant>,
) {
    // Any key except a fresh 'g' press cancels the gg sequence.
    let was_pending = *pending_g;
    *pending_g = None;
    match key {
        KeyCode::Char('j') | KeyCode::Down if len > 0 => {
            let i = state.selected().unwrap_or(0);
            state.select(Some((i + 1).min(len - 1)));
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let i = state.selected().unwrap_or(0);
            state.select(Some(i.saturating_sub(1)));
        }
        KeyCode::Char('g') => {
            if was_pending
                .map(|t| t.elapsed() < GG_TIMEOUT)
                .unwrap_or(false)
            {
                state.select(Some(0));
            } else {
                *pending_g = Some(Instant::now());
            }
        }
        KeyCode::Char('G') if len > 0 => {
            state.select(Some(len - 1));
        }
        _ => {}
    }
}

/// Send a command on the command channel and set the appropriate status message.
///
/// - Success → `success_msg` (transient, auto-dismissed after the prefs-configured TTL)
/// - Channel full → "System busy — please retry" (transient)
/// - Channel closed → "Command handler stopped — restart app" (persistent error)
pub(crate) fn send_command(app: &mut App, cmd: Command, success_msg: impl Into<String>) {
    match app.command_tx.try_send(cmd) {
        Ok(()) => app.push_transient_status(success_msg),
        Err(TrySendError::Full(_)) => {
            app.push_transient_status("System busy — please retry");
        }
        Err(TrySendError::Closed(_)) => {
            tracing::error!("command channel closed; command handler has stopped");
            app.push_status(StatusMessage::persistent(
                "Command handler stopped — restart app",
            ));
        }
    }
}

/// Simulate pressing `Enter` on the active tab.
///
/// Used by the mouse double-click handler to open the detail modal for the
/// currently selected row without duplicating the per-tab Enter logic.
pub(crate) fn handle_key_as_enter(app: &mut App) {
    use crossterm::event::{KeyEvent, KeyModifiers};
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    match app.active_tab {
        crate::app::Tab::Watchlist => watchlist::handle_watchlist_key(app, enter),
        crate::app::Tab::Positions => positions::handle_positions_key(app, enter),
        crate::app::Tab::Orders => orders::handle_orders_key(app, enter),
        crate::app::Tab::Account => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::app::test_helpers::make_test_app;
    use crate::app::Tab;

    #[test]
    fn handle_key_as_enter_orders_tab_is_noop() {
        let mut app = make_test_app();
        app.active_tab = Tab::Orders;
        // Enter on Orders with no selection should not crash and not open a modal.
        super::handle_key_as_enter(&mut app);
        assert!(app.modal.is_none());
    }

    #[test]
    fn handle_key_as_enter_account_tab_is_noop() {
        let mut app = make_test_app();
        app.active_tab = Tab::Account;
        // Account tab has no Enter action; should be a no-op.
        super::handle_key_as_enter(&mut app);
        assert!(app.modal.is_none());
    }
}
