//! Application logging setup using `tracing` and `tracing-appender`.
#[cfg(unix)]
use std::sync::Mutex;

use tracing_appender::non_blocking::WorkerGuard;
#[cfg(unix)]
use tracing_subscriber::Layer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

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

#[cfg(unix)]
struct SyslogLayer {
    logger: Mutex<syslog::Logger<syslog::LoggerBackend, syslog::Formatter3164>>,
}

#[cfg(unix)]
impl<S: tracing::Subscriber> Layer<S> for SyslogLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = MessageVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);
        if visitor.message.is_empty() {
            return;
        }
        if let Ok(mut logger) = self.logger.lock() {
            let msg = &visitor.message;
            let _ = match *event.metadata().level() {
                tracing::Level::ERROR => logger.err(msg),
                tracing::Level::WARN => logger.warning(msg),
                tracing::Level::INFO => logger.info(msg),
                _ => logger.debug(msg),
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
    let guard = init_with_dir(&log_dir)?;
    tracing::info!(log_dir = %log_dir.display(), "logging initialised");
    Ok(guard)
}

/// Set up file + syslog logging rooted at `log_dir`.
///
/// Separated from [`init`] so that tests can exercise the full setup path
/// with a temporary directory without touching the process-wide subscriber.
/// Uses `try_init` so that calling this when a global subscriber is already
/// installed (e.g. in a test process) silently succeeds rather than panicking.
pub(crate) fn init_with_dir(log_dir: &std::path::Path) -> anyhow::Result<WorkerGuard> {
    std::fs::create_dir_all(log_dir)?;

    // Non-blocking daily-rotating file writer
    let file_appender = tracing_appender::rolling::daily(log_dir, "alpaca-trader.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true);

    // EnvFilter: RUST_LOG env var takes priority, otherwise sensible defaults
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,alpaca_trader_rs=debug,tokio=warn,crossterm=warn,ratatui=warn")
    });

    let registry = tracing_subscriber::registry().with(filter).with(file_layer);

    #[cfg(unix)]
    {
        // Optional syslog layer — silently skipped if the socket is unavailable
        let syslog_layer = syslog::unix(syslog::Formatter3164 {
            facility: syslog::Facility::LOG_USER,
            hostname: None,
            process: "alpaca-trader".into(),
            pid: std::process::id(),
        })
        .ok()
        .map(|logger| SyslogLayer {
            logger: Mutex::new(logger),
        });

        if let Some(syslog) = syslog_layer {
            registry.with(syslog).try_init().ok();
        } else {
            registry.try_init().ok();
        }
    }

    #[cfg(not(unix))]
    registry.try_init().ok();

    Ok(guard)
}

fn log_dir() -> std::path::PathBuf {
    log_dir_from(dirs::home_dir())
}

/// Determine the log directory given an optional home path.
///
/// Resolution order (first hit wins):
/// 1. Platform-appropriate subdirectory under `home` (preferred)
/// 2. `./alpaca-trader-logs` relative to the current working directory
/// 3. `<temp_dir>/alpaca-trader-logs`
///
/// A `tracing::warn!` is emitted whenever a fallback is used so the operator
/// can see where logs are being written.
pub(crate) fn log_dir_from(home: Option<std::path::PathBuf>) -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    let platform_dir = home.map(|h| h.join("Library/Logs/alpaca-trader"));

    #[cfg(target_os = "windows")]
    let platform_dir = {
        let _ = home; // unused on Windows; log dir comes from %LOCALAPPDATA%
        dirs::data_local_dir().map(|d| d.join("alpaca-trader").join("logs"))
    };

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let platform_dir = home.map(|h| h.join(".local/share/alpaca-trader/logs"));

    if let Some(dir) = platform_dir {
        return dir;
    }

    // Fallback 1: current working directory
    if let Ok(cwd) = std::env::current_dir() {
        tracing::warn!(
            path = %cwd.display(),
            "$HOME is not set; writing logs relative to current directory"
        );
        return cwd.join("alpaca-trader-logs");
    }

    // Fallback 2: system temp directory
    let tmp = std::env::temp_dir();
    tracing::warn!(
        path = %tmp.display(),
        "could not determine current directory; writing logs to temp directory"
    );
    tmp.join("alpaca-trader-logs")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::layer::SubscriberExt;

    // ── Helper: run a closure under a per-thread subscriber that captures
    //    whatever MessageVisitor extracts from each tracing event. ─────────────

    struct MessageCapture(Arc<Mutex<String>>);

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for MessageCapture {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let mut visitor = MessageVisitor {
                message: String::new(),
            };
            event.record(&mut visitor);
            *self.0.lock().unwrap() = visitor.message.clone();
        }
    }

    /// Run `f` under a thread-local subscriber that records the last captured
    /// message into the returned string. Does not touch the global subscriber.
    fn capture<F: FnOnce()>(f: F) -> String {
        let captured = Arc::new(Mutex::new(String::new()));
        let sub = tracing_subscriber::registry().with(MessageCapture(Arc::clone(&captured)));
        tracing::subscriber::with_default(sub, f);
        let result = captured.lock().unwrap().clone();
        result
    }

    // ── MessageVisitor ────────────────────────────────────────────────────────

    #[test]
    fn message_visitor_record_debug_captures_message() {
        // tracing!("text") records the message via record_debug
        let msg = capture(|| tracing::info!("hello from test"));
        assert_eq!(msg, "hello from test");
    }

    #[test]
    fn message_visitor_record_debug_non_message_field_is_ignored() {
        // count = 42 exercises the record_debug non-"message" branch;
        // the text part still gets captured.
        let msg = capture(|| tracing::info!(count = 42, "with extra field"));
        assert_eq!(msg, "with extra field");
    }

    #[test]
    fn message_visitor_record_str_captures_explicit_message_field() {
        // message = "string" uses record_str for the message field
        let msg = capture(|| tracing::info!(message = "explicit string"));
        assert_eq!(msg, "explicit string");
    }

    #[test]
    fn message_visitor_record_str_non_message_field_is_ignored() {
        // name = "alice" is a &str that is NOT the "message" field;
        // exercises the record_str non-"message" branch.
        let msg = capture(|| tracing::info!(name = "alice", "with str field"));
        assert_eq!(msg, "with str field");
    }

    // ── SyslogLayer ───────────────────────────────────────────────────────────

    #[cfg(unix)]
    fn make_syslog_layer() -> Option<SyslogLayer> {
        syslog::unix(syslog::Formatter3164 {
            facility: syslog::Facility::LOG_USER,
            hostname: None,
            process: "alpaca-trader-test".into(),
            pid: 0,
        })
        .ok()
        .map(|l| SyslogLayer {
            logger: Mutex::new(l),
        })
    }

    #[test]
    #[cfg(unix)]
    fn syslog_layer_empty_message_returns_early_without_panic() {
        let Some(layer) = make_syslog_layer() else {
            return; // syslog socket unavailable — skip
        };
        // An event whose message field is an empty string should not panic.
        let sub = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(sub, || tracing::info!(""));
    }

    #[test]
    #[cfg(unix)]
    fn syslog_layer_dispatches_error_level() {
        let Some(layer) = make_syslog_layer() else {
            return;
        };
        let sub = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(sub, || tracing::error!("error level msg"));
    }

    #[test]
    #[cfg(unix)]
    fn syslog_layer_dispatches_warn_level() {
        let Some(layer) = make_syslog_layer() else {
            return;
        };
        let sub = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(sub, || tracing::warn!("warn level msg"));
    }

    #[test]
    #[cfg(unix)]
    fn syslog_layer_dispatches_info_level() {
        let Some(layer) = make_syslog_layer() else {
            return;
        };
        let sub = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(sub, || tracing::info!("info level msg"));
    }

    #[test]
    #[cfg(unix)]
    fn syslog_layer_dispatches_debug_level() {
        let Some(layer) = make_syslog_layer() else {
            return;
        };
        let sub = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(sub, || tracing::debug!("debug level msg"));
    }

    // ── init_with_dir ─────────────────────────────────────────────────────────

    #[test]
    fn init_with_dir_creates_log_dir_and_returns_guard() {
        let tmp = tempfile::tempdir().unwrap();
        let log_subdir = tmp.path().join("logs");
        // Directory does not exist yet — init_with_dir must create it.
        let _guard =
            init_with_dir(&log_subdir).expect("init_with_dir should succeed with a temp dir");
        assert!(
            log_subdir.exists(),
            "log dir should have been created by init_with_dir"
        );
    }

    #[test]
    fn init_with_dir_is_idempotent_when_subscriber_already_set() {
        let tmp = tempfile::tempdir().unwrap();
        // Call twice — second call must not panic (try_init silently ignores conflicts).
        let _g1 = init_with_dir(tmp.path()).expect("first call should succeed");
        let _g2 = init_with_dir(tmp.path()).expect("second call should not panic");
    }

    // ── log_dir ───────────────────────────────────────────────────────────────

    #[test]
    fn log_dir_returns_non_empty_path() {
        let dir = log_dir();
        assert!(
            dir.components().count() > 0,
            "log_dir() returned an empty path"
        );
    }

    // ── log_dir_from ─────────────────────────────────────────────────────────

    #[test]
    #[cfg(target_os = "macos")]
    fn home_present_returns_macos_log_path() {
        let dir = log_dir_from(Some(PathBuf::from("/Users/tester")));
        assert_eq!(
            dir,
            PathBuf::from("/Users/tester/Library/Logs/alpaca-trader")
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn home_present_last_component_is_alpaca_trader() {
        let dir = log_dir_from(Some(PathBuf::from("/Users/alice")));
        assert_eq!(dir.file_name().unwrap(), "alpaca-trader");
    }

    #[test]
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    fn home_present_returns_xdg_log_path() {
        let dir = log_dir_from(Some(PathBuf::from("/home/tester")));
        assert_eq!(
            dir,
            PathBuf::from("/home/tester/.local/share/alpaca-trader/logs")
        );
    }

    #[test]
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    fn home_present_last_component_is_logs() {
        let dir = log_dir_from(Some(PathBuf::from("/home/alice")));
        assert_eq!(dir.file_name().unwrap(), "logs");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn home_present_returns_windows_log_path() {
        // On Windows the home parameter is ignored; log dir comes from %LOCALAPPDATA%
        let dir = log_dir_from(Some(PathBuf::from("C:\\Users\\tester")));
        let dir_str = dir.to_str().unwrap_or("");
        assert!(
            dir_str.contains("alpaca-trader"),
            "expected alpaca-trader in Windows log path, got: {dir_str}"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn home_present_last_component_is_logs_on_windows() {
        let dir = log_dir_from(Some(PathBuf::from("C:\\Users\\alice")));
        assert_eq!(dir.file_name().unwrap(), "logs");
    }

    #[test]
    fn no_home_falls_back_to_non_panicking_dir() {
        let dir = log_dir_from(None);
        assert!(
            dir.ends_with("alpaca-trader-logs"),
            "fallback path should end with alpaca-trader-logs, got: {}",
            dir.display()
        );
    }

    #[test]
    fn no_home_fallback_is_absolute() {
        let dir = log_dir_from(None);
        assert!(
            dir.is_absolute(),
            "fallback log dir should be absolute, got: {}",
            dir.display()
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn log_dir_from_preserves_home_prefix() {
        let home = PathBuf::from("/tmp/fakehome");
        let dir = log_dir_from(Some(home.clone()));
        assert!(dir.starts_with(&home));
    }
}
