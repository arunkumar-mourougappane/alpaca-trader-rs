use anyhow::{anyhow, Context, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum AlpacaEnv {
    Paper,
    Live,
}

#[derive(Debug, Clone)]
pub struct AlpacaConfig {
    /// REST base URL, includes /v2, no trailing slash
    pub base_url: String,
    pub key: String,
    pub secret: String,
    pub env: AlpacaEnv,
}

impl AlpacaConfig {
    pub fn from_env() -> Result<Self> {
        let env_label = std::env::var("ALPACA_ENV").unwrap_or_else(|_| "paper".into());

        match env_label.to_lowercase().as_str() {
            "live" => {
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
            "paper" => {
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
            other => Err(anyhow!(
                "Unknown ALPACA_ENV value: '{}'. Use 'paper' or 'live'.",
                other
            )),
        }
    }

    pub fn env_label(&self) -> &'static str {
        match self.env {
            AlpacaEnv::Paper => "PAPER",
            AlpacaEnv::Live => "LIVE",
        }
    }
}
