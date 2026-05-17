//! User preferences persisted in `~/.config/alpaca-trader/config.toml`.
//!
//! [`AppPrefs`] is loaded at startup via [`AppPrefs::load`]. Missing fields
//! fall back to compiled defaults; unknown fields are silently ignored.
//! Credentials (API keys) are **never** stored here — they live in `.env` or
//! the OS keychain.
//!
//! # Priority order (highest wins)
//!
//! 1. CLI flags (`--paper`, `--dry-run`)
//! 2. Environment variables (`PAPER_ALPACA_*`, `LIVE_ALPACA_*`)
//! 3. `config.toml` preferences (this module)
//! 4. Compiled defaults (defined via `Default` impls below)
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ── Sub-sections ──────────────────────────────────────────────────────────────

/// Application-wide settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSection {
    /// Which environment to connect to when neither `--paper` nor `--live`
    /// flags are supplied.  Accepted values: `"paper"` | `"live"`.
    pub default_env: String,
    /// How often the REST polling task refreshes data (milliseconds).
    pub refresh_interval_ms: u64,
}

impl Default for AppSection {
    fn default() -> Self {
        Self {
            default_env: "live".into(),
            refresh_interval_ms: 5000,
        }
    }
}

/// UI display preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UiSection {
    /// Active colour theme. Accepted values: `"default"` | `"dark"` |
    /// `"high-contrast"`.  Theme switching UI is tracked in issue #62.
    pub theme: String,
    /// Show the Account panel tab.
    pub show_account_panel: bool,
    /// Show the Watchlist panel tab.
    pub show_watchlist: bool,
    /// Show the Positions panel tab.
    pub show_positions: bool,
    /// Show the Orders panel tab.
    pub show_orders: bool,
    /// Default equity-chart time range.  Accepted values:
    /// `"1D"` | `"1W"` | `"1M"` | `"YTD"`.  Range-picker UI is tracked in
    /// issue #77.
    pub default_equity_range: String,
}

impl Default for UiSection {
    fn default() -> Self {
        Self {
            theme: "default".into(),
            show_account_panel: true,
            show_watchlist: true,
            show_positions: true,
            show_orders: true,
            default_equity_range: "1D".into(),
        }
    }
}

/// WebSocket stream reconnection settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct StreamSection {
    /// Maximum number of reconnect attempts before giving up.  `0` means
    /// unlimited.
    pub reconnect_max_attempts: u32,
    /// Base backoff delay in milliseconds; doubles on each failed attempt up
    /// to 30 seconds.
    pub reconnect_backoff_base_ms: u64,
}

impl Default for StreamSection {
    fn default() -> Self {
        Self {
            reconnect_max_attempts: 0,
            reconnect_backoff_base_ms: 1000,
        }
    }
}

/// In-app notification settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NotificationsSection {
    /// Show a transient status-bar message when an order fill is received.
    pub fill_notifications_enabled: bool,
    /// How long fill notifications remain visible (milliseconds).
    pub fill_notification_ttl_ms: u64,
    /// How long generic transient status messages stay on screen
    /// (milliseconds).
    pub status_message_ttl_ms: u64,
}

impl Default for NotificationsSection {
    fn default() -> Self {
        Self {
            fill_notifications_enabled: true,
            fill_notification_ttl_ms: 4000,
            status_message_ttl_ms: 2000,
        }
    }
}

/// Safety guard settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SafetySection {
    /// Show a confirmation prompt before removing a symbol from the watchlist.
    pub confirm_watchlist_remove: bool,
}

impl Default for SafetySection {
    fn default() -> Self {
        Self {
            confirm_watchlist_remove: true,
        }
    }
}

/// HTTP/SOCKS proxy settings.
///
/// Leave all fields unset to use the `HTTP_PROXY` / `HTTPS_PROXY`
/// environment variables automatically (tracked in issue #90).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ProxySection {
    /// HTTP proxy URL, e.g. `"http://proxy.corp.com:8080"`.
    pub http: Option<String>,
    /// SOCKS5 proxy URL, e.g. `"socks5://proxy.corp.com:1080"`.
    pub socks5: Option<String>,
    /// Comma-separated list of hosts that bypass the proxy,
    /// e.g. `"localhost,127.0.0.1"`.
    pub no_proxy: Option<String>,
}

// ── Root struct ───────────────────────────────────────────────────────────────

/// All user preferences loaded from `~/.config/alpaca-trader/config.toml`.
///
/// Construct via [`AppPrefs::load`]; direct construction is mainly useful in
/// tests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AppPrefs {
    /// Application-wide settings (`[app]` section).
    pub app: AppSection,
    /// UI display preferences (`[ui]` section).
    pub ui: UiSection,
    /// WebSocket stream settings (`[stream]` section).
    pub stream: StreamSection,
    /// Notification settings (`[notifications]` section).
    pub notifications: NotificationsSection,
    /// Safety guard settings (`[safety]` section).
    pub safety: SafetySection,
    /// Proxy settings (`[proxy]` section).
    pub proxy: ProxySection,
}

impl AppPrefs {
    /// Returns the canonical path for the config file.
    ///
    /// Uses [`dirs::config_dir`] so the location is platform-appropriate:
    /// - **macOS** — `~/Library/Application Support/alpaca-trader/config.toml`
    /// - **Linux** — `~/.config/alpaca-trader/config.toml`
    /// - **Windows** — `%APPDATA%\alpaca-trader\config.toml`
    ///
    /// Returns `None` if the home directory cannot be determined.
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("alpaca-trader").join("config.toml"))
    }

    /// Loads preferences from [`AppPrefs::default_path`].
    ///
    /// - If the file is **absent**, creates it with compiled defaults and
    ///   prints a one-time notice to `stderr`.
    /// - If the file exists but cannot be parsed, logs a warning and returns
    ///   defaults (never panics).
    /// - Missing fields within a valid TOML file fall back to defaults.
    pub fn load() -> Self {
        let Some(path) = Self::default_path() else {
            tracing::warn!("cannot determine config directory; using default preferences");
            return Self::default();
        };
        Self::load_from(&path)
    }

    /// Load from an explicit path — used internally and in tests.
    pub fn load_from(path: &std::path::Path) -> Self {
        if !path.exists() {
            let defaults = Self::default();
            if let Err(e) = defaults.write_to(path) {
                tracing::warn!(path = %path.display(), error = %e, "could not write default config");
            } else {
                eprintln!(
                    "alpaca-trader: created default config at {}",
                    path.display()
                );
            }
            return defaults;
        }

        match std::fs::read_to_string(path) {
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "could not read config file; using defaults");
                Self::default()
            }
            Ok(text) => match toml::from_str::<Self>(&text) {
                Ok(prefs) => prefs,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "could not parse config file; using defaults");
                    Self::default()
                }
            },
        }
    }

    /// Serialises the preferences to TOML and writes to `path`, creating any
    /// missing parent directories.
    pub fn write_to(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_text = self.to_toml_string();
        std::fs::write(path, toml_text)?;
        Ok(())
    }

    /// Serialises to a TOML string with descriptive comments for each
    /// section.
    pub fn to_toml_string(&self) -> String {
        format!(
            r#"# alpaca-trader configuration
# Generated automatically on first launch. Edit and restart to apply changes.
# Credentials (API keys) are stored separately in the OS keychain, never here.

[app]
# Default environment when --paper / --live is not specified.
# Accepted values: "paper" | "live"
default_env = "{default_env}"
# REST polling interval in milliseconds.
refresh_interval_ms = {refresh_ms}

[ui]
# Colour theme. Accepted values: "default" | "dark" | "high-contrast"
theme = "{theme}"
show_account_panel = {show_account}
show_watchlist     = {show_watchlist}
show_positions     = {show_positions}
show_orders        = {show_orders}
# Default equity chart range. Accepted values: "1D" | "1W" | "1M" | "YTD"
default_equity_range = "{equity_range}"

[stream]
# Max reconnect attempts (0 = unlimited)
reconnect_max_attempts = {reconnect_max}
# Base backoff between reconnects in milliseconds (doubles each attempt, capped at 30 s)
reconnect_backoff_base_ms = {reconnect_base}

[notifications]
fill_notifications_enabled = {fill_enabled}
fill_notification_ttl_ms   = {fill_ttl}
status_message_ttl_ms      = {status_ttl}

[safety]
# Prompt for confirmation before removing a watchlist symbol
confirm_watchlist_remove = {confirm_remove}

[proxy]
# Leave commented to use HTTP_PROXY / HTTPS_PROXY environment variables
# http   = "http://proxy.corp.com:8080"
# socks5 = "socks5://proxy.corp.com:1080"
# no_proxy = "localhost,127.0.0.1"
"#,
            default_env = self.app.default_env,
            refresh_ms = self.app.refresh_interval_ms,
            theme = self.ui.theme,
            show_account = self.ui.show_account_panel,
            show_watchlist = self.ui.show_watchlist,
            show_positions = self.ui.show_positions,
            show_orders = self.ui.show_orders,
            equity_range = self.ui.default_equity_range,
            reconnect_max = self.stream.reconnect_max_attempts,
            reconnect_base = self.stream.reconnect_backoff_base_ms,
            fill_enabled = self.notifications.fill_notifications_enabled,
            fill_ttl = self.notifications.fill_notification_ttl_ms,
            status_ttl = self.notifications.status_message_ttl_ms,
            confirm_remove = self.safety.confirm_watchlist_remove,
        )
    }

    /// Returns the configured status-message TTL as a [`Duration`].
    pub fn status_ttl(&self) -> Duration {
        Duration::from_millis(self.notifications.status_message_ttl_ms)
    }

    /// Returns the configured fill-notification TTL as a [`Duration`].
    pub fn fill_ttl(&self) -> Duration {
        Duration::from_millis(self.notifications.fill_notification_ttl_ms)
    }

    /// Returns the configured REST polling interval as a [`Duration`].
    pub fn refresh_interval(&self) -> Duration {
        Duration::from_millis(self.app.refresh_interval_ms)
    }

    /// Returns the base reconnect backoff as a [`Duration`].
    pub fn reconnect_backoff_base(&self) -> Duration {
        Duration::from_millis(self.stream.reconnect_backoff_base_ms)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_toml(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn default_prefs_have_expected_values() {
        let p = AppPrefs::default();
        assert_eq!(p.app.default_env, "live");
        assert_eq!(p.app.refresh_interval_ms, 5000);
        assert_eq!(p.ui.theme, "default");
        assert!(p.ui.show_account_panel);
        assert!(p.ui.show_watchlist);
        assert_eq!(p.ui.default_equity_range, "1D");
        assert_eq!(p.stream.reconnect_max_attempts, 0);
        assert_eq!(p.stream.reconnect_backoff_base_ms, 1000);
        assert!(p.notifications.fill_notifications_enabled);
        assert_eq!(p.notifications.fill_notification_ttl_ms, 4000);
        assert_eq!(p.notifications.status_message_ttl_ms, 2000);
        assert!(p.safety.confirm_watchlist_remove);
        assert!(p.proxy.http.is_none());
    }

    #[test]
    fn load_from_valid_toml_overrides_defaults() {
        let f = write_toml(
            r#"
[app]
default_env = "paper"
refresh_interval_ms = 10000

[stream]
reconnect_max_attempts = 3
reconnect_backoff_base_ms = 500

[notifications]
status_message_ttl_ms = 1500
fill_notifications_enabled = false
"#,
        );
        let p = AppPrefs::load_from(f.path());
        assert_eq!(p.app.default_env, "paper");
        assert_eq!(p.app.refresh_interval_ms, 10000);
        assert_eq!(p.stream.reconnect_max_attempts, 3);
        assert_eq!(p.stream.reconnect_backoff_base_ms, 500);
        assert_eq!(p.notifications.status_message_ttl_ms, 1500);
        assert!(!p.notifications.fill_notifications_enabled);
        // Unspecified fields fall back to defaults
        assert_eq!(p.ui.theme, "default");
        assert!(p.safety.confirm_watchlist_remove);
    }

    #[test]
    fn load_from_invalid_toml_returns_defaults() {
        let f = write_toml("not valid toml !!!");
        let p = AppPrefs::load_from(f.path());
        assert_eq!(p, AppPrefs::default());
    }

    #[test]
    fn load_from_missing_file_creates_it_and_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("config.toml");
        assert!(!path.exists());
        let p = AppPrefs::load_from(&path);
        assert_eq!(p, AppPrefs::default());
        assert!(path.exists(), "config file should be created");
        // Round-trip: the created file should parse back to defaults
        let p2 = AppPrefs::load_from(&path);
        assert_eq!(p, p2);
    }

    #[test]
    fn to_toml_string_round_trips() {
        let mut p = AppPrefs::default();
        p.app.default_env = "paper".into();
        p.stream.reconnect_max_attempts = 5;
        p.notifications.status_message_ttl_ms = 3000;
        let toml_str = p.to_toml_string();
        let p2: AppPrefs = toml::from_str(&toml_str).unwrap();
        assert_eq!(p.app.default_env, p2.app.default_env);
        assert_eq!(
            p.stream.reconnect_max_attempts,
            p2.stream.reconnect_max_attempts
        );
        assert_eq!(
            p.notifications.status_message_ttl_ms,
            p2.notifications.status_message_ttl_ms
        );
    }

    #[test]
    fn duration_helpers_return_correct_values() {
        let mut p = AppPrefs::default();
        p.notifications.status_message_ttl_ms = 2500;
        p.notifications.fill_notification_ttl_ms = 6000;
        p.app.refresh_interval_ms = 8000;
        p.stream.reconnect_backoff_base_ms = 750;
        assert_eq!(p.status_ttl(), Duration::from_millis(2500));
        assert_eq!(p.fill_ttl(), Duration::from_millis(6000));
        assert_eq!(p.refresh_interval(), Duration::from_millis(8000));
        assert_eq!(p.reconnect_backoff_base(), Duration::from_millis(750));
    }

    #[test]
    fn partial_toml_file_fills_missing_sections_with_defaults() {
        let f = write_toml("[safety]\nconfirm_watchlist_remove = false\n");
        let p = AppPrefs::load_from(f.path());
        assert!(!p.safety.confirm_watchlist_remove);
        // All other sections should be default
        assert_eq!(p.app, AppSection::default());
        assert_eq!(p.stream, StreamSection::default());
    }
}
