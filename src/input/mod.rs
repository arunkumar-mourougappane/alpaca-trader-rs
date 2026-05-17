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

use tokio::sync::mpsc::error::TrySendError;

use crate::app::{App, StatusMessage};
use crate::commands::Command;

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
            app.status_msg = StatusMessage::persistent("Command handler stopped — restart app");
        }
    }
}
