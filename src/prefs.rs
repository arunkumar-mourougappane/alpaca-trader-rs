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
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use ratatui::symbols;
use serde::{Deserialize, Serialize};

// ── Chart marker ──────────────────────────────────────────────────────────────

/// Chart dataset marker style.
///
/// Controls the glyph used to draw line and scatter chart datasets.
/// Corresponds 1:1 to [`ratatui::symbols::Marker`].
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartMarker {
    /// High-resolution braille dots (requires UTF-8 + braille font).
    #[default]
    Braille,
    /// Simple dot (`·`); works on all terminals.
    Dot,
    /// Solid block (`█`).
    Block,
    /// Vertical bar (`|`).
    Bar,
    /// Half-block (`▄`); medium resolution, wide support.
    HalfBlock,
}

impl ChartMarker {
    /// Converts to the corresponding [`ratatui::symbols::Marker`] variant.
    pub fn to_ratatui(self) -> symbols::Marker {
        match self {
            Self::Braille => symbols::Marker::Braille,
            Self::Dot => symbols::Marker::Dot,
            Self::Block => symbols::Marker::Block,
            Self::Bar => symbols::Marker::Bar,
            Self::HalfBlock => symbols::Marker::HalfBlock,
        }
    }

    /// Returns the snake_case TOML string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Braille => "braille",
            Self::Dot => "dot",
            Self::Block => "block",
            Self::Bar => "bar",
            Self::HalfBlock => "half_block",
        }
    }
}

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
    /// Marker style used for all chart datasets.  Accepted values:
    /// `"braille"` | `"dot"` | `"block"` | `"bar"` | `"half_block"`.
    pub chart_marker: ChartMarker,
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
            chart_marker: ChartMarker::default(),
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
    /// Price alerts per symbol (`[price_alerts]` section).
    #[serde(default)]
    pub price_alerts: HashMap<String, crate::types::PriceAlert>,
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
        let proxy_http = self
            .proxy
            .http
            .as_deref()
            .map(|v| format!("http   = \"{v}\""))
            .unwrap_or_else(|| "# http   = \"http://proxy.corp.com:8080\"".into());
        let proxy_socks5 = self
            .proxy
            .socks5
            .as_deref()
            .map(|v| format!("socks5 = \"{v}\""))
            .unwrap_or_else(|| "# socks5 = \"socks5://proxy.corp.com:1080\"".into());
        let proxy_no_proxy = self
            .proxy
            .no_proxy
            .as_deref()
            .map(|v| format!("no_proxy = \"{v}\""))
            .unwrap_or_else(|| "# no_proxy = \"localhost,127.0.0.1\"".into());

        let mut toml_str = format!(
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
# Toggle dashboard panels. All default to true.
show_account_panel = {show_account}
show_watchlist     = {show_watchlist}
show_positions     = {show_positions}
show_orders        = {show_orders}
# Default historical date range for the equity chart.
# Accepted values: "1D" | "5D" | "1M" | "3M" | "1Y" | "ALL"
default_equity_range = "{equity_range}"
# Visual marker style for chart data points.
# Accepted values: "dot" | "block" | "braille"
chart_marker = "{chart_marker}"

[stream]
# WebSocket reconnection tuning.
reconnect_max_attempts    = {reconnect_max}
reconnect_backoff_base_ms = {reconnect_base}

[notifications]
# Toggle desktop notification popups for order execution fills.
fill_notifications_enabled = {fill_enabled}
# Notification display duration in milliseconds.
fill_notification_ttl_ms   = {fill_ttl}
# TUI status bar message duration in milliseconds.
status_message_ttl_ms      = {status_ttl}

[safety]
# Require user confirmation before removing a symbol from the watchlist.
confirm_watchlist_remove = {confirm_remove}

[proxy]
# Leave commented to use HTTP_PROXY / HTTPS_PROXY environment variables
{proxy_http}
{proxy_socks5}
{proxy_no_proxy}
"#,
            default_env = self.app.default_env,
            refresh_ms = self.app.refresh_interval_ms,
            theme = self.ui.theme,
            show_account = self.ui.show_account_panel,
            show_watchlist = self.ui.show_watchlist,
            show_positions = self.ui.show_positions,
            show_orders = self.ui.show_orders,
            equity_range = self.ui.default_equity_range,
            chart_marker = self.ui.chart_marker.as_str(),
            reconnect_max = self.stream.reconnect_max_attempts,
            reconnect_base = self.stream.reconnect_backoff_base_ms,
            fill_enabled = self.notifications.fill_notifications_enabled,
            fill_ttl = self.notifications.fill_notification_ttl_ms,
            status_ttl = self.notifications.status_message_ttl_ms,
            confirm_remove = self.safety.confirm_watchlist_remove,
            proxy_http = proxy_http,
            proxy_socks5 = proxy_socks5,
            proxy_no_proxy = proxy_no_proxy,
        );

        if !self.price_alerts.is_empty() {
            toml_str.push_str("\n[price_alerts]\n");
            let mut sorted_keys: Vec<&String> = self.price_alerts.keys().collect();
            sorted_keys.sort();
            for key in sorted_keys {
                if let Some(alert) = self.price_alerts.get(key) {
                    if alert.above.is_some() || alert.below.is_some() {
                        let mut parts = Vec::new();
                        if let Some(above) = alert.above {
                            parts.push(format!("above = {above}"));
                        }
                        if let Some(below) = alert.below {
                            parts.push(format!("below = {below}"));
                        }
                        toml_str.push_str(&format!("\"{key}\" = {{ {} }}\n", parts.join(", ")));
                    }
                }
            }
        }

        toml_str
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

    #[test]
    #[cfg(unix)]
    fn load_from_unreadable_file_returns_defaults() {
        use std::os::unix::fs::PermissionsExt;
        let f = write_toml("[app]\ndefault_env = \"paper\"\n");
        let path = f.path().to_path_buf();
        // Make the file unreadable.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let p = AppPrefs::load_from(&path);
        // Restore permissions so the temp file can be cleaned up.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            p,
            AppPrefs::default(),
            "unreadable file should yield defaults"
        );
    }

    #[test]
    fn default_chart_marker_is_braille() {
        let p = AppPrefs::default();
        assert_eq!(p.ui.chart_marker, ChartMarker::Braille);
    }

    #[test]
    fn chart_marker_as_str_round_trips() {
        let cases = [
            (ChartMarker::Braille, "braille"),
            (ChartMarker::Dot, "dot"),
            (ChartMarker::Block, "block"),
            (ChartMarker::Bar, "bar"),
            (ChartMarker::HalfBlock, "half_block"),
        ];
        for (variant, expected) in cases {
            assert_eq!(variant.as_str(), expected);
        }
    }

    #[test]
    fn chart_marker_to_ratatui_maps_all_variants() {
        use ratatui::symbols;
        assert_eq!(ChartMarker::Braille.to_ratatui(), symbols::Marker::Braille);
        assert_eq!(ChartMarker::Dot.to_ratatui(), symbols::Marker::Dot);
        assert_eq!(ChartMarker::Block.to_ratatui(), symbols::Marker::Block);
        assert_eq!(ChartMarker::Bar.to_ratatui(), symbols::Marker::Bar);
        assert_eq!(
            ChartMarker::HalfBlock.to_ratatui(),
            symbols::Marker::HalfBlock
        );
    }

    #[test]
    fn chart_marker_parses_from_toml() {
        let f = write_toml(
            r#"
[ui]
chart_marker = "dot"
"#,
        );
        let p = AppPrefs::load_from(f.path());
        assert_eq!(p.ui.chart_marker, ChartMarker::Dot);
    }

    #[test]
    fn chart_marker_all_variants_parse_from_toml() {
        let cases = [
            ("braille", ChartMarker::Braille),
            ("dot", ChartMarker::Dot),
            ("block", ChartMarker::Block),
            ("bar", ChartMarker::Bar),
            ("half_block", ChartMarker::HalfBlock),
        ];
        for (toml_val, expected) in cases {
            let content = format!("[ui]\nchart_marker = \"{toml_val}\"\n");
            let f = write_toml(&content);
            let p = AppPrefs::load_from(f.path());
            assert_eq!(
                p.ui.chart_marker, expected,
                "chart_marker = {toml_val:?} should parse to {expected:?}"
            );
        }
    }

    #[test]
    fn chart_marker_invalid_value_falls_back_to_defaults() {
        let f = write_toml("[ui]\nchart_marker = \"invalid_value\"\n");
        let p = AppPrefs::load_from(f.path());
        assert_eq!(
            p,
            AppPrefs::default(),
            "invalid chart_marker should fall back to defaults"
        );
    }

    #[test]
    fn chart_marker_missing_falls_back_to_braille() {
        let f = write_toml("[ui]\ntheme = \"dark\"\n");
        let p = AppPrefs::load_from(f.path());
        assert_eq!(p.ui.chart_marker, ChartMarker::Braille);
    }

    #[test]
    fn chart_marker_round_trips_through_toml_string() {
        let mut p = AppPrefs::default();
        p.ui.chart_marker = ChartMarker::HalfBlock;
        let toml_str = p.to_toml_string();
        let p2: AppPrefs = toml::from_str(&toml_str).unwrap();
        assert_eq!(p2.ui.chart_marker, ChartMarker::HalfBlock);
    }

    #[test]
    fn load_from_write_failure_returns_defaults() {
        // Pass a path whose parent directory does not exist and cannot be created
        // (a file used as if it were a directory). This causes write_to to fail
        // but load_from should still return defaults without panicking.
        let f = write_toml("");
        // Use the existing *file* as the "parent directory" — the child path
        // cannot exist, so path.exists() is false, and write_to will fail when
        // it tries to create_dir_all on a path whose ancestor is a regular file.
        let bogus_path = f.path().join("subdir").join("config.toml");
        let p = AppPrefs::load_from(&bogus_path);
        assert_eq!(
            p,
            AppPrefs::default(),
            "write failure path should still return defaults"
        );
    }

    #[test]
    fn price_alerts_round_trip() {
        let mut p = AppPrefs::default();
        let mut alerts = HashMap::new();
        alerts.insert(
            "AAPL".to_string(),
            crate::types::PriceAlert {
                above: Some(185.0),
                below: Some(170.0),
                ..Default::default()
            },
        );
        alerts.insert(
            "TSLA".to_string(),
            crate::types::PriceAlert {
                above: Some(250.5),
                below: None,
                ..Default::default()
            },
        );
        alerts.insert(
            "BRK.B".to_string(),
            crate::types::PriceAlert {
                above: Some(350.0),
                below: Some(340.0),
                ..Default::default()
            },
        );
        p.price_alerts = alerts;
        let toml_str = p.to_toml_string();
        let p2: AppPrefs = toml::from_str(&toml_str).unwrap();
        assert_eq!(p.price_alerts, p2.price_alerts);
    }

    #[test]
    fn proxy_round_trip() {
        let mut p = AppPrefs::default();
        p.proxy.http = Some("http://proxy.corp.com:8080".to_string());
        p.proxy.socks5 = Some("socks5://proxy.corp.com:1080".to_string());
        p.proxy.no_proxy = Some("localhost,127.0.0.1".to_string());

        let toml_str = p.to_toml_string();
        let p2: AppPrefs = toml::from_str(&toml_str).unwrap();
        assert_eq!(p.proxy, p2.proxy);
    }
}
