use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc::Sender, Notify};
use tokio_util::sync::CancellationToken;

use crate::client::AlpacaClient;
use crate::events::Event;

pub async fn run(
    tx: Sender<Event>,
    cancel: CancellationToken,
    client: Arc<AlpacaClient>,
    refresh_notify: Arc<Notify>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                poll_all(&client, &tx).await;
            }
            _ = refresh_notify.notified() => {
                poll_all(&client, &tx).await;
                let _ = tx.send(Event::StatusMsg(String::new())).await;
            }
            _ = cancel.cancelled() => break,
        }
    }
}

pub async fn poll_once(tx: Sender<Event>, client: Arc<AlpacaClient>) {
    poll_all(&client, &tx).await;
}

async fn poll_all(client: &AlpacaClient, tx: &Sender<Event>) {
    tokio::join!(
        poll_account(client, tx),
        poll_positions(client, tx),
        poll_orders(client, tx),
        poll_clock(client, tx),
        poll_watchlist(client, tx),
    );
}

async fn poll_account(client: &AlpacaClient, tx: &Sender<Event>) {
    match client.get_account().await {
        Ok(a) => {
            let _ = tx.send(Event::AccountUpdated(a)).await;
        }
        Err(e) => {
            let _ = tx
                .send(Event::StatusMsg(format!("Account error: {}", e)))
                .await;
        }
    }
}

async fn poll_positions(client: &AlpacaClient, tx: &Sender<Event>) {
    match client.get_positions().await {
        Ok(p) => {
            let _ = tx.send(Event::PositionsUpdated(p)).await;
        }
        Err(e) => {
            let _ = tx
                .send(Event::StatusMsg(format!("Positions error: {}", e)))
                .await;
        }
    }
}

async fn poll_orders(client: &AlpacaClient, tx: &Sender<Event>) {
    match client.get_orders("all").await {
        Ok(o) => {
            let _ = tx.send(Event::OrdersUpdated(o)).await;
        }
        Err(e) => {
            let _ = tx
                .send(Event::StatusMsg(format!("Orders error: {}", e)))
                .await;
        }
    }
}

async fn poll_clock(client: &AlpacaClient, tx: &Sender<Event>) {
    if let Ok(c) = client.get_clock().await {
        let _ = tx.send(Event::ClockUpdated(c)).await;
    }
}

async fn poll_watchlist(client: &AlpacaClient, tx: &Sender<Event>) {
    let summaries = match client.list_watchlists().await {
        Ok(s) => s,
        Err(e) => {
            let _ = tx
                .send(Event::StatusMsg(format!("Watchlist error: {}", e)))
                .await;
            return;
        }
    };
    if summaries.is_empty() {
        return;
    }
    match client.get_watchlist(&summaries[0].id).await {
        Ok(w) => {
            let _ = tx.send(Event::WatchlistUpdated(w)).await;
        }
        Err(e) => {
            let _ = tx
                .send(Event::StatusMsg(format!("Watchlist error: {}", e)))
                .await;
        }
    }
}
