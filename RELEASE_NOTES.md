# Release Notes — v0.4.0

**Release date:** 2026-05-12
**MSRV:** Rust 1.88+
**Previous release:** [v0.3.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.3.0)

---

## Overview

v0.4.0 is a UX and information-density release. The Account panel gains Day P&L, Open P&L, and
Account #. The Watchlist replaces Ask/Bid with Volume and Change%. The Symbol Detail modal adds
full OHLCV data and an intraday sparkline. The header correctly identifies pre-market and
after-hours trading sessions. Three new keyboard shortcuts round out the experience: `s` for
SELL SHORT from the Positions panel, `↑`/`↓` for cycling dropdown values in Order Entry, and `A`
for a new About modal. A longstanding bug that left the intraday sparkline permanently on
"Loading…" is fixed.

The test suite grows from **198 → 327 tests**.

---

## What's New

### About Modal (`A`)

A new global `A` key opens an About overlay that shows the app name, version, author contact
details, project and documentation URLs, and license — all embedded at compile time via `env!`
macros so no runtime I/O is needed. Any key press closes it.

```
╔═ About alpaca-trader-rs ══════════════════╗
║                                           ║
║   alpaca-trader-rs  v0.4.0                ║
║                                           ║
║   Alpaca Markets TUI trading terminal     ║
║   and async REST client library.          ║
║                                           ║
║  ── Author ─────────────────────────────  ║
║   Arunkumar Mourougappane                 ║
║   amouroug.dev@gmail.com                  ║
║   github.com/arunkumar-mourougappane      ║
║   anengineersrant.com                     ║
║                                           ║
║  ── Project ────────────────────────────  ║
║   github.com/arunkumar-mourougappane/     ║
║     alpaca-trader-rs                      ║
║   docs.rs/alpaca-trader-rs                ║
║                                           ║
║  ── License ────────────────────────────  ║
║   MIT OR Apache-2.0                       ║
║                                           ║
║              Press any key to close       ║
╚═══════════════════════════════════════════╝
```

### Symbol Detail — OHLCV + Intraday Sparkline + Watchlist Toggle

The Symbol Detail modal (`Enter` on a Watchlist or Positions row) now shows a full OHLCV snapshot
(Open, High, Low, Volume) alongside the existing bid/ask, an intraday 1-minute price sparkline,
and a `w` key to add or remove the symbol from the watchlist without leaving the modal.

The longstanding bug that caused the sparkline to remain on "Loading…" indefinitely is fixed —
intraday bars are now stored and rendered correctly.

### Account Panel — Day P&L, Open P&L, Account #

The Account panel displays two new P&L fields (green if positive, red if negative) and the
account number pulled from the Alpaca account response:

```
Portfolio Value   $125,432.18     Day P&L   +$843.22  (+0.68%)
Buying Power       $48,210.00     Open P&L  +$1,204.50
Cash               $48,210.00     Account # PA1234567
Long Market Value  $77,222.18     Status    ACTIVE
```

### Watchlist — Volume and Change%

The Ask and Bid price columns have been replaced with Volume (shares traded) and Change%
(day-over-day percentage change), giving a more actionable at-a-glance view:

```
Symbol   Name                     Price      Change      Volume
INTC     Intel Corporation        $24.18    -0.42%      45.2M
AMD      Advanced Micro Devices  $142.85    +1.24%      28.7M
```

Change% and Price are colour-coded green (positive) / red (negative).

### Header — PRE-MARKET / AFTER-HOURS State

The market clock in the header now correctly identifies and displays all four session states:

| State | Display |
|---|---|
| Pre-market (04:00–09:30 ET) | `PRE-MARKET` |
| Regular session (09:30–16:00 ET) | `OPEN` |
| After-hours (16:00–20:00 ET) | `AFTER-HOURS` |
| Overnight / weekend | `CLOSED` |

### `s` Key — SELL SHORT from Positions Panel

From the Positions panel, `s` opens the Order Entry modal pre-filled with the selected symbol and
the SELL SHORT side (existing `o` continues to open SELL). This mirrors the symbol-detail modal
which also offers `o` / `s`.

### ↑ / ↓ Arrow Keys in Order Entry Dropdowns

The Up and Down arrow keys now cycle through values in the Side, OrderType, and TimeInForce
dropdown fields inside the Order Entry modal — mirroring the existing Left/Right behaviour for
users who prefer vertical navigation.

---

## Bug Fixes

| Fix | Description |
|---|---|
| Intraday sparkline stuck on "Loading…" | `Event::IntradayBarsReceived` now correctly stores bars per symbol; Symbol Detail renders them on open |

---

## Tests

**327 tests total** (up from 198 in v0.3.0):

| Scope | Count | Highlights |
|---|---|---|
| Library (`src/stream/`, `src/types.rs`, `src/config.rs`) | 55 | Serde round-trips, env-var resolution, WebSocket integration |
| Binary crate (`src/app.rs`, `src/update.rs`, `src/handlers/`, `src/ui/`) | 249 | All new features + navigation + mouse modal handler |
| HTTP integration (`tests/client_tests.rs`) | 23 | All `AlpacaClient` methods against a `wiremock` mock |

New test coverage includes:
- Orders, Positions, and Watchlist panel `j`/`k`/`g`/`G` navigation and edge cases
- Mouse modal handler — submit button, Side thirds, OrderType halves, Confirm yes/no buttons
- About and Symbol Detail render paths
- Dashboard `render_status()` helper for all tab contexts
- Search handler backspace, empty-backspace, and character-append selection reset

---

## No Breaking Changes

v0.4.0 is fully backwards-compatible with v0.3.0. All credential resolution, CLI flags,
environment variables, and library API are unchanged.

---

## Getting Started

```bash
git clone https://github.com/arunkumar-mourougappane/alpaca-trader-rs
cd alpaca-trader-rs

# First run — app prompts for credentials and offers to save to keychain
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

