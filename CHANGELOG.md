# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project does not use semantic versioning — releases are tagged by date.

---

## [0.1.0] — 2026-05-10

First release. Ships as both an integratable Rust library and a standalone TUI trading dashboard.

### Added

#### Library (`alpaca_trader_rs`)
- `AlpacaConfig` — resolves paper/live credentials from `.env` at startup via `ALPACA_ENV` + `LIVE_`/`PAPER_` prefixed variables
- `AlpacaClient` — async REST client wrapping all core Alpaca v2 endpoints:
  - `get_account`, `get_positions`, `get_orders`, `submit_order`, `cancel_order`
  - `get_clock`, `list_watchlists`, `get_watchlist`, `add_to_watchlist`, `remove_from_watchlist`
- Domain types with full `serde::Deserialize` support: `AccountInfo`, `Position`, `Order`, `OrderRequest`, `OrderSide`, `OrderType`, `TimeInForce`, `Quote`, `MarketClock`, `Watchlist`, `WatchlistSummary`, `Asset`
- `Event` enum shared between the library streams and the TUI app

#### TUI App (`alpaca-trader` binary)
- Elm Architecture (TEA) event loop: typed `Event` channel, `update(app, event)` dispatch, `render(frame, app)` pure view
- **Account panel** — equity, buying power, cash, long market value, PDT flag, intraday equity sparkline
- **Watchlist panel** — live asset table with ask/bid prices from REST, inline `/` search filter, `a`/`d` add/remove
- **Positions panel** — unrealised P&L per position with totals footer, live price column
- **Orders panel** — Open / Filled / Cancelled sub-tabs switchable with `1`/`2`/`3` (context-sensitive: these keys switch global panels from other tabs)
- **Order Entry modal** — Symbol, Side (BUY/SELL), Type (LIMIT/MARKET), Qty, Price fields; live Est. Total; buying power indicator; `Tab` field navigation
- **Symbol Detail modal** — ask/bid, exchange, asset flags (tradable, shortable, fractionable, ETB)
- **Help overlay** — full keyboard reference (`?`)
- **Confirm modal** — for order cancel and watchlist removal actions
- **Add Symbol modal** — type-to-search ticker input
- Background tasks: REST polling every 5 s with immediate refresh on `r`; crossterm `EventStream` input task; 250 ms tick for clock updates
- Graceful shutdown via `tokio_util::sync::CancellationToken`; terminal raw mode restored on exit
- `run.sh` script with `--paper` / `--live` flags; `ALPACA_ENV` in `.env` selects the active environment

#### Tests (101 total)
- `types.rs` — serde round-trips, enum `as_str()`, `OrderRequest` serde rename (`order_type` → `"type"`)
- `config.rs` — paper/live env resolution, slash trimming, MSRV, missing-var error paths (using `temp-env`)
- `app.rs` — `Tab`/`OrderField` navigation cycles, `filtered_orders()`, `push_equity()` cap at 120 entries, watchlist search filtering
- `update.rs` — all `Event` variants → state mutations; all keyboard paths including modal field editing, search mode, context-sensitive `1`/`2`/`3`
- `handlers/rest.rs` — `poll_once` emits all five event types; error path sends `StatusMsg`; cancellation exits cleanly
- `tests/client_tests.rs` — all 11 `AlpacaClient` HTTP methods against a `wiremock` mock server, including auth headers and query params

#### CI / Tooling
- GitHub Actions: Format, Clippy (`-D warnings`), Test (ubuntu + macos), MSRV (1.88), Docs
- Security audit workflow (`cargo audit`) on Cargo file changes and weekly schedule
- Dependabot for `cargo` and `github-actions` dependency updates (weekly)
- `rust-version = "1.88"` declared in `Cargo.toml`

#### Documentation
- `docs/architecture.md` — TEA design, library/app boundary, module layout, data flow diagram
- `docs/credentials-setup.md` — paper and live API key setup, env var reference
- `docs/ui-mockups.md` — ASCII mockups for all panels and modals, full keyboard/mouse interaction spec
- `docs/api-research.md` — live-tested REST endpoint shapes and watchlist API notes
- `docs/testing.md` — testing strategy, mock patterns, bugs found during testing
- `docs/github-actions.md` — GitHub Actions reference for Rust projects
- `docs/licensing.md` — license types and Collaboration Agreement process

### Fixed

- `&id[..8]` panic in order cancel confirm when order ID is shorter than 8 bytes — now uses `id.len().min(8)`
- `1`/`2`/`3` keys in Orders panel were intercepted by the global tab-switch handler, making sub-tab switching unreachable — added `if app.active_tab != Tab::Orders` guards on the global arms
- `collapsible_match` clippy errors across all three panel key handlers and the Order Entry modal field handler — moved `if` conditions into match arm guards
- `dtolnay/rust-toolchain@1.100` typo in CI (Rust 1.100.0 does not exist) — corrected to `@1.88`
- `reqwest 0.13` breaking change: `.query()` moved behind the `query` feature — added `"query"` to reqwest features

### Dependencies

| Crate | Version | Role |
|---|---|---|
| `tokio` | 1 | Async runtime |
| `reqwest` | 0.13 | HTTP client (`json` + `query` features) |
| `tokio-tungstenite` | 0.29 | WebSocket (Phase 2) |
| `serde` / `serde_json` | 1 | Serialization |
| `dotenvy` | 0.15 | `.env` loading |
| `anyhow` | 1 | Error handling |
| `chrono` | 0.4 | Date/time |
| `ratatui` | 0.30 | TUI rendering |
| `crossterm` | 0.29 | Terminal backend |
| `wiremock` | 0.6 | HTTP mocking (dev) |
| `temp-env` | 0.3 | Env var scoping (dev) |

### Known Limitations / Phase 2

- WebSocket market data streaming (`MarketStream`) not yet implemented — prices shown are from REST polling
- WebSocket account/trade stream (`AccountStream`) not yet implemented — order fill notifications require manual refresh
- Order submission and watchlist mutation are UI-only stubs — REST calls will be wired in Phase 2

---

[0.1.0]: https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.1.0
