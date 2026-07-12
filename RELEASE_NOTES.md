# Release Notes — v0.8.0

**Release date:** 2026-07-12
**MSRV:** Rust 1.88+
**Previous release:** [v0.7.1](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.7.1)

---

## Overview

v0.8.0 is a major feature release focused on order flexibility, watchlist alerting, and in-app configuration. The headline changes are:

1. **Bracket Orders** — submit entry + take-profit + stop-loss legs atomically from Order Entry
2. **Extended Order Types** — Stop, Stop-Limit, and Trailing Stop join Market/Limit, plus an Extended Hours flag
3. **Watchlist Price Alerts** — set an above/below threshold per symbol; crossing it flashes the status bar and rings the terminal bell, and alerts persist across restarts
4. **In-App Preferences Modal** — edit every setting, including live/paper API credentials against the OS keychain, without leaving the app
5. **Configurable Chart Marker** — choose braille, dot, block, bar, or half-block rendering for all charts
6. **Dynamic Intraday Chart Labels** — x-axis end label reflects the last received bar; y-axis shows min/max price
7. **Stream Reconnect Indicator** — header distinguishes loading, reconnecting (with attempt count), and permanently offline per stream
8. **Unified Symbol/Position Detail Modal** — both detail modals now share one implementation and have full feature parity
9. **License simplified to MIT-only**
10. **`.env` loads automatically in debug builds only** — no feature flag needed; release builds are unaffected

Test count grows from **800 → 1254 tests**.

---

## What's New

### Bracket Orders

Order Entry gains a **Bracket** checkbox, available for Market and Limit orders on BUY/SELL (not SellShort) sides.
Enabling it reveals **Take Profit $** and **Stop Loss $** fields, plus an optional **Stop Loss Limit $** (leave blank for a market SL leg, or fill it in for a stop-limit SL leg).
All three legs submit atomically via `order_class=bracket`. Directional validation catches an invalid TP/SL relative to the entry price before submission.

### Extended Order Types

The order Type field now cycles through **Market, Limit, Stop, Stop-Limit, and Trailing Stop** (by price or percent), replacing the old binary Market/Limit toggle. An **Extended Hours** flag is available for eligible Limit + Day orders.

### Watchlist Price Alerts

Press `A` on a Watchlist row to set an above/below price threshold. When a live quote crosses it, the status bar flashes and the terminal bell rings; a `🔔` marker appears next to symbols with an active alert.
Alerts — including whether they've already fired — are saved to `config.toml`, so a restart won't replay an already-acknowledged crossing.
`Shift-C` clears all configured alerts at once, gated behind a confirmation modal when `confirm_watchlist_remove` is enabled.

### In-App Preferences Modal (`P`)

A full-screen Preferences editor covers App, UI, Stream, Notifications, Safety, Proxy, and Credentials sections. Changes apply on `Ctrl-S`; `Esc` discards.
The **Credentials** section manages live and paper Alpaca API key/secret pairs independently, writing to the OS-native keychain (macOS Keychain / Windows Credential Store / Linux keyutils) — values are never written to `config.toml`.

### Configurable Chart Marker

A new `chart_marker` preference (`braille` / `dot` / `block` / `bar` / `half_block`) controls the rendering style of every chart in the app, set from the Preferences UI section.

### Dynamic Intraday Chart Labels

The intraday chart's x-axis end label now reflects the timestamp of the last received bar (e.g. `11:47` mid-session) instead of always showing `16:00`. Y-axis min/max price labels were added so the visible price range is readable without opening the crosshair tooltip.

### Stream Reconnect Indicator

The dashboard header now differentiates three states per stream: initial loading, actively reconnecting (with attempt count), and permanently offline — replacing a single ambiguous warning badge.

### Unified Symbol Detail / Position Detail Modal

Both modals now render through a single shared implementation. Position Detail gained OHLCV, crosshair, and asset-flag rows it previously lacked; Symbol Detail gained the position-summary and open-orders panes when the symbol is held.

---

## Bug Fixes

- **Zero prices during closed-market hours** — watchlist, positions, the equity chart, and detail modals now filter out non-positive live quotes and fall back to the last known REST price instead of showing `$0.00` or a false equity cliff.
- **Windows double-input on key release** — Crossterm key-release events are now ignored, fixing duplicate keystrokes on Windows terminals.
- **Windows keychain prompt stdin hang** — the non-echoed y/n keychain-save prompt now uses `rpassword::read_password()` uniformly across platforms instead of a Windows-only code path that could hang.
- **Proxy config erased on exit** — `config.toml`'s `[proxy]` section (`http`, `socks5`, `no_proxy`) now round-trips correctly instead of being silently overwritten with a blank template on clean exit.
- **Dotted ticker symbols (e.g. `BRK.B`) dropped from alerts on reload** — the `[price_alerts]` TOML key is now quoted, avoiding a dotted-key misparse.

---

## Changed

- **License: MIT-only** — the Apache-2.0 dual-license option has been dropped. `Cargo.toml`, `LICENSE.md`, `README.md`, `CONTRIBUTING.md`, and `docs/licensing.md` are updated accordingly; `LICENSE-APACHE` has been removed. If you depended on the Apache-2.0 terms, only MIT applies going forward.
- **`.env` loading is debug-build only** — `.env` now loads automatically in `cargo build` (debug) without any feature flag. Release builds — including installed/pre-compiled binaries — do **not** read `.env`; use environment variables, the OS keychain, or the interactive credential prompt instead.

---

## Documentation

- **`docs/architecture.md`** — directory tree now includes the previously undocumented `src/handlers/` module (REST polling, command dispatch, input event stream).
- **`docs/ui-mockups.md`** — Order Entry mockup updated with bracket/extended-type fields; Symbol/Position Detail mockups merged and updated with dynamic axis labels; Preferences Credentials section mockup added; Watchlist alert UI documented.
- **`docs/future-features.md`** — Price Alerts marked ✅ Implemented; related-issues table extended to cover all v0.8.0 issues.
- **`docs/testing.md`** — test counts and binary breakdown updated to match the current suite.

---

## Tests

**1254 tests total** (up from 800 in v0.7.1):

| Scope | Count |
|---|---|
| Library (`src/lib.rs`: `types`, `config`, `stream`, `prefs`, `logging`) | 134 |
| App (`src/main.rs`: `app`, `update`, `input/*`, `ui/*`, `handlers/*`) | 1090 |
| HTTP integration (`tests/client_tests.rs`) | 29 |
| Doc-tests | 1 |

---

## Upgrade Notes

- If you rely on `.env` for credentials in a **release** build or installed binary, switch to environment variables, the OS keychain (`P` → Credentials, or the first-run prompt), or a `.env` loaded manually before launch — release builds no longer read `.env` automatically.
- If your tooling references the Apache-2.0 license text, note the project is now MIT-only.
- Existing `config.toml` files are read without modification; new keys (`chart_marker`, `[price_alerts]`, Preferences sections) default sensibly if absent.

---

## Getting Started

```bash
git clone https://github.com/arunkumar-mourougappane/alpaca-trader-rs
cd alpaca-trader-rs

./run.sh --paper   # paper trading (recommended for first run)
./run.sh           # live trading
```

See [README.md](README.md) for full setup and
[docs/credentials-setup.md](docs/credentials-setup.md) for API key setup.
