use std::sync::Mutex;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

// ── Syslog layer ──────────────────────────────────────────────────────────────

struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}").trim_matches('"').to_string();
        }
    }
}

struct SyslogLayer {
    logger: Mutex<syslog::Logger<syslog::LoggerBackend, syslog::Formatter3164>>,
}

impl<S: tracing::Subscriber> Layer<S> for SyslogLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = MessageVisitor { message: String::new() };
        event.record(&mut visitor);
        if visitor.message.is_empty() {
            return;
        }
        if let Ok(mut logger) = self.logger.lock() {
            let msg = &visitor.message;
            let _ = match *event.metadata().level() {
                tracing::Level::ERROR => logger.err(msg),
                tracing::Level::WARN  => logger.warning(msg),
                tracing::Level::INFO  => logger.info(msg),
                _                     => logger.debug(msg),
            };
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Initialise file + syslog logging. Returns a `WorkerGuard` that **must** be
/// kept alive for the entire process — dropping it flushes and closes the log.
///
/// Call this before `enable_raw_mode()` so stdout is still safe to use for any
/// early error messages from this function itself.
pub fn init() -> anyhow::Result<WorkerGuard> {
    let log_dir = log_dir();
    std::fs::create_dir_all(&log_dir)?;

    // Non-blocking daily-rotating file writer
    let file_appender = tracing_appender::rolling::daily(&log_dir, "alpaca-trader.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true);

    // EnvFilter: RUST_LOG env var takes priority, otherwise sensible defaults
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,alpaca_trader_rs=debug,tokio=warn,crossterm=warn,ratatui=warn")
    });

    // Optional syslog layer — silently skipped if the socket is unavailable
    let syslog_layer = syslog::unix(syslog::Formatter3164 {
        facility: syslog::Facility::LOG_USER,
        hostname: None,
        process: "alpaca-trader".into(),
        pid: std::process::id(),
    })
    .ok()
    .map(|logger| SyslogLayer { logger: Mutex::new(logger) });

    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer);

    if let Some(syslog) = syslog_layer {
        registry.with(syslog).init();
    } else {
        registry.init();
    }

    tracing::info!(log_dir = %log_dir.display(), "logging initialised");
    Ok(guard)
}

fn log_dir() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        let mut p = dirs::home_dir().expect("no home directory");
        p.push("Library/Logs/alpaca-trader");
        p
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut p = dirs::data_local_dir().unwrap_or_else(|| {
            let mut h = dirs::home_dir().expect("no home directory");
            h.push(".local/share");
            h
        });
        p.push("alpaca-trader/logs");
        p
    }
}
