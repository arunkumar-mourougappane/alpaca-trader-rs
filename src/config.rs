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
                })
            }
        }
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
