# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project does not use semantic versioning — releases are tagged by date.

---

## [0.7.0] — 2026-05-21

Adds interactive chart crosshairs, a keyboard shortcut help overlay, global symbol search, equity-range toggling, column sorting, an orders symbol filter, position detail modal, double-click support, and PDT/day-trade account metrics. Extensive documentation overhaul covering architecture, UI mockups, account management design, and library reference. Test count grows from **541 → 800**.

### Added

#### Charts & Account Panel
- **Equity chart interactive crosshair** (`src/ui/account.rs`, `src/input/`) — keyboard (`←/h`, `→/l`) and mouse-click crosshair with a tooltip showing the date and portfolio value at the selected data point; `Esc` clears it.
- **Equity date-range toggle** (`src/ui/account.rs`, `src/prefs.rs`) — `p` cycles the equity chart between 1D / 1W / 1M / YTD ranges, each backed by a separate `GET /v2/account/portfolio/history` call.
- **Symbol detail intraday crosshair** (`src/ui/modals.rs`, `src/input/modal.rs`) — keyboard crosshair inside the Symbol Detail modal mirrors the Account panel crosshair pattern.
- **PDT flag and day-trade metrics** (`src/types.rs`, `src/ui/account.rs`) — `short_market_value`, `daytrade_count`, and the Pattern Day Trader flag are now fetched and displayed in the Account panel.

#### Navigation & Modals
- **Keyboard shortcut help overlay** (`src/ui/modals.rs`, `src/input/modal.rs`) — `?` opens a full-screen help modal listing all key bindings by section; any key closes it. (#91)
- **Global symbol search modal** (`src/app.rs`, `src/input/`) — `Ctrl-F` or `/` (from non-Watchlist tabs) opens a floating search input; `Enter` opens Symbol Detail for the typed ticker. (#95)
- **Position detail modal** (`src/app.rs`, `src/input/positions.rs`) — `Enter` on a Positions row opens a dedicated `PositionDetail` modal with an intraday chart and P&L summary; `o` launches Order Entry pre-filled with SELL. (#87)
- **Double-click to open detail** (`src/input/mouse.rs`) — double-clicking a list row opens the same detail modal as `Enter`; clicking outside an open modal dismisses it. (#92)

#### Orders
- **Symbol filter** (`src/input/orders.rs`, `src/app.rs`) — `f` activates an inline filter bar; type a ticker to narrow the visible rows; `Enter`/`Esc` closes; `F` clears. (#86)
- **Column sorting** (`src/input/orders.rs`, `src/input/positions.rs`) — `s` cycles the sort column; `S` toggles direction (Asc ↔ Desc) on both the Orders and Positions tables.

### Documentation
- **`docs/architecture.md`** — technology stack fully corrected (removed stale `apca`/`ratatui-textarea` entries, added all actual crates with versions); directory tree expanded with `src/input/`; Update/View function snippets corrected; data-flow diagram updated with `command_tx` channel.
- **`docs/ui-mockups.md`** — corrected Orders footer; added Position Detail modal section; fixed Symbol Detail trigger; added per-modal keyboard tables.
- **`docs/testing.md`** — test count updated 101 → 800; full `src/input/` + `src/ui/` test file layout.
- **`docs/future-features.md`** — Status column added; 7 issues marked ✅ Implemented.
- **`docs/library.md`** — new file: full library API reference with 6 usage examples.
- **`docs/account-management.md`** — new file: design spec for in-app credential entry, multi-profile support, and settings modal (Phases 1–3).

### Changed
- **About modal** — `A` opens an About modal with version, author, and project info baked in at compile time via `env!` macros.

[0.7.0]: https://github.com/arunkumar-mourougappane/alpaca-trader-rs/compare/v0.6.0...v0.7.0

## [0.6.0] — 2026-05-19

Adds live WebSocket chart streaming, order fill notifications, a P&L footer in the Positions panel,
clipboard copy, watchlist-removal confirmation, refresh spinner, status bar message queue, a Filled
Price column in Orders, and vim `gg`/`G` navigation. Internal refactoring consolidates repeated
formatting, navigation, and render-test helpers. Test count grows from **449 → 541**.

### Added

#### Charts
- **Live WebSocket chart streaming** (`src/handlers/stream.rs`, `src/ui/account.rs`) — intraday
  equity and position charts are fed with real-time quote ticks between REST poll cycles, giving
  smooth braille curve updates without waiting for the next 60-second poll.

#### Orders
- **Filled Price column** (`src/types.rs`, `src/ui/orders.rs`) — `filled_avg_price` field added to
  the `Order` type and surfaced as a new column in the Orders table so executed prices are always
  visible.
- **Order fill notifications** (`src/handlers/rest.rs`, `src/update.rs`) — when a polled order
  transitions to `filled`, a transient status bar message flashes "✓ \<SYMBOL\> order filled at
  $\<price\>" so traders are notified without leaving the current panel.

#### Positions
- **P&L summary footer row** (`src/ui/positions.rs`) — a pinned footer row below the Positions
  table shows aggregate unrealised P&L across all open positions in both dollar and percentage form.

#### Watchlist
- **Removal confirmation modal** (`src/update.rs`, `src/ui/`) — pressing `d` on a watchlist entry
  now shows a "Remove \<SYMBOL\> from watchlist?" yes/no confirmation before sending the delete
  command.

#### Keyboard
- **Clipboard copy** (`c` key, Positions/Watchlist/Orders) — copies the selected row's symbol to
  the system clipboard; confirmation shown in the status bar.
- **`gg` / `G` jump navigation** (Positions, Orders, Watchlist) — vim-style `gg` jumps to the
  first row; `G` jumps to the last row; `j`/`k` scroll one row at a time.

#### UX
- **Refresh spinner and last-updated timestamp** — header area shows a spinning Braille frame while
  a REST poll is in flight and the hh:mm:ss timestamp of the last successful refresh.
- **Status bar message queue** — rapid events (fill notifications, clipboard confirmations, errors)
  are queued and shown sequentially instead of overwriting each other.

### Refactored

- **`src/ui/formatting.rs`** (new) — shared `format_dollar`, `format_price`, `format_pct_ratio`,
  and `header_cell` helpers; removes duplicate private formatting functions from `positions.rs`,
  `account.rs`, and `orders.rs`.
- **`src/ui/test_helpers.rs`** (new) — `render_to_string(width, height, fn)` helper eliminates
  `TestBackend` boilerplate from every UI module's test section.
- **`ThemeColors::bordered_block`** (`src/ui/theme.rs`) — single call replaces all inline
  `Block::default().title().borders(ALL).border_style()` chains across the codebase.
- **`handle_nav_key` + `SelectionState` trait** (`src/input/mod.rs`) — extracted shared vim-nav
  logic works generically over both `ListState` and `TableState`; removes ~20 duplicated lines from
  each of the three input handlers.
- **`OrderEntryState::with_side` builder** (`src/app.rs`) — replaces manual field mutation in the
  modal handler.

### Tests

**541 tests total** (up from 449 in v0.5.0):

| Scope | Count |
|---|---|
| Library (`src/stream/`, `src/types.rs`, `src/config.rs`) | 99 |
| Binary crate (`src/app.rs`, `src/update.rs`, `src/handlers/`, `src/ui/`) | 413 |
| HTTP integration (`tests/client_tests.rs`) | 29 |

Coverage additions include:
- Live chart streaming event handlers
- Order fill notification detection and dispatch
- P&L footer rendering (zero positions, single, multi)
- Watchlist removal confirmation modal flow
- Clipboard copy keybinding paths
- `gg`/`G` navigation in all three table panels
- Status bar message queue ordering and dequeue behaviour
- Shared formatting helpers (`format_dollar`, `format_price`, `format_pct_ratio`, `header_cell`)

---

## [0.5.0] — 2026-05-18

Adds a `--dry-run` mode, persistent user preferences, runtime colour-theme switching, Windows
platform support, and braille line charts throughout the UI. Fixes paper-trading watchlist
display, market-closed price data, and a silent resize event that left the UI clipped after
terminal resize. Test count grows from **327 → 449**.

### Added

#### CLI
- **`--dry-run` flag** — intercepts order submissions and shows them as `[DRY-RUN]` in the status
  bar without transmitting to Alpaca. All read-only operations (account, positions, watchlist) still
  hit the configured environment.

#### User Preferences
- **Persistent TOML config** (`src/prefs.rs`) — preferences are saved to the OS config directory
  (`~/.config/alpaca-trader/prefs.toml` on Linux/macOS, `%APPDATA%\alpaca-trader\prefs.toml` on
  Windows). Supported prefs: `app.default_env` (paper/live), `ui.theme`.

#### UI
- **Runtime colour-theme switching** (`T` key) — cycles between the available colour themes
  without restarting; theme selection is persisted to `prefs.toml` automatically.
- **Braille line charts** — equity and intraday price charts upgraded from `Sparkline` to ratatui
  `Chart` with double-resolution braille canvas, giving a much sharper visual.

#### Platform
- **Windows support** — full CI matrix (`Test`, `Coverage`) now includes `windows-latest`;
  platform-conditional code paths for log directory resolution, syslog (unix-only), and keychain.

#### CI / Quality
- **Windows code coverage** — `cargo-llvm-cov` runs on both `ubuntu-latest` and `windows-latest`;
  both reports are merged in Codecov with `carryforward: true` flags (`Linux` / `Windows`).
- **`codecov.yml`** — project and patch thresholds (5%), flag groups defined, `src/main.rs` ignored.

### Fixed

- **Terminal resize ignored** (`src/update.rs`) — `Event::Resize` now sets `app.needs_redraw =
  true`; the main loop draws an extra frame immediately so the layout adapts without waiting for the
  next tick (up to 250 ms).
- **Watchlist paper-trading message** (`src/update.rs`) — a persistent "Watchlists unavailable in
  paper mode" notice is now shown when the paper endpoint signals the watchlist API is unsupported.
- **Watchlist price/Change% when market is closed** — REST snapshot data is now used to populate
  Price and Change% columns even when the market is closed and live quotes are unavailable.
- **`MessageVisitor` unused-struct warning on Windows** — `MessageVisitor` and all syslog-related
  helpers are now gated to `#[cfg(unix)]`, eliminating the dead-code warning that failed
  clippy `-D warnings` on Windows CI.

### Tests

**449 tests total** (up from 327 in v0.4.0):

| Scope | Count |
|---|---|
| Library (`src/stream/`, `src/types.rs`, `src/config.rs`) | 99 |
| Binary crate (`src/app.rs`, `src/update.rs`, `src/handlers/`, `src/ui/`) | 320 |
| HTTP integration (`tests/client_tests.rs`) | 29 |
| Doc-tests | 1 |

Coverage additions include:
- `resize_event_sets_needs_redraw` and `resize_event_does_not_quit_or_change_state`
- Logging module expanded to ~90% coverage (MessageVisitor, SyslogLayer, log-dir resolution)
- Orders and Positions UI renderer unit tests
- Client module improved coverage

---

## [0.4.0] — 2026-05-12

Adds the About modal, SELL SHORT from positions, up/down arrow navigation in Order Entry dropdowns,
OHLCV and intraday sparkline in Symbol Detail, Day/Open P&L fields in the Account panel, Volume
and Change% columns in the Watchlist, and PRE-MARKET / AFTER-HOURS state detection in the header.
Fixes the intraday sparkline stuck on "Loading…". Test count grows from **198 → 327**.

### Added

#### Modals
- **About modal** (`A` key — global): displays app name, version (from `CARGO_PKG_VERSION`),
  author info, project URLs, and license (`CARGO_PKG_LICENSE`); any key press closes it.
  `A → About this app` added to the Help overlay GLOBAL section and `A:About` to all status bars.
- **Symbol Detail modal** — OHLCV fields (Open, High, Low, Volume), intraday 1-minute price
  sparkline, and `w` key to toggle watchlist membership for the displayed symbol.

#### Panels
- **Account panel** — Day P&L and Open P&L fields with colour coding (green = positive,
  red = negative); Account number displayed alongside account status.
- **Watchlist panel** — Volume and Change% columns replace the previous Ask/Bid columns.
- **Header** — Market clock now correctly identifies and displays PRE-MARKET and AFTER-HOURS
  states in addition to OPEN and CLOSED.

#### Keyboard
- **`s` key** (Positions panel) — opens Order Entry pre-filled with the selected symbol and
  SELL SHORT side.
- **↑ / ↓ arrow keys** (Order Entry modal) — cycle through values in the Side, OrderType, and
  TimeInForce dropdown fields, mirroring the existing `←` / `→` behaviour.

### Fixed

- **Intraday sparkline stuck on "Loading…"** (`src/update.rs`) — `Event::IntradayBarsReceived`
  now correctly stores bars keyed by symbol and the Symbol Detail modal renders them immediately.

### Tests

**327 tests total** (up from 198 in v0.3.0):

| Scope | Count |
|---|---|
| Library (`src/stream/`, `src/types.rs`, `src/config.rs`) | 55 |
| Binary crate (`src/app.rs`, `src/update.rs`, `src/handlers/`, `src/ui/`) | 249 |
| HTTP integration (`tests/client_tests.rs`) | 23 |

Coverage additions include:
- Orders / Positions / Watchlist panel navigation (`j`/`k`/`g`/`G` and arrow keys)
- Mouse modal handler (`handle_modal_mouse`) — submit button, Side/OrderType radio clicks, Confirm yes/no
- Symbol Detail and About modal render paths
- Search handler `Backspace` and character-append edge cases
- Dashboard `render_status()` helper for all four tab contexts

---

## [0.3.0] — 2026-05-10

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

[0.6.0]: https://github.com/arunkumar-mourougappane/alpaca-trader-rs/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/arunkumar-mourougappane/alpaca-trader-rs/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/arunkumar-mourougappane/alpaca-trader-rs/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/arunkumar-mourougappane/alpaca-trader-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/arunkumar-mourougappane/alpaca-trader-rs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.1.0
