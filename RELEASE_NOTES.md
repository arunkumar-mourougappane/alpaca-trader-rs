# Release Notes — v0.2.0

**Release date:** 2026-05-10
**MSRV:** Rust 1.88+
**Previous release:** [v0.1.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.1.0)

---

## Overview

v0.2.0 completes the Phase 2 roadmap: live WebSocket streaming for market data and account events, a fully wired async command channel for order submission and watchlist mutations, structured logging, and a raft of UX and reliability improvements. The test suite has grown from 101 to **188 tests**.

---

## What's New

### WebSocket Streaming (Phase 2 — now complete)

- **`src/stream/market.rs`** — connects to the Alpaca IEX market data WebSocket, authenticates, and streams real-time NBBO quotes for every symbol in the active watchlist. Resubscribes automatically when the watchlist changes; sends an explicit `unsubscribe` message for removed symbols (the protocol merges subscriptions and does not replace them). Reconnects with exponential backoff (1 s → 30 s cap) on disconnect.
- **`src/stream/account.rs`** — connects to the Alpaca account/trade update stream and forwards `TradeUpdate` events for order fills and status changes. Same exponential-backoff reconnection strategy.
- **Connection status indicators** — the TUI header now shows live `[MKT ●]` / `[ACCT ●]` badges, updated in real time via `Event::StreamConnected` / `Event::StreamDisconnected`.

### Command Channel (Phase 2 — now complete)

- **`src/commands.rs`** — defines the `Command` enum (`SubmitOrder`, `CancelOrder`, `AddToWatchlist`, `RemoveFromWatchlist`) bridging the synchronous `update()` function to the async task world.
- **`src/handlers/commands.rs`** — async command-handler task that receives commands from `update()` over an `mpsc` channel and dispatches them to `AlpacaClient`. Live order submission and watchlist mutation now make real REST calls.

### Logging

- **`src/logging.rs`** — structured logging via `tracing` + `tracing-appender` + `syslog`. Writes to a rolling file at a platform-appropriate path (`$HOME/Library/Logs/alpaca-trader/` on macOS, `$HOME/.local/share/alpaca-trader/` on Linux) and forwards to the system syslog. `RUST_LOG` controls the log level at runtime.

### UX Improvements

- **Mouse click handling** — clicking a tab bar label switches to that panel; clicking Orders sub-tab labels switches sub-tabs. Hit-testing uses actual rendered label widths, not fixed offsets.
- **Order Entry validation** — the order form is validated before dispatching a command: empty symbol, zero quantity, and a missing price on limit orders are all rejected with a status-bar error message.
- **Time-in-Force toggle** — the Order Entry modal now includes a DAY / GTC selector. When the market is closed, submitting a DAY order is blocked with a warning.
- **Status message auto-dismiss** — status-bar messages clear automatically after 3 seconds; no manual `Esc` required.
- **Portfolio history sparkline** — the Account panel equity sparkline is now pre-populated from `GET /account/portfolio/history` at startup (1-minute bars for the current trading day), so the chart is immediately useful even before the first real-time tick.

### Developer Experience

- **`#![deny(missing_docs)]`** — enforced across the entire library crate. Every public struct, enum, variant, field, and method now has a `///` doc comment; every public module has a `//!` module-level doc.
- **CI: code coverage** — a `coverage` job runs `cargo-llvm-cov --lcov` and uploads results to Codecov. Codecov badge added to README.
- **CI: release workflow** — `.github/workflows/release.yml` compiles and uploads pre-built binaries for `x86_64-linux`, `x86_64-darwin`, and `aarch64-darwin` on every `v*` tag push. Release badge added to README.
- **Release profile** — `Cargo.toml` gains `[profile.release]` with `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`, expected to reduce binary size by ~30–50%.

---

## Bug Fixes

| Fix | File | Detail |
|-----|------|--------|
| Stream unsubscribe on symbol removal | `src/stream/market.rs` | The Alpaca IEX protocol merges subscriptions; removed symbols were still streaming until the connection recycled. An explicit `unsubscribe` frame is now sent before the re-subscribe. |
| Client header panic | `src/client.rs` | `AlpacaClient::new` called `.unwrap()` on header value construction; non-ASCII or space-containing keys now return a proper `Result::Err`. |
| Logging init with `$HOME` unset | `src/logging.rs` | Logging initialiser no longer panics when `$HOME` is absent; falls back to a temporary directory. |
| Tab bar hit-test | `src/handlers/input.rs` | Mouse clicks on tab bar labels used fixed offsets; replaced with widths measured from the rendered text. |
| Orders sub-tab hit-test | `src/handlers/input.rs` | Sub-tab mouse click areas now use the exact rects captured during the last `render()` pass. |
| Closed command channel | `src/handlers/commands.rs` | Command sends to a closed or full channel are now handled gracefully instead of silently dropped. |

---

## Tests

**188 tests total** (up from 101 in v0.1.0):

| Scope | Count | Highlights |
|---|---|---|
| Library (`src/stream/`, `src/types.rs`, `src/config.rs`) | 43 | 8 WebSocket integration tests (auth, auth-failure, cancel, reconnect) × 2 streams; 2 unsubscribe tests; serde + env-var unit tests |
| Binary crate (`src/app.rs`, `src/update.rs`, `src/handlers/`) | 126 | State logic, keyboard dispatch, mouse click paths, validation, command dispatch, REST polling |
| HTTP integration (`tests/client_tests.rs`) | 19 | All 11 `AlpacaClient` methods against a `wiremock` mock; auth header validation; regression tests for non-ASCII keys |

---

## Key Bindings (unchanged from v0.1.0)

| Key | Action |
|-----|--------|
| `1` / `2` / `3` | Switch panel — or Orders sub-tab when Orders is active |
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

---

## Getting Started

```bash
git clone https://github.com/arunkumar-mourougappane/alpaca-trader-rs
cd alpaca-trader-rs
cp .env.example .env
# Fill in your credentials — see docs/credentials-setup.md
./run.sh           # paper trading (default)
./run.sh --live    # live trading
```

See [README.md](README.md) for full setup instructions and [docs/](docs/) for architecture, API research, and testing strategy.
