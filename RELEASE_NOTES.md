# Release Notes — v0.5.0

**Release date:** 2026-05-18
**MSRV:** Rust 1.88+
**Previous release:** [v0.4.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.4.0)

---

## Overview

v0.5.0 is a quality, portability, and UX release. The five headline changes are:

1. **`--dry-run` mode** — test order workflows without sending a single real order.
2. **Persistent user preferences** — theme and environment settings survive restarts.
3. **Runtime colour-theme switching** (`T` key) — switch themes live and have the choice remembered.
4. **Windows support** — full CI build, test, and code-coverage matrix on `windows-latest`.
5. **Braille line charts** — sharper double-resolution equity and intraday charts.

Four bug fixes land alongside: the terminal resize no-op, paper-trading watchlist messaging,
market-closed price data, and a Windows clippy warning in the logging module.

Test count grows from **327 → 449 tests**.

---

## What's New

### `--dry-run` Mode

Pass `--dry-run` to intercept all order submissions before they reach Alpaca. The order is shown
as `[DRY-RUN]` in the status bar and logged, but no network request is made. Read-only operations
(account data, positions, watchlist, quotes) are unaffected.

```bash
alpaca-trader --paper --dry-run   # safe sandbox — simulated env + no real orders
alpaca-trader --dry-run           # live data, intercepted orders
```

The dry-run state is visible in the header so it's impossible to forget it is active.

### Persistent User Preferences

A `prefs.toml` file is created on first run in the OS config directory:

| Platform | Path |
|---|---|
| Linux / macOS | `~/.config/alpaca-trader/prefs.toml` |
| Windows | `%APPDATA%\alpaca-trader\prefs.toml` |

Current persisted settings:

| Key | Default | Description |
|---|---|---|
| `app.default_env` | `"live"` | Which environment to connect to when `--paper` is not passed |
| `ui.theme` | `"default"` | Active colour theme name |

The file is created automatically with defaults on first run and never requires manual editing.

### Runtime Colour-Theme Switching (`T` key)

Press `T` at any time to cycle through the available colour themes. The selection is written to
`prefs.toml` immediately so it persists across restarts. The key binding is documented in the Help
overlay (`?`).

### Windows Support

The full CI pipeline now runs on `windows-latest` in addition to the existing `ubuntu-latest` and
`macos-latest` runners:

- **Build** — `cargo build --release` succeeds on Windows with no feature flags needed
- **Tests** — all 449 tests pass on Windows (platform-conditional tests skip unix-only paths)
- **Code coverage** — `cargo-llvm-cov` runs on both Linux and Windows; both reports are uploaded to
  Codecov with separate `Linux` / `Windows` flags and merged into the coverage badge

### Braille Line Charts

The equity sparkline in the Account panel and the intraday price chart in the Symbol Detail modal
have been upgraded from ratatui's `Sparkline` widget to a full `Chart` with a braille canvas.
Braille cells pack 2×4 dots per cell, giving roughly double the effective resolution at the same
terminal width.

---

## Bug Fixes

| Fix | Details |
|---|---|
| Terminal resize silently ignored | `Event::Resize` now sets `needs_redraw = true`; the main loop draws an extra frame before blocking for the next event, so layout adapts immediately instead of waiting up to 250 ms for the next tick |
| Paper-trading watchlist shows blank panel | A persistent info message ("Watchlists unavailable in paper mode") is now shown when the paper endpoint returns a 422, matching the behaviour users expect from the `--paper` flag |
| Price and Change% blank when market is closed | Watchlist rows now fall back to REST snapshot data for price and daily change when the market is closed and live quotes are unavailable |
| `MessageVisitor` dead-code warning on Windows | `struct MessageVisitor` and all syslog helpers are gated to `#[cfg(unix)]`; Windows builds are now clippy-clean with `-D warnings` |

---

## Tests

**449 tests total** (up from 327 in v0.4.0):

| Scope | Count | Notable additions |
|---|---|---|
| Library (`src/stream/`, `src/types.rs`, `src/config.rs`) | 99 | Logging module ~90% coverage; client coverage improvements |
| Binary crate (`src/app.rs`, `src/update.rs`, `src/handlers/`, `src/ui/`) | 320 | Resize handler, theme switching, prefs persistence, UI renderer tests |
| HTTP integration (`tests/client_tests.rs`) | 29 | Additional REST method coverage |
| Doc-tests | 1 | `with_dry_run` example |

---

## No Breaking Changes

v0.5.0 is fully backwards-compatible with v0.4.0. All credential resolution, CLI flags,
environment variables, and library API are unchanged. The only new required file is `prefs.toml`,
which is created automatically with safe defaults.

---

## Getting Started

```bash
git clone https://github.com/arunkumar-mourougappane/alpaca-trader-rs
cd alpaca-trader-rs

# First run — app prompts for credentials and offers to save to keychain
./run.sh --paper   # paper trading (simulated funds — recommended for first run)
./run.sh           # live trading  (real money — default)

# Try out dry-run mode without any risk
./run.sh --paper --dry-run
```

Or configure via `.env`:

```bash
cp .env.example .env
# Fill in your credentials — see docs/credentials-setup.md
./run.sh --paper
```

See [README.md](README.md) for full setup options and [docs/credentials-setup.md](docs/credentials-setup.md) for obtaining API keys from the Alpaca dashboard.

