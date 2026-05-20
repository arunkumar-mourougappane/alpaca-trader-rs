# Release Notes — v0.6.0

**Release date:** 2026-05-19
**MSRV:** Rust 1.88+
**Previous release:** [v0.5.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.5.0)

---

## Overview

v0.6.0 is a UX and real-time data release. The ten headline changes are:

1. **Live WebSocket chart streaming** — intraday charts update with every quote tick, not just once per poll cycle.
2. **Filled Price column in Orders** — `filled_avg_price` is now shown directly in the Orders table.
3. **Order fill notifications** — a status bar flash when a polled order transitions to `filled`.
4. **P&L summary footer row** — aggregate unrealised P&L shown in dollar and percentage form below the Positions table.
5. **Watchlist removal confirmation** — a yes/no modal before `d` deletes a watchlist entry.
6. **Clipboard copy (`c` key)** — copies the selected symbol to the system clipboard from Positions and Watchlist panels.
7. **`gg` / `G` jump navigation** — vim-style jump-to-top / jump-to-bottom in Positions, Orders, and Watchlist.
8. **Refresh spinner and last-updated timestamp** — visual feedback in the header while a REST poll is in flight.
9. **Status bar message queue** — rapid events are queued and shown sequentially instead of overwriting each other.
10. **Internal refactoring** — shared formatting helpers, a render-test helper, and a generic vim-nav trait eliminate ~150 lines of duplication.

Test count grows from **449 → 540 tests**.

---

## What's New

### Live WebSocket Chart Streaming

Intraday equity and position charts are now fed with real-time quote ticks between REST poll cycles. Previously, charts only updated once per 60-second poll. Now every WebSocket quote event advances the chart curve immediately, giving smooth braille updates at market speed.

### Filled Price Column in Orders

The `filled_avg_price` field is now part of the `Order` type and surfaced as a dedicated column in the Orders table. Traders no longer need to open a position detail to confirm the execution price.

### Order Fill Notifications

When a REST poll detects that an order has transitioned to `filled`, a transient status bar message flashes:

```
✓ AAPL order filled at $213.42
```

The notification is queued behind any other pending status messages so nothing is silently dropped.

### P&L Summary Footer Row

A pinned footer row appears below the Positions table showing aggregate unrealised P&L across all open positions:

```
TOTAL    —    —    —    +$1,234.56   +2.34%
```

The footer updates whenever the positions data refreshes.

### Watchlist Removal Confirmation

Pressing `d` on a watchlist entry now opens a confirmation modal:

```
Remove AAPL from watchlist? [y / n]
```

The deletion is only sent to Alpaca after `y` is pressed. Pressing `n` or `Esc` dismisses the modal without any change.

### Clipboard Copy (`c` Key)

Press `c` on any selected row in Positions or Watchlist to copy the row's symbol to the system clipboard. A confirmation message appears in the status bar:

```
Copied AAPL to clipboard
```

On the Orders panel, `c` retains its existing behaviour of cancelling the selected order.

### `gg` / `G` Jump Navigation

Vim-style jump bindings work in all three table panels:

| Key | Action |
|-----|--------|
| `gg` | Jump to first row |
| `G` | Jump to last row |
| `j` / `↓` | Move down one row |
| `k` / `↑` | Move up one row |

### Refresh Spinner and Last-Updated Timestamp

The header now shows a spinning Braille frame while a REST poll is in flight, and the `hh:mm:ss` timestamp of the last successful refresh once the poll completes. This makes it clear whether the displayed data is fresh.

### Status Bar Message Queue

Events such as fill notifications, clipboard confirmations, and errors are enqueued rather than overwriting each other. Messages are shown sequentially and cleared automatically after their display duration elapses.

---

## Refactoring Highlights

- **`src/ui/formatting.rs`** (new) — shared `format_dollar`, `format_price`, `format_pct_ratio`, and `header_cell` helpers; removes duplicate private formatting functions from `positions.rs`, `account.rs`, and `orders.rs`.
- **`src/ui/test_helpers.rs`** (new) — `render_to_string(width, height, fn)` eliminates `TestBackend` boilerplate from every UI module's test section.
- **`ThemeColors::bordered_block`** — single call replaces all inline `Block::default().title().borders(ALL).border_style()` chains.
- **`handle_nav_key` + `SelectionState` trait** — extracted shared vim-nav logic works generically over both `ListState` and `TableState`; removes ~20 duplicated lines from each of the three input handlers.
- **`OrderEntryState::with_side` builder** — replaces manual field mutation in the modal handler.

---

## Tests

**540 tests total** (up from 449 in v0.5.0):

| Scope | Count |
|---|---|
| Library (`src/stream/`, `src/types.rs`, `src/config.rs`) | 100 |
| Binary crate (`src/app.rs`, `src/update.rs`, `src/handlers/`, `src/ui/`) | 410 |
| HTTP integration (`tests/client_tests.rs`) | 29 |
| Doc-tests | 1 |

Coverage additions include live chart streaming handlers, order fill notification detection, P&L footer rendering, watchlist removal modal flow, clipboard keybinding paths, `gg`/`G` navigation, status bar queue ordering, and shared formatting helpers.

---

## No Breaking Changes

v0.6.0 is fully backwards-compatible with v0.5.0. All CLI flags, credential resolution, environment variables, and library API are unchanged.

---

## Getting Started

```bash
git clone https://github.com/arunkumar-mourougappane/alpaca-trader-rs
cd alpaca-trader-rs

./run.sh --paper   # paper trading (recommended for first run)
./run.sh           # live trading
```

Or configure via `.env`:

```bash
cp .env.example .env
# Fill in your credentials — see docs/credentials-setup.md
./run.sh --paper
```

See [README.md](README.md) for full setup options and [docs/credentials-setup.md](docs/credentials-setup.md) for API key setup.
