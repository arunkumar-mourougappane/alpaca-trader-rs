//! Runtime configuration loaded from environment variables.
use anyhow::{Context, Result};

#[cfg(test)]
mod tests {
    use super::*;

    fn paper_vars(endpoint: &str) -> Vec<(&'static str, Option<String>)> {
        vec![
            ("PAPER_ALPACA_ENDPOINT", Some(endpoint.into())),
            ("PAPER_ALPACA_KEY", Some("PKTEST000".into())),
            ("PAPER_ALPACA_SECRET", Some("secret000".into())),
        ]
    }

    fn live_vars(endpoint: &str) -> Vec<(&'static str, Option<String>)> {
        vec![
            ("LIVE_ALPACA_ENDPOINT", Some(endpoint.into())),
            ("LIVE_ALPACA_KEY", Some("AKTEST000".into())),
            ("LIVE_ALPACA_SECRET", Some("secret000".into())),
        ]
    }

    #[test]
    fn env_label_paper() {
        let cfg = AlpacaConfig {
            base_url: String::new(),
            key: String::new(),
            secret: String::new(),
            env: AlpacaEnv::Paper,
            dry_run: false,
        };
        assert_eq!(cfg.env_label(), "PAPER");
    }

    #[test]
    fn env_label_live() {
        let cfg = AlpacaConfig {
            base_url: String::new(),
            key: String::new(),
            secret: String::new(),
            env: AlpacaEnv::Live,
            dry_run: false,
        };
        assert_eq!(cfg.env_label(), "LIVE");
    }

    #[test]
    fn from_env_paper_selects_paper_vars() {
        temp_env::with_vars(paper_vars("https://paper-api.alpaca.markets/v2"), || {
            let cfg = AlpacaConfig::from_env(AlpacaEnv::Paper).unwrap();
            assert_eq!(cfg.env, AlpacaEnv::Paper);
            assert_eq!(cfg.base_url, "https://paper-api.alpaca.markets/v2");
            assert_eq!(cfg.key, "PKTEST000");
            assert_eq!(cfg.secret, "secret000");
        });
    }

    #[test]
    fn from_env_paper_trailing_slash_stripped() {
        temp_env::with_vars(paper_vars("https://paper-api.alpaca.markets/v2/"), || {
            let cfg = AlpacaConfig::from_env(AlpacaEnv::Paper).unwrap();
            assert_eq!(cfg.base_url, "https://paper-api.alpaca.markets/v2");
        });
    }

    #[test]
    fn from_env_live_appends_v2() {
        temp_env::with_vars(live_vars("https://api.alpaca.markets"), || {
            let cfg = AlpacaConfig::from_env(AlpacaEnv::Live).unwrap();
            assert_eq!(cfg.env, AlpacaEnv::Live);
            assert_eq!(cfg.base_url, "https://api.alpaca.markets/v2");
        });
    }

    #[test]
    fn from_env_live_no_double_slash() {
        temp_env::with_vars(live_vars("https://api.alpaca.markets/"), || {
            let cfg = AlpacaConfig::from_env(AlpacaEnv::Live).unwrap();
            assert_eq!(cfg.base_url, "https://api.alpaca.markets/v2");
        });
    }

    #[test]
    fn from_env_missing_paper_key_errors() {
        temp_env::with_vars(
            [
                (
                    "PAPER_ALPACA_ENDPOINT",
                    Some("https://paper-api.alpaca.markets/v2".to_string()),
                ),
                ("PAPER_ALPACA_KEY", None),
                ("PAPER_ALPACA_SECRET", Some("secret".to_string())),
            ],
            || {
                assert!(AlpacaConfig::from_env(AlpacaEnv::Paper).is_err());
            },
        );
    }

    #[test]
    fn from_env_missing_live_key_errors() {
        temp_env::with_vars(
            [
                (
                    "LIVE_ALPACA_ENDPOINT",
                    Some("https://api.alpaca.markets".to_string()),
                ),
                ("LIVE_ALPACA_KEY", None),
                ("LIVE_ALPACA_SECRET", Some("secret".to_string())),
            ],
            || {
                assert!(AlpacaConfig::from_env(AlpacaEnv::Live).is_err());
            },
        );
    }

    // ── from_credentials ─────────────────────────────────────────────────────

    fn make_creds(endpoint: &str, env: AlpacaEnv) -> ResolvedCredentials {
        ResolvedCredentials {
            endpoint: endpoint.into(),
            key: "AKTEST000".into(),
            secret: "secret000".into(),
            env,
        }
    }

    #[test]
    fn from_credentials_empty_endpoint_returns_error() {
        let creds = make_creds("", AlpacaEnv::Paper);
        let result = AlpacaConfig::from_credentials(creds);
        assert!(result.is_err(), "empty endpoint should produce an error");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("endpoint must not be empty"),
            "error message should mention the empty endpoint"
        );
    }

    #[test]
    fn from_credentials_live_appends_v2() {
        let creds = make_creds("https://api.alpaca.markets", AlpacaEnv::Live);
        let cfg = AlpacaConfig::from_credentials(creds).unwrap();
        assert_eq!(cfg.base_url, "https://api.alpaca.markets/v2");
        assert_eq!(cfg.env, AlpacaEnv::Live);
        assert!(!cfg.dry_run);
    }

    #[test]
    fn from_credentials_paper_uses_endpoint_as_is() {
        let creds = make_creds("https://paper-api.alpaca.markets", AlpacaEnv::Paper);
        let cfg = AlpacaConfig::from_credentials(creds).unwrap();
        assert_eq!(cfg.base_url, "https://paper-api.alpaca.markets");
        assert!(
            !cfg.base_url.ends_with("/v2"),
            "paper endpoint must not gain /v2"
        );
        assert_eq!(cfg.env, AlpacaEnv::Paper);
    }

    #[test]
    fn from_credentials_trims_trailing_slash_live() {
        let creds = make_creds("https://api.alpaca.markets/", AlpacaEnv::Live);
        let cfg = AlpacaConfig::from_credentials(creds).unwrap();
        assert_eq!(
            cfg.base_url, "https://api.alpaca.markets/v2",
            "trailing slash must be stripped before /v2 is appended"
        );
    }

    #[test]
    fn from_credentials_trims_trailing_slash_paper() {
        let creds = make_creds("https://paper-api.alpaca.markets/", AlpacaEnv::Paper);
        let cfg = AlpacaConfig::from_credentials(creds).unwrap();
        assert_eq!(
            cfg.base_url, "https://paper-api.alpaca.markets",
            "trailing slash must be stripped from paper endpoint"
        );
    }

    #[test]
    fn from_credentials_preserves_key_and_secret() {
        let creds = ResolvedCredentials {
            endpoint: "https://api.alpaca.markets".into(),
            key: "MY_KEY".into(),
            secret: "MY_SECRET".into(),
            env: AlpacaEnv::Live,
        };
        let cfg = AlpacaConfig::from_credentials(creds).unwrap();
        assert_eq!(cfg.key, "MY_KEY");
        assert_eq!(cfg.secret, "MY_SECRET");
    }
}

/// Credentials resolved from env vars, OS keychain, or an interactive prompt.
///
/// Produced by the binary-crate's `credentials::resolve()` and consumed by
/// [`AlpacaConfig::from_credentials`].
#[derive(Debug, Clone)]
pub struct ResolvedCredentials {
    /// Raw endpoint URL (without `/v2` normalisation).
    ///
    /// For live trading this is typically `https://api.alpaca.markets`.
    /// For paper trading it is `https://paper-api.alpaca.markets/v2` (already
    /// contains `/v2` — [`AlpacaConfig::from_credentials`] handles both forms).
    pub endpoint: String,
    /// Alpaca API key ID (`APCA-API-KEY-ID` header value).
    pub key: String,
    /// Alpaca API secret key (`APCA-API-SECRET-KEY` header value).
    pub secret: String,
    /// Which trading environment these credentials belong to.
    pub env: AlpacaEnv,
}

/// Selects which Alpaca trading environment to connect to.
#[derive(Debug, Clone, PartialEq)]
pub enum AlpacaEnv {
    /// Alpaca paper-trading environment — uses simulated funds with no real money.
    Paper,
    /// Alpaca live-trading environment — uses real funds; handle with care.
    Live,
}

/// Runtime configuration loaded from environment variables.
///
/// Construct via [`AlpacaConfig::from_env`]; the individual fields are
/// exposed so downstream code can read the resolved values without
/// re-parsing the environment.
#[derive(Debug, Clone)]
pub struct AlpacaConfig {
    /// REST base URL including `/v2`, without a trailing slash.
    ///
    /// Example: `https://paper-api.alpaca.markets/v2`
    pub base_url: String,
    /// Alpaca API key ID (`APCA-API-KEY-ID` header value).
    pub key: String,
    /// Alpaca API secret key (`APCA-API-SECRET-KEY` header value).
    pub secret: String,
    /// Which environment (paper / live) this config targets.
    pub env: AlpacaEnv,
    /// When `true`, order submissions are simulated locally and never sent to
    /// the Alpaca API. All read-only calls (account, positions, watchlist …)
    /// still use live or paper data from the selected environment.
    pub dry_run: bool,
}

impl AlpacaConfig {
    /// Load configuration from environment variables for the specified environment.
    ///
    /// Only the variables for the requested environment are read and validated —
    /// the opposing set is ignored entirely. The environment is determined by the
    /// `--paper` CLI flag: pass [`AlpacaEnv::Paper`] when `--paper` is supplied,
    /// or [`AlpacaEnv::Live`] otherwise (the default).
    ///
    /// | Env | Variables required |
    /// |-----|--------------------|
    /// | [`AlpacaEnv::Paper`] | `PAPER_ALPACA_ENDPOINT`, `PAPER_ALPACA_KEY`, `PAPER_ALPACA_SECRET` |
    /// | [`AlpacaEnv::Live`]  | `LIVE_ALPACA_ENDPOINT`,  `LIVE_ALPACA_KEY`,  `LIVE_ALPACA_SECRET`  |
    ///
    /// Returns an error if any required variable for the chosen environment is missing.
    pub fn from_env(env: AlpacaEnv) -> Result<Self> {
        match env {
            AlpacaEnv::Live => {
                let endpoint = std::env::var("LIVE_ALPACA_ENDPOINT")
                    .context("LIVE_ALPACA_ENDPOINT not set")?;
                let key = std::env::var("LIVE_ALPACA_KEY").context("LIVE_ALPACA_KEY not set")?;
                let secret =
                    std::env::var("LIVE_ALPACA_SECRET").context("LIVE_ALPACA_SECRET not set")?;
                // Live endpoint does not include /v2
                let base_url = format!("{}/v2", endpoint.trim_end_matches('/'));
                Ok(Self {
                    base_url,
                    key,
                    secret,
                    env: AlpacaEnv::Live,
                    dry_run: false,
                })
            }
            AlpacaEnv::Paper => {
                let endpoint = std::env::var("PAPER_ALPACA_ENDPOINT")
                    .context("PAPER_ALPACA_ENDPOINT not set")?;
                let key = std::env::var("PAPER_ALPACA_KEY").context("PAPER_ALPACA_KEY not set")?;
                let secret =
                    std::env::var("PAPER_ALPACA_SECRET").context("PAPER_ALPACA_SECRET not set")?;
                // Paper endpoint already includes /v2
                let base_url = endpoint.trim_end_matches('/').to_string();
                Ok(Self {
                    base_url,
                    key,
                    secret,
                    env: AlpacaEnv::Paper,
                    dry_run: false,
                })
            }
        }
    }

    /// Build configuration from pre-resolved credentials.
    ///
    /// Applies the same URL normalisation as [`AlpacaConfig::from_env`]:
    /// live endpoints have `/v2` appended; paper endpoints are used as-is
    /// (with any trailing slash stripped).
    ///
    /// # Errors
    ///
    /// Returns an error if the endpoint string is empty.
    pub fn from_credentials(creds: ResolvedCredentials) -> Result<Self> {
        if creds.endpoint.is_empty() {
            return Err(anyhow::anyhow!("endpoint must not be empty"));
        }
        let base_url = match creds.env {
            AlpacaEnv::Live => {
                format!("{}/v2", creds.endpoint.trim_end_matches('/'))
            }
            AlpacaEnv::Paper => creds.endpoint.trim_end_matches('/').to_string(),
        };
        Ok(Self {
            base_url,
            key: creds.key,
            secret: creds.secret,
            env: creds.env,
            dry_run: false,
        })
    }

    /// Set the dry-run flag, consuming and returning `self`.
    ///
    /// When `dry_run` is `true`, order submission calls will be intercepted
    /// and simulated locally without contacting the Alpaca API.
    ///
    /// ```
    /// # use alpaca_trader_rs::config::{AlpacaConfig, AlpacaEnv};
    /// let config = AlpacaConfig {
    ///     base_url: "http://localhost".into(),
    ///     key: "k".into(),
    ///     secret: "s".into(),
    ///     env: AlpacaEnv::Paper,
    ///     dry_run: false,
    /// }.with_dry_run(true);
    /// assert!(config.dry_run);
    /// ```
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Returns a short uppercase label for the current environment.
    ///
    /// Returns `"PAPER"` or `"LIVE"`. Useful for status-bar display.
    pub fn env_label(&self) -> &'static str {
        match self.env {
            AlpacaEnv::Paper => "PAPER",
            AlpacaEnv::Live => "LIVE",
        }
    }
}
