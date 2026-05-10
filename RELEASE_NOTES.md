# Release Notes — v0.1.0

**Release date:** 2026-05-09
**MSRV:** Rust 1.88+

---

## Overview

First public release of `alpaca-trader-rs` — an Alpaca Markets trading toolkit for Rust shipping as both an embeddable library crate and a standalone terminal UI application.

---

## What's Included

### Library (`alpaca_trader_rs`)

- **`AlpacaClient`** — async REST client built on `reqwest` 0.13 with APCA authentication headers. Covers all core endpoints: account info, positions, orders (list / submit / cancel), market clock, and watchlists.
- **`AlpacaConfig`** — loads paper or live credentials from environment variables, with trailing-slash normalisation and automatic `/v2` suffix handling.
- **Domain types** — fully `serde`-deserializable structs for `AccountInfo`, `Position`, `Order`, `Quote`, `Watchlist`, `MarketClock`, and supporting enums (`OrderSide`, `OrderType`, `TimeInForce`).
- **`Event` enum** — bridges terminal input, REST poll results, WebSocket data (Phase 2), and control signals over a single `tokio::sync::mpsc` channel.

### Application (`alpaca-trader` binary)

- **TUI dashboard** built on `ratatui` 0.30 and `crossterm` 0.29 with four panels:
  - **Account** — equity, buying power, cash, day-trade count, and a scrolling equity sparkline.
  - **Watchlist** — symbol list with live search/filter (`/`), add (`a`), and remove (`d`).
  - **Positions** — open positions table with P&L and totals footer.
  - **Orders** — Open / Filled / Cancelled sub-tabs; inline order entry modal; cancel confirmation dialog.
- **Order Entry modal** — market or limit orders, buy/sell, qty, price, time-in-force.
- **Symbol Detail modal** — per-symbol quote snapshot.
- **Help overlay** — full keyboard reference (`?`).
- **Paper / Live switching** — `run.sh --paper` (default) / `run.sh --live`; header badge shows `[PAPER]` (cyan) or `[LIVE]` (red) at all times.
- **REST poll loop** — concurrent five-endpoint poll every 5 seconds via `tokio::join!`; manual refresh via `r` key.

### Key Bindings

| Key | Action |
|-----|--------|
| `1` / `2` / `3` | Switch panel (Account / Watchlist / Positions) — or switch Orders sub-tab when Orders is active |
| `4` | Switch to Orders panel |
| `Tab` / `Shift-Tab` | Cycle panels forward / backward |
| `j` / `k` or `↑` / `↓` | Navigate rows |
| `g` / `G` | Jump to first / last row |
| `Enter` | Open symbol detail |
| `o` | New order |
| `c` | Cancel selected order |
| `a` / `d` | Add / remove watchlist symbol |
| `/` | Search / filter watchlist |
| `r` | Force refresh |
| `?` | Help overlay |
| `Esc` | Close modal |
| `q` / `Ctrl-C` | Quit |

### Infrastructure

- **GitHub Actions CI** — `fmt`, `clippy` (`-D warnings`), `test` (Ubuntu + macOS matrix), `msrv` (1.88), `docs` jobs.
- **Security audit** — `cargo-audit` on every `Cargo.toml`/`Cargo.lock` push and weekly schedule.
- **Dependabot** — weekly automated dependency updates for Cargo and GitHub Actions.

### Tests

101 tests across three tiers:

| Scope | Count | Approach |
|---|---|---|
| `types`, `config` | 20 | Pure unit — serde, enum helpers, env var parsing |
| `app`, `update`, `handlers/rest` | 67 | State logic, keyboard dispatch, async REST polling |
| `tests/client_tests.rs` | 14 | HTTP integration via `wiremock` mock server |

---

## Bug Fixes

- **Panic on short order IDs** (`src/update.rs`) — `&id[..8]` replaced with `&id[..id.len().min(8)]` to prevent a byte-index panic on IDs shorter than 8 characters.
- **Orders panel `1`/`2`/`3` keys unreachable** (`src/update.rs`) — global key handler was intercepting digit keys before the Orders sub-tab handler could see them. Fixed by adding `if app.active_tab != Tab::Orders` guards.

---

## Known Limitations / Phase 2

The following features are designed but not yet implemented:

- WebSocket market data streaming (real-time quotes)
- WebSocket account/trade stream
- Live order submission (REST wired; UI modal complete)
- Watchlist add/remove wired to REST API

---

## Getting Started

```bash
git clone https://github.com/amouroug/alpaca-trader-rs
cd alpaca-trader-rs
cp .env.example .env
# Fill in credentials — see docs/credentials-setup.md
./run.sh           # paper trading (default)
./run.sh --live    # live trading
```

See [README.md](README.md) for full setup instructions and [docs/](docs/) for architecture, API research, and testing strategy.
