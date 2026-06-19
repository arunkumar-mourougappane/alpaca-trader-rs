# apca Parity & Beyond

A prioritized checklist of changes needed to bring `alpaca-trader-rs` to feature and
quality parity with the [`apca`](https://github.com/d-e-s-o/apca) crate — and then
past it, capitalizing on areas where this project already has a structural advantage.

Items are grouped into five tiers. Tiers 1–2 are blockers for production use. Tiers
3–5 are quality and ecosystem improvements.

---

## Tier 1 — Critical: Correctness & Silent Failures

These are bugs present today that silently corrupt state or hide errors from the user.
Fix these before any production use.

- [ ] **Check HTTP status codes before deserializing responses** (`src/client.rs`)
  All API calls pass responses directly to serde without calling `.error_for_status()`.
  A `401 Unauthorized`, `403 Forbidden`, `422 Unprocessable`, or `429 Too Many Requests`
  currently silently fails or panics in serde. Every `client.get/post/delete` needs to
  check the status code and return a typed error before attempting deserialization.

- [ ] **Stop swallowing channel send errors** (`src/update.rs`, `src/handlers/rest.rs`)
  Every `let _ = tx.send(...).await` silently drops the event if the channel is full or
  the receiver is gone. At minimum these should log a warning; ideally they trigger a
  shutdown or reconnect signal so the app does not silently become a zombie.

- [ ] **Do not default `market_open = true` when clock is unavailable** (`src/ui/modals.rs:245`)
  If the clock API fails, the order entry modal silently permits DAY orders during market
  close. When clock state is unknown the modal should either disable order entry or display
  a visible warning.

- [ ] **Surface buying-power parse failures** (`src/ui/modals.rs:373`)
  A `parse::<f64>()` failure silently defaults buying power to `0`, blocking the user
  from placing any order with no explanation. Log the failure and show a status message.

- [ ] **Surface equity parse failures** (`src/app.rs:469`)
  Invalid equity strings silently skip sparkline updates. Log and emit a `StatusMsg`.

- [ ] **Propagate command errors to the UI** (`src/handlers/commands.rs`)
  Order submission failures, cancellation failures, and watchlist mutation failures are
  `warn!`-logged but never sent to the UI. Each error path should emit an
  `Event::StatusMsg` so the user knows the command failed.

- [ ] **Fix time-in-force fallthrough** (`src/handlers/commands.rs:52`)
  Any TIF value other than `"gtc"` silently maps to `Day`, including invalid values.
  Match exhaustively and return an error for unrecognised values.

---

## Tier 2 — High: Feature Completeness to Match `apca`

`apca` covers the full Alpaca API surface. These gaps mean orders that the broker
supports cannot be placed or managed through this project.

- [ ] **Add missing order types** (`src/types.rs`, `src/client.rs`, `src/ui/modals.rs`)
  `OrderType` currently has `Market` and `Limit` only. Add:
  - `Stop`
  - `StopLimit`
  - `TrailingStop` (by price and by percent)

  Update `OrderRequest`, the order entry modal, and input validation for each new type.

- [ ] **Add missing time-in-force values** (`src/types.rs`, `src/handlers/commands.rs`)
  `TimeInForce` currently has `Day` and `Gtc` only. Add:
  - `Ioc` (Immediate or Cancel)
  - `Opg` (At Open)
  - `Cls` (At Close)
  - `Fok` (Fill or Kill — required for crypto)

- [ ] **Replace `String` monetary values with a typed decimal type** (`src/types.rs`)
  `equity`, `buying_power`, `cash`, `avg_entry_price`, `unrealized_pl`, `limit_price`,
  and all other monetary fields are raw `String`. Callers must `.parse::<f64>()` with no
  compile-time safety and risk floating-point rounding errors. Replace with
  `rust_decimal::Decimal` (or a newtype wrapping it) across all public types. This is one
  of `apca`'s biggest usability advantages (`Num` wrapper) and the single highest-leverage
  type-safety improvement available.

- [ ] **Replace `anyhow::Result` in the public library API with typed errors** (`src/client.rs`, `src/config.rs`)
  `anyhow` is appropriate inside the binary but is a poor library API — callers cannot
  pattern-match on error variants. Expose a typed `Error` enum (e.g. `AuthError`,
  `RateLimitError`, `NetworkError`, `ApiError { status, message }`) and use `anyhow`
  only inside the binary crates.

- [ ] **Add rate-limit handling and retry logic** (`src/client.rs`)
  Alpaca returns `429 Too Many Requests` with a `Retry-After` header. The client has no
  retry logic at all. Add exponential backoff with jitter for transient errors (`429`,
  `5xx`) with a configurable retry budget (default: 3 attempts).

- [ ] **Add SIP and crypto feed support to the market data stream** (`src/stream/market.rs`)
  The WebSocket stream is hard-coded to `wss://stream.data.alpaca.markets/v2/iex`. Add a
  `DataFeed` enum (`Iex`, `Sip`, `Crypto`) stored in `AlpacaConfig` and used to select
  the stream endpoint at connection time.

- [ ] **Support multiple watchlists** (`src/handlers/rest.rs`, `src/app.rs`, `src/ui/watchlist.rs`)
  The app hard-codes `summaries[0]` everywhere, always loading and modifying the first
  watchlist. List all watchlists, let the user select one (see also `future-features.md`),
  and track the active watchlist ID in app state.

- [ ] **Add pagination support for orders** (`src/client.rs`)
  `get_orders` hard-codes `limit=100`. Alpaca supports `page_token`-based pagination.
  Orders beyond 100 are silently truncated. Add a paginated fetch that collects all pages
  before returning, or expose an async iterator.

---

## Tier 3 — Medium: Reliability & UI Polish

- [ ] **Add retry/backoff to REST polling** (`src/handlers/rest.rs`)
  Single-attempt polling means any transient network blip causes a missed update. Add at
  least 3 retry attempts with exponential backoff before surfacing a failure to the UI.

- [ ] **Make poll intervals configurable** (`src/handlers/rest.rs`, `src/main.rs`)
  The 5-second REST poll interval and 250 ms tick are hard-coded constants. Expose them
  in `AlpacaConfig` or a separate `AppConfig` struct so users can tune for latency vs.
  API quota usage.

- [ ] **Add jitter to WebSocket reconnection backoff** (`src/stream/market.rs`, `src/stream/account.rs`)
  Both streams use pure exponential backoff (1 s → 2 s → 4 s → … → 30 s) with no jitter.
  Under thundering-herd conditions all clients reconnect in lockstep. Add ±25 % random
  jitter at each backoff step.

- [ ] **Add extended hours support to order entry** (`src/ui/modals.rs`, `src/types.rs`)
  The order entry modal has no `extended_hours: bool` field. Alpaca supports pre-market
  and after-hours trading for limit orders. Show and wire up this toggle when
  `market_open == false`.

- [ ] **Enforce symbol format in input validation** (`src/input/validation.rs`)
  Symbol validation only strips whitespace — `$$$`, `1234`, and empty strings all pass
  and fail only at the API layer. Add a regex for valid ticker format (1–5 uppercase
  alpha characters for equities; handle crypto pair format separately).

- [ ] **Add buying-power check for market orders** (`src/input/validation.rs`)
  The current check covers only limit orders (qty × price). Market orders with very large
  quantities pass validation and fail silently at the API. Add a conservative estimate
  check using `current_price × qty` against available buying power.

- [ ] **Warn on PDT restriction pre-flight** (`src/input/validation.rs`, `src/app.rs`)
  `AccountInfo` exposes `daytrade_count` and `pattern_day_trader` but neither is used.
  Before submitting a same-day round-trip order, warn the user if they are approaching
  the 3-day-trade limit or are already flagged as a PDT with < $25 k equity.

- [ ] **Skip event sends when state has not changed** (`src/handlers/rest.rs`)
  Every poll sends full state updates regardless of whether anything changed. Derive
  `PartialEq` on `AccountInfo`, `Position`, and `Order`, and skip sending the event when
  the new value equals the cached value. Reduces unnecessary redraws and downstream
  allocations.

- [ ] **Cap equity-history cast safely** (`src/app.rs:470`)
  `(equity * 100.0) as u64` truncates silently for very large values. Use saturating
  arithmetic or a checked cast.

- [ ] **Make the equity sparkline history length configurable** (`src/app.rs`)
  The 120-entry cap is hard-coded. Tie it to the terminal width or expose it as a config
  option.

---

## Tier 4 — Library API & Ecosystem Quality

- [ ] **Add a `tui` Cargo feature flag** (`Cargo.toml`)
  `ratatui`, `crossterm`, and `clap` are always compiled, even for library consumers who
  only want the REST/WebSocket client. Add an optional `tui` feature (enabled by default
  for the binary target) so downstream crates do not pull in terminal dependencies.

- [ ] **Use typed `Uuid` for all ID fields** (`src/types.rs`, `Cargo.toml`)
  Order IDs, watchlist IDs, and asset IDs are raw `String`. Add the `uuid` crate and
  use `Uuid` for all ID fields to prevent mixing up ID types at compile time.

- [ ] **Add `AlpacaClient::with_timeout()` and `with_base_url()` builder methods** (`src/client.rs`)
  The client has no way to configure request timeouts or override the base URL without
  env vars. Both are needed for testability and alternative environments (sandboxes,
  proxies).

- [ ] **Derive `Clone`, `Debug`, and `PartialEq` on all public types** (`src/types.rs`)
  Several public structs are missing these derives, making them awkward to use in async
  contexts where data must be cloned across tasks. Audit every public type and add the
  missing derives.

- [ ] **Write and publish full rustdoc with examples** (`src/lib.rs`, `src/client.rs`, `src/types.rs`)
  Most public structs and methods have no doc comments. Add module-level documentation
  and at least one `# Example` block per public method in `AlpacaClient`. Verify with
  `cargo doc --no-deps --open`.

---

## Tier 5 — Test Coverage Gaps

- [ ] **Add HTTP error response tests** (`tests/client_tests.rs`)
  No tests exist for `401`, `403`, `422`, or `429` responses. Add wiremock tests for
  each status code and assert the correct error variant is returned.

- [ ] **Add WebSocket reconnection integration tests** (`tests/`)
  No test verifies that streams reconnect after a server-side close. Add tests using a
  mock WebSocket server that closes the connection mid-session and assert that the stream
  re-authenticates and delivers subsequent messages.

- [ ] **Add input validation unit tests** (`src/input/validation.rs`)
  `validation.rs` has no tests. Add cases for: empty symbol, non-alpha symbol, market
  order with no quantity, limit order with no price, quantity exceeding buying power, and
  PDT warning threshold.

- [ ] **Add concurrent REST + WebSocket integration tests** (`tests/`)
  No test exercises polling and streaming simultaneously. Add a test that fires REST
  responses and WebSocket quotes concurrently and verifies the app state stays consistent.

- [ ] **Add credential resolution tier tests** (`tests/`)
  The existing tests cover unified env vars and per-env vars. Add tests for the keychain
  fallback (mock the `keyring` layer) and interactive prompt fallback (mock `rpassword`),
  covering all four resolution tiers in isolation.

---

## Comparison Summary

| Dimension | alpaca-trader-rs (current) | apca |
|-----------|---------------------------|------|
| Monetary types | `String` — caller parses | Typed `Num` newtype |
| Order types | Market, Limit | Market, Limit, Stop, StopLimit, TrailingStop |
| Time-in-force | Day, GTC | Day, GTC, IOC, OPG, CLS, FOK |
| Error type | `anyhow::Result` (opaque) | Typed enum (matchable) |
| HTTP error handling | None — silent serde failure | Explicit per-status handling |
| Rate limiting | None | Built-in |
| Pagination | Hard-coded limit=100 | Full pagination support |
| Data feeds | IEX only | IEX, SIP, OTC, Crypto |
| Reconnection | Exp. backoff, no jitter | Managed by library |
| Credential setup | 4-tier + native keychain | Env vars only |
| Interactive TUI | Full dashboard | None (CLI companion `apcacli` only) |
| Feature flags | None | N/A |
| Maturity | v0.4.0 | v0.30.0, 46 releases |
| License | MIT | GPL-3.0 |

The credential management story and the interactive TUI are structural advantages that
`apca` cannot match without a major redesign. Completing Tiers 1 and 2 above closes the
remaining gaps and produces a project that is meaningfully better than `apca` for any
developer who wants both a typed Alpaca client and a ready-to-run trading terminal.
