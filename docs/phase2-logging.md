# Phase 2 — Logging Research

Logging strategy for `alpaca-trader-rs`. The key constraint: the TUI puts the terminal in raw mode via `crossterm::execute!(stdout, EnterAlternateScreen)` — writing to stdout or stderr corrupts the UI. All logs must go to **file and/or syslog only**, never to any stream.

---

## Recommendation Summary

| Decision | Choice | Reason |
|---|---|---|
| Logging facade | **`tracing`** | Native async/tokio support; spans track context across tasks |
| File output | **`tracing-appender`** | Non-blocking writes; daily rotation built-in |
| Syslog output | **`syslog-tracing`** | Native tracing `Layer`; works on macOS + Linux |
| Composition | **`tracing-subscriber` Registry** | Stack multiple layers cleanly |
| Log directory | Platform-specific (see §5) | Respects OS conventions, no root needed |
| Rolling strategy | **Daily** | Aligns with trading day; `tracing-appender` supports natively |
| Format | **Plaintext** (dev), **JSON** (prod, opt-in) | Human-readable by default; machine-parseable via env var |
| Level control | **`RUST_LOG` env var** | Zero-code, standard, `tracing-subscriber` reads it natively |

---

## Why `tracing` over `log`

The app uses `tokio` with multiple concurrent tasks: REST poller, input handler, tick task, and Phase 2 stream tasks. With `log`, all messages from all tasks interleave with no way to know which task or which trade triggered a log line.

`tracing` adds **spans**: structured context wrappers that survive `.await` boundaries. A span opened around a REST poll includes all log lines emitted during that poll, making the output readable even under concurrent activity.

```rust
// REST poller — span tracks the whole poll cycle
let span = tracing::info_span!("rest_poll");
async {
    tracing::info!("polling account");
    let account = client.get_account().await?;
    tracing::info!(equity = %account.equity, "account updated");
}.instrument(span).await;
```

`tracing` also bridges the `log` facade via `tracing-log`, so third-party crates that still use `log::info!()` emit into the same subscriber without any changes.

---

## Crates

```toml
[dependencies]
# Logging
tracing           = "0.1"
tracing-appender  = "0.2"

# Runtime composition and env-filter
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Syslog layer
syslog-tracing    = "0.2"
syslog            = "6"

# Cross-platform log directory
dirs              = "5"
```

| Crate | Version | Role |
|---|---|---|
| `tracing` | 0.1 | Macros: `info!`, `warn!`, `error!`, `debug!`, spans |
| `tracing-appender` | 0.2 | Non-blocking file writer; daily rolling |
| `tracing-subscriber` | 0.3 | Registry to compose multiple layers; `EnvFilter` for `RUST_LOG` |
| `syslog-tracing` | 0.2 | Tracing `Layer` that writes to syslog |
| `syslog` | 6 | Underlying syslog connection (Unix socket on macOS/Linux) |
| `dirs` | 5 | `dirs::home_dir()`, `dirs::data_local_dir()` — XDG on Linux |

### Why not `log4rs`?
Heavy dependency tree, config-file-driven (more ops overhead), and has a known performance issue where gzip compression on log rotation blocks the calling thread.

### Why not `fern`?
Excellent for `log`-based apps, but doesn't integrate natively with `tracing` spans — you'd lose the async context tracing provides.

### Why `syslog-tracing` over the plain `syslog` crate?
The plain `syslog` crate integrates with the `log` facade, not with `tracing`. `syslog-tracing` implements `tracing_subscriber::Layer`, so it composes with the same Registry that holds the file layer.

---

## Implementation

### Initialization in `main.rs`

Logging must be initialized **before** `enable_raw_mode()` and before any async tasks are spawned. The `_guard` returned by `non_blocking` must be kept alive for the duration of the process — dropping it flushes and closes the log file.

```rust
// src/main.rs
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};
use tracing_appender::rolling;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Initialise logging BEFORE terminal raw mode
    let _log_guard = init_logging()?;

    tracing::info!("alpaca-trader starting");

    // Terminal setup (raw mode — stdout now belongs to ratatui)
    enable_raw_mode()?;
    // ... rest of main ...
}

fn init_logging() -> anyhow::Result<impl Drop> {
    let log_dir = log_dir();
    std::fs::create_dir_all(&log_dir)?;

    // Non-blocking daily-rotating file writer
    let file_appender = rolling::daily(&log_dir, "alpaca-trader.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // File layer — no ANSI colour codes in files
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_line_number(false);

    // Syslog layer
    let syslog_layer = syslog_tracing::Syslog::new(
        "alpaca-trader",
        syslog::Facility::LOG_USER,
    );

    // Level filter: RUST_LOG env var, default to info
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            EnvFilter::new(
                "info,alpaca_trader_rs=debug,tokio=warn,crossterm=warn,ratatui=warn",
            )
        });

    Registry::default()
        .with(filter)
        .with(file_layer)
        .with(syslog_layer)
        .init();

    tracing::info!(log_dir = %log_dir.display(), "logging initialised");

    Ok(guard)  // caller must keep guard alive for the whole process
}
```

### Log directory (platform-aware)

```rust
fn log_dir() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        let mut p = dirs::home_dir().expect("no home directory");
        p.push("Library/Logs/alpaca-trader");
        p
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut p = dirs::data_local_dir()
            .unwrap_or_else(|| {
                let mut h = dirs::home_dir().expect("no home directory");
                h.push(".local/share");
                h
            });
        p.push("alpaca-trader/logs");
        p
    }
}
```

| OS | Path |
|---|---|
| macOS | `~/Library/Logs/alpaca-trader/alpaca-trader.log.YYYY-MM-DD` |
| Linux | `~/.local/share/alpaca-trader/logs/alpaca-trader.log.YYYY-MM-DD` |

No root privileges needed. The directory is created at startup if it does not exist.

---

## Log Levels for This App

| Level | When to use | Examples |
|---|---|---|
| `ERROR` | Trading-breaking; needs immediate attention | Connection to Alpaca lost, order submission 5xx, auth failure |
| `WARN` | Recoverable but notable | Slow API response, stale market data, cancel on already-filled order |
| `INFO` | Normal business events | Order placed/filled/cancelled, position opened/closed, reconnected, market open/close |
| `DEBUG` | Detailed flow — development only | Parsed response fields, task start/stop, watchlist subscription changed |
| `TRACE` | Frame-level — disabled in prod | Raw WebSocket bytes, every tick event |

### Instrumented handlers (examples)

```rust
// handlers/rest.rs
use tracing::{info, warn, error, instrument};

#[instrument(skip(client, tx))]
async fn poll_account(client: &AlpacaClient, tx: &Sender<Event>) {
    match client.get_account().await {
        Ok(a) => {
            info!(equity = %a.equity, buying_power = %a.buying_power, "account polled");
            let _ = tx.send(Event::AccountUpdated(a)).await;
        }
        Err(e) => {
            error!(error = %e, "failed to poll account");
            let _ = tx.send(Event::StatusMsg(format!("Account error: {e}"))).await;
        }
    }
}

// stream/market.rs
#[instrument(skip(tx, config))]
pub async fn run(tx: Sender<Event>, config: AlpacaConfig, symbols: Vec<String>) {
    info!(feed = "iex", symbol_count = symbols.len(), "connecting market stream");
    // ...
    warn!("market stream disconnected, reconnecting in {}s", delay_secs);
}

// handlers/commands.rs
#[instrument(skip(client, tx))]
async fn handle_submit_order(req: &OrderRequest, client: &AlpacaClient, tx: &Sender<Event>) {
    info!(symbol = %req.symbol, side = %req.side, "submitting order");
    match client.submit_order(req).await {
        Ok(order) => info!(order_id = %order.id, status = %order.status, "order accepted"),
        Err(e)    => error!(error = %e, "order submission failed"),
    }
}
```

---

## Rolling File Behaviour

`tracing-appender::rolling::daily` creates one file per day:

```
~/Library/Logs/alpaca-trader/
├── alpaca-trader.log.2026-05-08
├── alpaca-trader.log.2026-05-09
└── alpaca-trader.log.2026-05-10   ← today (current)
```

Old files are not deleted automatically — use a simple cron or logrotate entry to purge files older than 14 days:

```
# /etc/logrotate.d/alpaca-trader  (Linux)
~/.local/share/alpaca-trader/logs/*.log.* {
    rotate 14
    daily
    missingok
    notifempty
    compress
}
```

If size-based rotation is also needed, add [`rolling-file`](https://crates.io/crates/rolling-file) (v0.2) and wrap the `File` it manages inside `tracing_appender::non_blocking`.

---

## JSON Output (optional, opt-in)

Set `ALPACA_LOG_FORMAT=json` to switch to structured JSON — useful when feeding logs into a log aggregator (ELK, Loki, etc.):

```rust
let file_layer: Box<dyn Layer<_> + Send + Sync> =
    if std::env::var("ALPACA_LOG_FORMAT").as_deref() == Ok("json") {
        Box::new(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
    } else {
        Box::new(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
    };
```

**JSON line example:**
```json
{"timestamp":"2026-05-10T14:32:45.123Z","level":"INFO","target":"alpaca_trader_rs::handlers::rest","fields":{"message":"order accepted","order_id":"e8a19a22","status":"filled"}}
```

---

## Runtime Level Control

No code changes needed — `RUST_LOG` is read at startup by `EnvFilter::try_from_default_env()`:

```bash
# Default (set in init_logging)
# info,alpaca_trader_rs=debug,tokio=warn

# Override for a debug session
RUST_LOG=alpaca_trader_rs=trace ./run.sh --paper

# Silence everything except errors
RUST_LOG=error ./run.sh --live

# Debug a specific module only
RUST_LOG=alpaca_trader_rs::stream=debug,info ./run.sh --paper
```

---

## What Changes in Phase 2

Logging is wired during Phase 2 implementation. The file and syslog initialisation belongs in `src/main.rs` before `enable_raw_mode()`. Each new module gets `use tracing::{info, warn, error, debug, instrument}` and annotates its async functions with `#[instrument]`.

| Phase 2 file | Instrumentation |
|---|---|
| `src/commands.rs` | `info!` on each command received; `error!` on API failure |
| `src/handlers/commands.rs` | `#[instrument]` on handler, `info!`/`error!` per operation |
| `src/stream/market.rs` | `info!` on connect/subscribe; `warn!` on disconnect; `debug!` per quote batch |
| `src/stream/account.rs` | `info!` on connect; `info!` per trade fill; `warn!` on disconnect |

---

## Files to Create / Modify

| File | Change |
|---|---|
| `Cargo.toml` | Add `tracing`, `tracing-appender`, `tracing-subscriber`, `syslog-tracing`, `syslog`, `dirs` |
| `src/main.rs` | Add `init_logging()` call before `enable_raw_mode()` |
| `src/logging.rs` | **Create** — `init_logging()` and `log_dir()` functions |
| `src/lib.rs` | Add `pub mod logging` |
| All Phase 2 handler files | Add `use tracing::{...}` and `#[instrument]` |
