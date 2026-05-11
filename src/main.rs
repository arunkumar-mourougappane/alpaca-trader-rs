use std::io;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::EnableMouseCapture,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::{mpsc, watch, Notify};
use tokio_util::sync::CancellationToken;

use alpaca_trader_rs::{
    client::AlpacaClient,
    config::{AlpacaConfig, AlpacaEnv},
    events::Event,
};

/// Alpaca Markets TUI trading terminal.
///
/// Connects to the **live** account by default. Pass `--paper` to use the
/// paper-trading environment (simulated funds, no real money at risk).
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Connect to the paper-trading environment (simulated funds).
    /// Omit to use the live account (real money — default).
    #[arg(long)]
    paper: bool,
}

// Bridge library modules into the binary crate so sub-modules can use `crate::config` etc.
mod client {
    pub use alpaca_trader_rs::client::*;
}
mod commands {
    pub use alpaca_trader_rs::commands::*;
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
mod credentials;
mod handlers;
mod input;
mod ui;
mod update;

use app::App;
use update::update;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    dotenvy::dotenv().ok();

    // ── Logging — must come before enable_raw_mode() ───────────────────────────
    let _log_guard = alpaca_trader_rs::logging::init().unwrap_or_else(|e| {
        eprintln!("Warning: failed to initialise logging: {e}");
        // Return a no-op guard by creating a throwaway channel
        let (_, nb) = tracing_appender::non_blocking(std::io::sink());
        nb
    });

    let env = if args.paper {
        AlpacaEnv::Paper
    } else {
        AlpacaEnv::Live
    };

    // Resolve credentials before entering raw-mode (may print/prompt to terminal).
    let creds = credentials::resolve(env).map_err(|e| {
        tracing::error!(error = %e, "credential resolution failed");
        eprintln!("Error: {e}");
        e
    })?;

    let config = AlpacaConfig::from_credentials(creds).map_err(|e| {
        tracing::error!(error = %e, "configuration error");
        eprintln!("Configuration error: {e}");
        e
    })?;

    tracing::info!(env = config.env_label(), "alpaca-trader starting");

    let client = Arc::new(AlpacaClient::new(config.clone()));
    let refresh_notify = Arc::new(Notify::new());

    // Command channel — sync update() → async command handler
    let (command_tx, command_rx) = mpsc::channel::<alpaca_trader_rs::commands::Command>(8);

    // Symbol watch channel — pushes watchlist symbols to the market stream
    let (symbol_tx, symbol_rx) = watch::channel::<Vec<String>>(vec![]);

    // ── Terminal setup ─────────────────────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(
        config.clone(),
        refresh_notify.clone(),
        command_tx,
        symbol_tx,
    );

    // Event channel
    let (tx, mut rx) = mpsc::channel::<Event>(256);

    // Cancellation token shared by all background tasks
    let cancel = CancellationToken::new();

    // ── Background tasks ───────────────────────────────────────────────────────

    // Input task
    tokio::spawn(handlers::input::run(tx.clone(), cancel.clone()));

    // REST polling task
    tokio::spawn(handlers::rest::run(
        tx.clone(),
        cancel.clone(),
        client.clone(),
        refresh_notify.clone(),
    ));

    // Command execution task (order submit, cancel, watchlist mutations)
    tokio::spawn(handlers::commands::run(
        command_rx,
        tx.clone(),
        client.clone(),
        refresh_notify.clone(),
        cancel.clone(),
    ));

    // Market data WebSocket stream
    tokio::spawn(alpaca_trader_rs::stream::market::run(
        tx.clone(),
        cancel.clone(),
        config.clone(),
        symbol_rx,
    ));

    // Account/trade updates WebSocket stream
    tokio::spawn(alpaca_trader_rs::stream::account::run(
        tx.clone(),
        cancel.clone(),
        config.clone(),
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

    tracing::info!("all tasks spawned, entering main loop");

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
    tracing::info!("shutting down");
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
