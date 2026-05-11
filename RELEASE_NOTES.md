# Release Notes — v0.3.0

**Release date:** unreleased
**MSRV:** Rust 1.88+
**Previous release:** [v0.2.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.2.0)

---

## Overview

v0.3.0 overhauls credential management. The app no longer requires a `.env` file to run — on
first launch it prompts for API keys via the terminal and offers to save them to the OS native
keychain, so subsequent runs need no configuration at all. For CI, containers, and power users the
existing env-var approach is preserved and extended with a new unified `ALPACA_API_KEY` /
`ALPACA_API_SECRET` pair. The `ALPACA_ENV` env var is retired in favour of a `--paper` CLI flag,
and a new `--reset` flag lets users clear stored keychain entries from the command line.

The test suite grows from **188 → 198 tests**.

---

## What's New

### OS-Native Keychain Credential Storage

Credentials are now stored and retrieved from the platform keychain — no extra software required,
no C-library dependencies:

| Platform | Backend |
|---|---|
| macOS | Keychain Access (`apple-native`) |
| Windows | Credential Store (`windows-native`) |
| Linux | Kernel keyutils (`linux-native`) — cross-compiles cleanly, no `libdbus` |

Graceful degradation when the keychain is unavailable (locked, WSL, headless CI) — the app warns
and continues with session-only credentials.

### Interactive First-Run Provisioning

When no credentials are found in the environment or keychain, `alpaca-trader` prompts for the API
key and secret directly on the terminal (via `rpassword`, which opens `/dev/tty` directly and
is unaffected by stdin redirection). After a successful entry the user is offered the option to
save the credentials to the keychain (default: yes).

```
alpaca-trader --paper
  → Alpaca paper API key: ****
  → Alpaca paper API secret: ****
  → Save to keychain? [Y/n]: Y
  ✓ Paper credentials saved.
```

On subsequent runs the keychain is queried automatically — no further input required.

### `--paper` CLI Flag

`ALPACA_ENV` is retired. The active trading environment is now selected on the command line:

```bash
alpaca-trader           # live account (real money — default)
alpaca-trader --paper   # paper account (simulated funds)
```

`run.sh` accepts the same flags and passes them through to the binary.

### `ALPACA_API_KEY` / `ALPACA_API_SECRET` Unified Env Vars

A single credential pair now works for both environments. This is ideal for CI pipelines,
Docker images, and systemd services where environment-switching happens at the process level:

```bash
export ALPACA_API_KEY=your-key-id
export ALPACA_API_SECRET=your-secret-key
alpaca-trader --paper   # or without --paper for live
```

**Full credential resolution order (highest priority first):**

1. `ALPACA_API_KEY` + `ALPACA_API_SECRET` — unified pair
2. `LIVE_ALPACA_KEY` / `LIVE_ALPACA_SECRET` or `PAPER_ALPACA_KEY` / `PAPER_ALPACA_SECRET` — per-environment (developer `.env` files)
3. OS-native keychain — returning desktop users
4. Interactive TTY prompt — first-run desktop

### `--reset` Flag

Clear stored keychain entries for a given environment without entering the TUI:

```bash
alpaca-trader --reset paper   # removes paper keychain entries
alpaca-trader --reset live    # removes live keychain entries
```

If the credentials for that environment were loaded from a `.env` file or environment variable
(rather than the keychain) the command prints the exact variable names to unset and suggests
which file to edit.

### New Public Library API

| Item | Description |
|---|---|
| `ResolvedCredentials` | Public struct carrying resolved `endpoint`, `key`, `secret`, and `env` |
| `AlpacaConfig::from_credentials(ResolvedCredentials)` | New constructor; applies the same URL normalisation as `from_env` |

---

## ⚠️ Breaking Changes

| Change | Migration |
|---|---|
| **Default environment is now live** (was paper) | Pass `--paper` explicitly if you relied on the old paper default |
| **`ALPACA_ENV` is no longer read** | Remove it from `.env` / shell config; use `--paper` flag instead |
| **`AlpacaConfig::from_env()` now requires an `AlpacaEnv` argument** | Update call sites: `AlpacaConfig::from_env(AlpacaEnv::Paper)` |

---

## Tests

**198 tests total** (up from 188 in v0.2.0):

| Scope | Count | Highlights |
|---|---|---|
| Library (`src/stream/`, `src/types.rs`, `src/config.rs`) | 42 | Serde round-trips, env-var resolution, WebSocket integration tests |
| Binary crate (`src/app.rs`, `src/update.rs`, `src/handlers/`, `src/credentials.rs`) | 137 | State logic, keyboard dispatch, credential resolution (11 new), `--reset` paths |
| HTTP integration (`tests/client_tests.rs`) | 19 | All 11 `AlpacaClient` methods against a `wiremock` mock |

New credential tests cover:

- Unified `ALPACA_API_KEY`/`ALPACA_API_SECRET` for live and paper environments
- Per-env prefixed vars (`LIVE_*`, `PAPER_*`)
- Priority ordering (unified vars beat prefixed vars)
- Custom endpoint override via `LIVE_ALPACA_ENDPOINT` / `PAPER_ALPACA_ENDPOINT`
- Empty value filtering (empty string treated as absent)
- Cross-env isolation (paper vars not visible when resolving live, and vice versa)

---

## Dependencies Added

| Crate | Version | Role |
|---|---|---|
| `rpassword` | 7 | TTY password prompts (reads `/dev/tty` directly) |
| `keyring` | 3 | OS-native keychain with platform-conditional native features |

---

## Upgrade Guide from v0.2.0

1. **Check your default environment.** If you ran `./run.sh` or `alpaca-trader` without flags and expected paper trading, add `--paper` to your command or script.

2. **Remove `ALPACA_ENV`.** If it's in your `.env` file or shell profile, delete the line — it is silently ignored and may cause confusion.

3. **Optionally migrate keys to the keychain.** Run `alpaca-trader --paper` (or without `--paper` for live). If keys are already in `.env` they'll be picked up; the app will not re-prompt. To trigger the save-to-keychain flow, unset the env vars and run again.

4. **Library consumers**: update `AlpacaConfig::from_env()` calls to pass the environment: `AlpacaConfig::from_env(AlpacaEnv::Live)` or `AlpacaConfig::from_env(AlpacaEnv::Paper)`.

---

## Getting Started

```bash
git clone https://github.com/arunkumar-mourougappane/alpaca-trader-rs
cd alpaca-trader-rs

# First run — app will prompt for credentials and offer to save to keychain
./run.sh --paper   # paper trading (simulated funds — recommended for first run)
./run.sh           # live trading  (real money — default)
```

Or configure via `.env`:

```bash
cp .env.example .env
# Fill in your credentials — see docs/credentials-setup.md
./run.sh --paper
```

See [README.md](README.md) for full setup options and [docs/credentials-setup.md](docs/credentials-setup.md) for obtaining API keys from the Alpaca dashboard.

