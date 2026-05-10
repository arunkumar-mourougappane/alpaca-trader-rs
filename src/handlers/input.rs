use crossterm::event::{Event as CEvent, EventStream};
use futures::StreamExt;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

use crate::events::Event as AppEvent;

pub async fn run(tx: Sender<AppEvent>, cancel: CancellationToken) {
    let mut stream = EventStream::new();
    loop {
        tokio::select! {
            Some(Ok(evt)) = stream.next() => {
                let event: Option<AppEvent> = match evt {
                    CEvent::Key(k) => Some(AppEvent::Input(k)),
                    CEvent::Mouse(m) => Some(AppEvent::Mouse(m)),
                    CEvent::Resize(w, h) => Some(AppEvent::Resize(w, h)),
                    _ => None,
                };
                if let Some(e) = event {
                    if tx.send(e).await.is_err() { break; }
                }
            }
            _ = cancel.cancelled() => break,
        }
    }
}
