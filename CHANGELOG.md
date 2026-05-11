# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project does not use semantic versioning — releases are tagged by date.

---

## [0.3.0] — unreleased

Introduces OS-native keychain credential storage with an interactive first-run provisioning flow, and replaces the `ALPACA_ENV` environment variable with a `--paper` CLI flag.

### Added

#### Credential Storage (`src/credentials.rs`)
- Tiered credential resolution: environment variables → OS-native keychain → interactive TTY prompt
- `credentials::resolve(AlpacaEnv)` must be called before `enable_raw_mode()` — it may print to stderr and read from stdin
- OS-native backends via platform-conditional `keyring` v3 — no C-library dependency on any platform:
  - macOS: Keychain Access (`apple-native`)
  - Windows: Credential Store (`windows-native`)
  - Linux: kernel keyutils (`linux-native`) — cross-compiles cleanly; no `libdbus`
- First-run interactive prompt via `rpassword` v7 — opens `/dev/tty` directly, unaffected by stdin redirection
- Prompts user to store credentials in the OS keychain after successful entry (default: yes)
- Graceful degradation when keychain is unavailable (locked, WSL, headless) — warns and continues session-only
- Clear error with instructions when running headless (no TTY) with no credentials configured

#### Library (`alpaca_trader_rs`)
- `ResolvedCredentials` — public struct carrying resolved `endpoint`, `key`, `secret`, and `env`
- `AlpacaConfig::from_credentials(ResolvedCredentials)` — new constructor; applies the same URL normalisation as `from_env`

### Changed

- **`--paper` CLI flag** replaces `ALPACA_ENV` env var for environment selection
  - `alpaca-trader` (no flag) → **live** account (real money, new default)
  - `alpaca-trader --paper` → paper account (simulated funds)
- **Credential resolution order** (highest → lowest priority):
  1. `ALPACA_API_KEY` + `ALPACA_API_SECRET` — unified pair; ideal for CI, Docker, systemd
  2. `LIVE_ALPACA_KEY` + `LIVE_ALPACA_SECRET` (or `PAPER_*`) — per-environment; developer `.env` files
  3. OS-native keychain — returning desktop users
  4. Interactive TTY prompt — first-run desktop
- **`AlpacaConfig::from_env(AlpacaEnv)`** — signature now takes the environment explicitly; `ALPACA_ENV` is no longer read
- `src/main.rs` — calls `credentials::resolve(env)` before `enable_raw_mode()`, then `AlpacaConfig::from_credentials()`
- `run.sh` — passes `--paper` to the binary instead of exporting `ALPACA_ENV`; `--live` kept as a no-op alias; `.env` loading is now optional
- `.env.example` — removed `ALPACA_ENV=paper`; clarified which vars belong to each environment
- `docs/credentials-setup.md`, `docs/architecture.md`, `docs/testing.md`, `README.md`, `docs/README.md` — all updated to document the new `--paper` flag and credential resolution flow

### ⚠️ Breaking Changes

- **Default environment changed from paper → live.** Users who relied on the old paper default must now pass `--paper` explicitly.
- `ALPACA_ENV` is no longer read. Setting it in the environment or `.env` file has no effect.
- `AlpacaConfig::from_env()` now requires an `AlpacaEnv` argument.

### Dependencies

- Added `rpassword = "7"` (TTY password prompts)
- Added `keyring = "3"` with platform-conditional native features (macOS / Windows / Linux)

---

## [0.2.0] — 2026-05-10

Completes the Phase 2 roadmap: live WebSocket streaming, wired async command channel, structured logging, mouse interaction, and a suite of reliability fixes. Test count grows from 101 → **188**.

### Added

#### Streaming (Phase 2)
- `src/stream/market.rs` — IEX WebSocket market data stream; subscribes to NBBO quotes for all watchlist symbols; live resubscription (with explicit `unsubscribe` for removed symbols) when the watchlist changes; exponential backoff reconnection (1 s → 30 s)
- `src/stream/account.rs` — Account/trade update WebSocket stream forwarding `TradeUpdate` events for order fills; same backoff reconnection strategy
- WebSocket connection status badges (`[MKT ●]` / `[ACCT ●]`) in the TUI header via `Event::StreamConnected` / `Event::StreamDisconnected`

#### Command Channel (Phase 2)
- `src/commands.rs` — `Command` enum (`SubmitOrder`, `CancelOrder`, `AddToWatchlist`, `RemoveFromWatchlist`) bridging the sync `update()` to async tasks
- `src/handlers/commands.rs` — async handler task that dispatches commands to `AlpacaClient`; live order submission and watchlist mutation now make real REST calls

#### Logging
- `src/logging.rs` — structured logging via `tracing` + `tracing-appender` + `syslog`; platform-appropriate log path (`$HOME/Library/Logs/` on macOS, `$HOME/.local/share/` on Linux); `RUST_LOG` env filter support

#### UX
- Mouse click handling — tab bar and Orders sub-tab labels are clickable; hit-testing uses actual rendered label widths
- Order Entry validation — rejects empty symbol, zero qty, or missing price on limit orders before dispatching
- Time-in-Force toggle (DAY / GTC) in Order Entry modal; market-closed warning blocks DAY orders when market is closed
- Status message auto-dismiss — status bar messages clear after 3 seconds
- Portfolio history sparkline — Account panel equity sparkline pre-populated from `GET /account/portfolio/history` at startup

#### Developer Experience
- `#![deny(missing_docs)]` enforced; `///` doc comments on every public library item; `//!` module docs on every public module
- CI `coverage` job — `cargo-llvm-cov --lcov` + Codecov upload; Codecov badge in README
- CI `release` workflow — pre-compiled binaries for `x86_64-linux`, `x86_64-darwin`, `aarch64-darwin` on `v*` tag pushes; Release badge in README
- `[profile.release]` — `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"` (~30–50% binary size reduction)
- `RUSTDOCFLAGS: "--cfg docsrs -D warnings"` in CI docs job

### Fixed

- **Stream unsubscribe on symbol removal** (`src/stream/market.rs`) — Alpaca IEX merges subscriptions; explicit `unsubscribe` frame now sent before re-subscribe when symbols are removed
- **Client header panic** (`src/client.rs`) — non-ASCII / space-containing API keys now return `Err` instead of panicking in `AlpacaClient::new`
- **Logging init without `$HOME`** (`src/logging.rs`) — graceful fallback to temp directory when `$HOME` is unset
- **Tab bar hit-test** — mouse clicks on tab labels use rendered text widths, not fixed offsets
- **Orders sub-tab hit-test** (`src/handlers/input.rs`) — sub-tab click areas use exact rects from the last `render()` pass
- **Closed command channel** (`src/handlers/commands.rs`) — full or closed channel handled gracefully instead of silently dropped

### Changed

- `update.rs` refactored into per-panel input submodules under `src/handlers/`
- `App` now holds `command_tx` (`mpsc::Sender<Command>`) and `symbol_tx` (`watch::Sender<Vec<String>>`) channels
- `update()` `WatchlistUpdated` handler pushes updated symbol list to the market stream for live resubscription
- README Status table: all four Phase 2 items marked **Done**; test count updated to 188
- `Cargo.toml` version bumped to `0.2.0`

### Tests

188 total (43 lib + 126 binary + 19 HTTP integration):

- 8 WebSocket integration tests (auth success, auth failure, cancellation, reconnect-after-close) × 2 stream modules
- 2 unsubscribe integration tests (removal triggers unsubscribe frame; addition does not)
- Regression tests for non-ASCII API key header construction

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

---

[0.3.0]: https://github.com/arunkumar-mourougappane/alpaca-trader-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/arunkumar-mourougappane/alpaca-trader-rs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.1.0
