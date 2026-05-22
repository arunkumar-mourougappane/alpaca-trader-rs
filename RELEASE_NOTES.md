# Release Notes — v0.7.0

**Release date:** 2026-05-21
**MSRV:** Rust 1.88+
**Previous release:** [v0.6.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.6.0)

---

## Overview

v0.7.0 is a major feature release focused on interactivity and discoverability. The headline changes are:

1. **Interactive Chart Crosshairs** — keyboard and mouse-driven crosshair tooltips on the equity chart and symbol detail intraday chart
2. **Help Overlay** — `?` brings up a full-screen keyboard shortcut reference
3. **Global Symbol Search** — `Ctrl-F` / `/` opens a floating symbol search from any panel
4. **Position Detail Modal** — `Enter` on a position shows a dedicated modal with intraday chart and P&L
5. **Double-Click Support** — double-clicking a row opens the same detail as `Enter`; outside-click dismisses
6. **Orders Symbol Filter** — `f` activates an inline filter bar to narrow orders by ticker
7. **Column Sorting** — `s`/`S` sorts and reverses both Orders and Positions tables
8. **Equity Date-Range Toggle** — `p` cycles the equity chart between 1D / 1W / 1M / YTD
9. **PDT & Day-Trade Metrics** — Pattern Day Trader flag and day-trade count shown in Account panel
10. **Documentation Overhaul** — architecture, UI mockups, testing, library API, and account management design docs all updated or created

Test count grows from **541 → 800 tests**.

---

## What's New

### Interactive Chart Crosshairs

Both the Account equity chart and the Symbol Detail intraday chart now support interactive crosshairs.
Use `←`/`h` and `→`/`l` to move the crosshair, or click a data point with the mouse.
A tooltip shows the date and value at the selected point. `Esc` clears the crosshair.

### Help Overlay (`?`)

Press `?` from any panel to open a full-screen keyboard shortcut reference.
Every key binding is listed by category — navigation, panels, modals, sorting, filtering, and charts.
Any key press closes the overlay.

### Global Symbol Search (`Ctrl-F` / `/`)

From any non-Watchlist panel, press `Ctrl-F` or `/` to open a floating symbol search input.
Type a ticker symbol and press `Enter` to open the Symbol Detail modal immediately.
`Esc` cancels without navigating.

### Position Detail Modal

`Enter` on a Positions row opens a dedicated `PositionDetail` modal showing an intraday chart,
current P&L (unrealized + realized), quantity, cost basis, and current price.
Press `o` inside the modal to jump directly to Order Entry pre-filled with a SELL order for that symbol.

### Double-Click & Outside-Click

Double-clicking a row in any list panel opens the detail modal for that row, matching the `Enter` key behavior.
Clicking outside an open modal (outside its bounding box) dismisses it.

### Orders Symbol Filter

Press `f` on the Orders panel to activate an inline filter bar at the bottom.
Type a symbol to narrow visible rows in real time. `Enter` or `Esc` closes the filter bar. `F` clears the active filter.

### Column Sorting

`s` cycles the active sort column on both the Orders and Positions tables.
`S` toggles the sort direction (Ascending ↔ Descending).
The selected column header shows a `▲`/`▼` indicator.

### Equity Date-Range Toggle

Press `p` on the Account panel to cycle the equity chart between four ranges:
**1D** (intraday), **1W** (one week), **1M** (one month), and **YTD** (year-to-date).
The selected range is persisted in `~/.config/alpaca-trader-rs/prefs.toml` across restarts.

### PDT Flag and Day-Trade Metrics

The Account panel now shows `daytrade_count` and the Pattern Day Trader (`pdt`) flag.
`short_market_value` is also fetched and displayed alongside `long_market_value`.

---

## Bug Fixes

- **Orders Enter key** — `Enter` now only confirms the filter bar when active; previously the docs incorrectly described an Orders detail modal that never existed.
- **Crosshair out-of-bounds** — fixed a panic when the crosshair index exceeded the chart data length after a data refresh.

---

## Documentation

A complete documentation overhaul was performed in this release:

- **`docs/architecture.md`** — technology stack corrected; `apca` and `ratatui-textarea` removed (never used); 4 missing crates added; `AlpacaClient` method list corrected (13 real methods); REST endpoints table corrected; data-flow diagram updated.
- **`docs/ui-mockups.md`** — Orders footer corrected; Position Detail modal section added; per-modal keyboard tables added.
- **`docs/testing.md`** — test layout updated with `src/input/` directory; count updated 101 → 800.
- **`docs/future-features.md`** — 7 issues marked ✅ Implemented with PR links.
- **`docs/library.md`** — new: full library API reference with 6 usage examples.
- **`docs/account-management.md`** — new: design spec for in-app credential entry, multi-profile support, and settings modal (Phases 1–3).

---

## Internal / Refactoring

- **`src/input/`** — input handlers split into per-panel modules (`account.rs`, `modal.rs`, `orders.rs`, `positions.rs`, `watchlist.rs`, `mouse.rs`), each with full unit-test coverage.
- **Test helpers** — `render_test_frame()` and similar helpers in `src/ui/test_helpers.rs` used consistently across all UI render tests.
- **Coverage** — every `match` arm, `if let`, and modal variant is now covered by a dedicated test.

---

## Tests

**800 tests total** (up from 541 in v0.6.0):

| Scope | Count |
|---|---|
| Library (`src/types.rs`, `src/config.rs`, `src/prefs.rs`, `src/client.rs`) | ~120 |
| Input handlers (`src/input/`) | ~180 |
| UI render (`src/ui/`) | ~370 |
| HTTP integration (`tests/client_tests.rs`) | ~100 |
| Doc-tests | ~30 |

---

## No Breaking Changes

v0.7.0 is fully backwards-compatible with v0.6.0. All CLI flags,
credential resolution, environment variables, and library API are unchanged.
Existing `prefs.toml` files are read without modification; the new `equity_range` key
defaults to `1D` if absent.

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
