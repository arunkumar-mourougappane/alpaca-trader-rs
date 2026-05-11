//! Tiered credential resolution for Alpaca API keys.
//!
//! Resolution order (highest → lowest priority):
//!
//! 1. **`ALPACA_API_KEY` + `ALPACA_API_SECRET`** — unified pair; ideal for CI,
//!    Docker, and systemd where a single key pair is configured.
//! 2. **`{ENV}_ALPACA_KEY` + `{ENV}_ALPACA_SECRET`** — per-environment vars
//!    (`LIVE_*` or `PAPER_*`); developer `.env` files and multi-environment setups.
//! 3. **OS-native keychain** (macOS Keychain Access, Windows Credential Store,
//!    Linux kernel keyutils) — no C-library dependency on any platform.
//! 4. **Interactive TTY prompt** via `rpassword` — runs once on first launch
//!    and offers to persist credentials in the OS keychain.
//!
//! Call [`resolve`] **before** `enable_raw_mode()` — it may print to stderr
//! and read from stdin.

use std::io::{self, BufRead as _, IsTerminal as _};

use anyhow::{bail, Result};

use crate::config::{AlpacaEnv, ResolvedCredentials};

const SERVICE: &str = "alpaca-trader-rs";

const DEFAULT_LIVE_ENDPOINT: &str = "https://api.alpaca.markets";
const DEFAULT_PAPER_ENDPOINT: &str = "https://paper-api.alpaca.markets/v2";

/// Resolve credentials for `env` using the tiered lookup strategy.
///
/// Must be called **before** `enable_raw_mode()`.
pub fn resolve(env: AlpacaEnv) -> Result<ResolvedCredentials> {
    let (prefix, kr_prefix, default_ep) = match env {
        AlpacaEnv::Live => ("LIVE_ALPACA", "live", DEFAULT_LIVE_ENDPOINT),
        AlpacaEnv::Paper => ("PAPER_ALPACA", "paper", DEFAULT_PAPER_ENDPOINT),
    };
    let env_label = match env {
        AlpacaEnv::Live => "live",
        AlpacaEnv::Paper => "paper",
    };

    let endpoint =
        std::env::var(format!("{prefix}_ENDPOINT")).unwrap_or_else(|_| default_ep.to_string());

    // ── Step 1a: unified env vars (ALPACA_API_KEY / ALPACA_API_SECRET) ─────────
    // These take priority and work regardless of live/paper mode — ideal for CI,
    // Docker, and systemd where a single key pair is configured.
    let unified_key = std::env::var("ALPACA_API_KEY")
        .ok()
        .filter(|s| !s.is_empty());
    let unified_secret = std::env::var("ALPACA_API_SECRET")
        .ok()
        .filter(|s| !s.is_empty());

    if let (Some(key), Some(secret)) = (unified_key, unified_secret) {
        tracing::debug!(
            env = env_label,
            "credentials loaded from ALPACA_API_KEY / ALPACA_API_SECRET"
        );
        return Ok(ResolvedCredentials {
            endpoint,
            key,
            secret,
            env,
        });
    }

    // ── Step 1b: per-environment env vars ({ENV}_ALPACA_KEY / _SECRET) ─────────
    let env_key = std::env::var(format!("{prefix}_KEY"))
        .ok()
        .filter(|s| !s.is_empty());
    let env_secret = std::env::var(format!("{prefix}_SECRET"))
        .ok()
        .filter(|s| !s.is_empty());

    if let (Some(key), Some(secret)) = (env_key, env_secret) {
        tracing::debug!(
            env = env_label,
            "credentials loaded from environment variables"
        );
        return Ok(ResolvedCredentials {
            endpoint,
            key,
            secret,
            env,
        });
    }

    // ── Step 2: OS keychain ────────────────────────────────────────────────────
    // keychain_usable is true when the keychain backend is present and healthy.
    // On unsupported platforms the entire block is compiled away.
    let mut keychain_usable = false;

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    match try_keychain_load(kr_prefix) {
        Ok(Some((key, secret))) => {
            tracing::debug!(env = env_label, "credentials loaded from OS keychain");
            return Ok(ResolvedCredentials {
                endpoint,
                key,
                secret,
                env,
            });
        }
        Ok(None) => {
            // Not stored yet — fall through to interactive prompt.
            keychain_usable = true;
        }
        Err(KeychainStatus::Unavailable(msg)) => {
            eprintln!(
                "Warning: OS keychain unavailable ({msg}) — \
                 credentials will not be persisted."
            );
            // keychain_usable remains false
        }
        Err(KeychainStatus::Hard(e)) => return Err(e),
    }

    // Suppress unused-variable warning on platforms without keychain support.
    let _ = keychain_usable;

    // ── Step 3: interactive TTY prompt ─────────────────────────────────────────
    if !io::stdin().is_terminal() {
        bail!(
            "No {env_label} Alpaca credentials found and no interactive terminal is available.\n\
             Set {prefix}_KEY and {prefix}_SECRET in your environment or .env file."
        );
    }

    eprintln!();
    eprintln!("No {env_label} Alpaca credentials found.");
    if matches!(env, AlpacaEnv::Live) {
        eprintln!("⚠️  Live trading uses real money. Proceed with care.");
    }
    eprintln!("Visit https://app.alpaca.markets to generate API credentials.");
    eprintln!();

    let key_prompt = format!("{} API Key   (APCA-API-KEY-ID): ", env_label.to_uppercase());
    let secret_prompt = format!("{} API Secret:                  ", env_label.to_uppercase());

    let key = rpassword::prompt_password(key_prompt)?;
    let secret = rpassword::prompt_password(secret_prompt)?;

    if key.trim().is_empty() || secret.trim().is_empty() {
        bail!("API key and secret must not be empty.");
    }

    // ── Step 4: offer to save to keychain ─────────────────────────────────────
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    if keychain_usable {
        offer_keychain_save(kr_prefix, &key, &secret);
    }

    eprintln!();
    Ok(ResolvedCredentials {
        endpoint,
        key,
        secret,
        env,
    })
}

/// Delete stored keychain credentials for `env` and exit.
///
/// Prints a status message to stderr. On platforms without keychain support,
/// prints an informational message and returns without error.
pub fn reset(env: AlpacaEnv) {
    let (kr_prefix, env_label) = match env {
        AlpacaEnv::Live => ("live", "live"),
        AlpacaEnv::Paper => ("paper", "paper"),
    };

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        let key_user = format!("{kr_prefix}-api-key");
        let secret_user = format!("{kr_prefix}-api-secret");
        let mut removed = 0u8;
        let mut any_error = false;

        for user in [&key_user, &secret_user] {
            match keyring::Entry::new(SERVICE, user) {
                Ok(entry) => match entry.delete_credential() {
                    Ok(()) => removed += 1,
                    Err(keyring::Error::NoEntry) => {}
                    Err(e) => {
                        eprintln!("Warning: could not remove {user} from keychain: {e}");
                        any_error = true;
                    }
                },
                Err(e) => {
                    eprintln!("Warning: keychain init error for {user}: {e}");
                    any_error = true;
                }
            }
        }

        if any_error {
            eprintln!("Some entries could not be removed. You may need to remove them manually.");
        } else if removed == 0 {
            eprintln!("No {env_label} credentials found in keychain (nothing to remove).");
        } else {
            eprintln!("✓ {env_label} credentials removed from keychain.");
        }
        return;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = kr_prefix;
        eprintln!("Keychain storage is not supported on this platform — nothing to reset.");
        eprintln!(
            "To clear credentials, unset the {}_ALPACA_KEY / {}_ALPACA_SECRET \
             environment variables.",
            env_label.to_uppercase(),
            env_label.to_uppercase()
        );
    }
}

// ── Keychain helpers (compiled only on supported platforms) ───────────────────

/// Distinguishes between "entry not found" and a hard backend error.
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
enum KeychainStatus {
    /// The keychain is locked or the backend is unavailable.
    Unavailable(String),
    /// An unexpected hard error occurred.
    Hard(anyhow::Error),
}

/// Try to read a key+secret pair from the OS keychain.
///
/// Returns:
/// - `Ok(Some(_))` — credentials found
/// - `Ok(None)` — entry absent (`NoEntry`); first-run, safe to prompt
/// - `Err(KeychainStatus::Unavailable)` — keychain locked/inaccessible; warn and prompt without saving
/// - `Err(KeychainStatus::Hard)` — unexpected error; propagate to caller
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn try_keychain_load(prefix: &str) -> Result<Option<(String, String)>, KeychainStatus> {
    let key = load_one_entry(&format!("{prefix}-api-key"))?;
    let secret = load_one_entry(&format!("{prefix}-api-secret"))?;
    match (key, secret) {
        (Some(k), Some(s)) => Ok(Some((k, s))),
        _ => Ok(None), // one or both absent — first run
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn load_one_entry(user: &str) -> Result<Option<String>, KeychainStatus> {
    let entry = keyring::Entry::new(SERVICE, user)
        .map_err(|e| KeychainStatus::Hard(anyhow::anyhow!("keyring init error: {e}")))?;

    match entry.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(keyring::Error::NoStorageAccess(e) | keyring::Error::PlatformFailure(e)) => {
            Err(KeychainStatus::Unavailable(e.to_string()))
        }
        Err(e) => Err(KeychainStatus::Hard(anyhow::anyhow!(
            "keychain read error: {e}"
        ))),
    }
}

/// Prompt the user to save credentials to the OS keychain.
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn offer_keychain_save(prefix: &str, key: &str, secret: &str) {
    eprint!("Store credentials in OS keychain for future logins? [Y/n]: ");
    let mut answer = String::new();
    if io::stdin().lock().read_line(&mut answer).is_err() {
        return;
    }
    if !(answer.trim().is_empty() || answer.trim().eq_ignore_ascii_case("y")) {
        return;
    }
    match save_keychain_pair(prefix, key, secret) {
        Ok(()) => eprintln!("✓ Credentials saved to keychain."),
        Err(e) => eprintln!("Warning: could not save to keychain: {e}"),
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn save_keychain_pair(prefix: &str, key: &str, secret: &str) -> Result<()> {
    keyring::Entry::new(SERVICE, &format!("{prefix}-api-key"))
        .and_then(|e| e.set_password(key))
        .map_err(|e| anyhow::anyhow!("keychain write error (key): {e}"))?;
    keyring::Entry::new(SERVICE, &format!("{prefix}-api-secret"))
        .and_then(|e| e.set_password(secret))
        .map_err(|e| anyhow::anyhow!("keychain write error (secret): {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: clear all credential env vars around a test closure.
    fn with_no_env_creds<F: FnOnce()>(f: F) {
        temp_env::with_vars(
            [
                ("ALPACA_API_KEY", None::<&str>),
                ("ALPACA_API_SECRET", None::<&str>),
                ("LIVE_ALPACA_KEY", None::<&str>),
                ("LIVE_ALPACA_SECRET", None::<&str>),
                ("PAPER_ALPACA_KEY", None::<&str>),
                ("PAPER_ALPACA_SECRET", None::<&str>),
                ("LIVE_ALPACA_ENDPOINT", None::<&str>),
                ("PAPER_ALPACA_ENDPOINT", None::<&str>),
            ],
            f,
        );
    }

    // ── Unified env var tests ─────────────────────────────────────────────────

    #[test]
    fn unified_vars_resolve_for_live() {
        with_no_env_creds(|| {
            temp_env::with_vars(
                [
                    ("ALPACA_API_KEY", Some("unified-key")),
                    ("ALPACA_API_SECRET", Some("unified-secret")),
                ],
                || {
                    let creds = resolve(AlpacaEnv::Live).unwrap();
                    assert_eq!(creds.key, "unified-key");
                    assert_eq!(creds.secret, "unified-secret");
                    assert!(matches!(creds.env, AlpacaEnv::Live));
                },
            );
        });
    }

    #[test]
    fn unified_vars_resolve_for_paper() {
        with_no_env_creds(|| {
            temp_env::with_vars(
                [
                    ("ALPACA_API_KEY", Some("unified-key")),
                    ("ALPACA_API_SECRET", Some("unified-secret")),
                ],
                || {
                    let creds = resolve(AlpacaEnv::Paper).unwrap();
                    assert_eq!(creds.key, "unified-key");
                    assert_eq!(creds.secret, "unified-secret");
                    assert!(matches!(creds.env, AlpacaEnv::Paper));
                },
            );
        });
    }

    #[test]
    fn unified_vars_use_default_live_endpoint() {
        with_no_env_creds(|| {
            temp_env::with_vars(
                [
                    ("ALPACA_API_KEY", Some("k")),
                    ("ALPACA_API_SECRET", Some("s")),
                ],
                || {
                    let creds = resolve(AlpacaEnv::Live).unwrap();
                    assert_eq!(creds.endpoint, DEFAULT_LIVE_ENDPOINT);
                },
            );
        });
    }

    #[test]
    fn unified_vars_use_default_paper_endpoint() {
        with_no_env_creds(|| {
            temp_env::with_vars(
                [
                    ("ALPACA_API_KEY", Some("k")),
                    ("ALPACA_API_SECRET", Some("s")),
                ],
                || {
                    let creds = resolve(AlpacaEnv::Paper).unwrap();
                    assert_eq!(creds.endpoint, DEFAULT_PAPER_ENDPOINT);
                },
            );
        });
    }

    // ── Per-environment env var tests ─────────────────────────────────────────

    #[test]
    fn live_prefixed_vars_resolve() {
        with_no_env_creds(|| {
            temp_env::with_vars(
                [
                    ("LIVE_ALPACA_KEY", Some("live-key")),
                    ("LIVE_ALPACA_SECRET", Some("live-secret")),
                ],
                || {
                    let creds = resolve(AlpacaEnv::Live).unwrap();
                    assert_eq!(creds.key, "live-key");
                    assert_eq!(creds.secret, "live-secret");
                    assert!(matches!(creds.env, AlpacaEnv::Live));
                },
            );
        });
    }

    #[test]
    fn paper_prefixed_vars_resolve() {
        with_no_env_creds(|| {
            temp_env::with_vars(
                [
                    ("PAPER_ALPACA_KEY", Some("paper-key")),
                    ("PAPER_ALPACA_SECRET", Some("paper-secret")),
                ],
                || {
                    let creds = resolve(AlpacaEnv::Paper).unwrap();
                    assert_eq!(creds.key, "paper-key");
                    assert_eq!(creds.secret, "paper-secret");
                    assert!(matches!(creds.env, AlpacaEnv::Paper));
                },
            );
        });
    }

    #[test]
    fn unified_vars_take_priority_over_prefixed() {
        with_no_env_creds(|| {
            temp_env::with_vars(
                [
                    ("ALPACA_API_KEY", Some("unified-key")),
                    ("ALPACA_API_SECRET", Some("unified-secret")),
                    ("LIVE_ALPACA_KEY", Some("live-key")),
                    ("LIVE_ALPACA_SECRET", Some("live-secret")),
                ],
                || {
                    let creds = resolve(AlpacaEnv::Live).unwrap();
                    assert_eq!(creds.key, "unified-key");
                    assert_eq!(creds.secret, "unified-secret");
                },
            );
        });
    }

    #[test]
    fn custom_endpoint_from_env_is_used() {
        with_no_env_creds(|| {
            temp_env::with_vars(
                [
                    ("ALPACA_API_KEY", Some("k")),
                    ("ALPACA_API_SECRET", Some("s")),
                    ("LIVE_ALPACA_ENDPOINT", Some("https://custom.example.com")),
                ],
                || {
                    let creds = resolve(AlpacaEnv::Live).unwrap();
                    assert_eq!(creds.endpoint, "https://custom.example.com");
                },
            );
        });
    }

    // ── Empty value filtering ─────────────────────────────────────────────────

    #[test]
    fn empty_unified_key_falls_through_to_prefixed() {
        with_no_env_creds(|| {
            temp_env::with_vars(
                [
                    ("ALPACA_API_KEY", Some("")),
                    ("ALPACA_API_SECRET", Some("secret")),
                    ("LIVE_ALPACA_KEY", Some("live-key")),
                    ("LIVE_ALPACA_SECRET", Some("live-secret")),
                ],
                || {
                    let creds = resolve(AlpacaEnv::Live).unwrap();
                    assert_eq!(creds.key, "live-key");
                },
            );
        });
    }

    #[test]
    fn empty_unified_secret_falls_through_to_prefixed() {
        with_no_env_creds(|| {
            temp_env::with_vars(
                [
                    ("ALPACA_API_KEY", Some("some-key")),
                    ("ALPACA_API_SECRET", Some("")),
                    ("LIVE_ALPACA_KEY", Some("live-key")),
                    ("LIVE_ALPACA_SECRET", Some("live-secret")),
                ],
                || {
                    let creds = resolve(AlpacaEnv::Live).unwrap();
                    assert_eq!(creds.key, "live-key");
                },
            );
        });
    }

    #[test]
    fn live_prefixed_vars_not_used_for_paper() {
        with_no_env_creds(|| {
            temp_env::with_vars(
                [
                    ("LIVE_ALPACA_KEY", Some("live-key")),
                    ("LIVE_ALPACA_SECRET", Some("live-secret")),
                    ("PAPER_ALPACA_KEY", Some("paper-key")),
                    ("PAPER_ALPACA_SECRET", Some("paper-secret")),
                ],
                || {
                    let creds = resolve(AlpacaEnv::Paper).unwrap();
                    assert_eq!(creds.key, "paper-key");
                    assert_eq!(creds.secret, "paper-secret");
                },
            );
        });
    }
}
