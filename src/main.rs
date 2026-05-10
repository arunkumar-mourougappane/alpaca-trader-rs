use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    event::EnableMouseCapture,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::{mpsc, Notify};
use tokio_util::sync::CancellationToken;

use alpaca_trader_rs::{client::AlpacaClient, config::AlpacaConfig, events::Event};

// Bridge library modules into the binary crate so sub-modules can use `crate::config` etc.
mod client {
    pub use alpaca_trader_rs::client::*;
}
mod config {
    pub use alpaca_trader_rs::config::*;
}
mod events {
    pub use alpaca_trader_rs::events::*;
}
mod types {
    pub use alpaca_trader_rs::types::*;
}

mod app;
mod handlers;
mod ui;
mod update;

use app::App;
use update::update;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = AlpacaConfig::from_env().map_err(|e| {
        eprintln!("Configuration error: {}", e);
        eprintln!("Copy .env.example to .env and fill in your Alpaca API credentials.");
        e
    })?;

    let client = Arc::new(AlpacaClient::new(config.clone()));
    let refresh_notify = Arc::new(Notify::new());

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config, refresh_notify.clone());

    // Event channel
    let (tx, mut rx) = mpsc::channel::<Event>(256);

    // Cancellation token shared by all background tasks
    let cancel = CancellationToken::new();

    // Input task
    tokio::spawn(handlers::input::run(tx.clone(), cancel.clone()));

    // REST polling task
    tokio::spawn(handlers::rest::run(
        tx.clone(),
        cancel.clone(),
        client.clone(),
        refresh_notify.clone(),
    ));

    // Tick task — drives clock refresh every 250 ms
    {
        let tx = tx.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(250));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if tx.send(Event::Tick).await.is_err() { break; }
                    }
                    _ = cancel.cancelled() => break,
                }
            }
        });
    }

    // Initial data load before first render
    tokio::spawn(handlers::rest::poll_once(tx.clone(), client.clone()));

    // ── Main loop ──────────────────────────────────────────────────────────────
    loop {
        terminal.draw(|f| ui::render(f, &mut app))?;

        match rx.recv().await {
            Some(Event::Quit) | None => break,
            Some(event) => update(&mut app, event),
        }

        if app.should_quit {
            break;
        }
    }

    // ── Cleanup ────────────────────────────────────────────────────────────────
    cancel.cancel();
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
