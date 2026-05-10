# Testing Strategy

Research document covering test coverage strategy, mock patterns, crate selection, and a complete test case inventory for every module in `alpaca-trader-rs`.

No tests are written yet. This document is the reference for the implementation session.

---

## Overview

The codebase splits cleanly into three testability tiers:

| Tier | Modules | Approach |
|---|---|---|
| **Pure unit** | `types`, `config`, `app`, `update` | No mocking — manipulate structs directly, simulate key events |
| **HTTP integration** | `client` | `wiremock` mock HTTP server; `reqwest` hits it for real |
| **Async task** | `handlers/rest`, `handlers/input` | `tokio::test` with channel receivers; verify emitted `Event` variants |

Current dev-dependency count: **zero**. All additions are listed below.

---

## Recommended Dev Dependencies

```toml
[dev-dependencies]
wiremock   = "0.6"   # async HTTP mock server
tokio-test = "0.4"   # assert_ready!, assert_pending! for channel/future assertions
temp-env   = "0.3"   # thread-safe env var scoping for config tests
```

`serde_json` is already in `[dependencies]` and available in tests.

### Why wiremock over mockito

`mockito` uses a global mock server with thread-local state. Under `#[tokio::test]`, multiple tests run concurrently in the same process and share that global state — mocks bleed between tests. `wiremock` starts a fresh server per test (one `MockServer::start().await` call), is fully async, and automatically verifies every mounted mock was hit exactly the expected number of times on `drop`. A test that fails to call a declared endpoint is itself a test failure.

### Why temp-env over `std::env::set_var`

`std::env::set_var` is not thread-safe when multiple tests run in parallel. `temp-env::with_vars(vars, closure)` acquires a process-wide lock for the duration of the closure, sets the specified vars, runs the closure, then restores the original values — even if the closure panics.

### Why no trait injection for AlpacaClient

`AlpacaClient` wraps `reqwest::Client` directly. Since `wiremock` starts a real HTTP server and we point `AlpacaConfig::base_url` at `server.uri()`, `reqwest` exercises the actual serialization, header injection, and error-handling code paths. Introducing a trait layer would let mocks lie about those paths.

---

## Test File Layout

```
alpaca-trader-rs/
├── tests/
│   └── client_tests.rs    ← integration tests for AlpacaClient (wiremock)
└── src/
    ├── types.rs            ← #[cfg(test)] mod tests { }
    ├── config.rs           ← #[cfg(test)] mod tests { }
    ├── app.rs              ← #[cfg(test)] mod tests { }
    ├── update.rs           ← #[cfg(test)] mod tests { }
    └── handlers/
        ├── rest.rs         ← #[cfg(test)] mod tests { }
        └── input.rs        ← #[cfg(test)] mod tests { }
```

`client_tests.rs` lives in `tests/` (Rust integration test directory) so it links against the compiled library crate. All other test modules are inline `#[cfg(test)]` blocks.

---

## Mock Patterns

### 1. Env vars — `temp-env`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_paper() {
        temp_env::with_vars(
            [
                ("ALPACA_ENV",            Some("paper")),
                ("PAPER_ALPACA_ENDPOINT", Some("https://paper-api.alpaca.markets/v2")),
                ("PAPER_ALPACA_KEY",      Some("PKTEST000")),
                ("PAPER_ALPACA_SECRET",   Some("secret000")),
            ],
            || {
                let cfg = AlpacaConfig::from_env().unwrap();
                assert_eq!(cfg.env, AlpacaEnv::Paper);
                assert_eq!(cfg.base_url, "https://paper-api.alpaca.markets/v2");
                assert_eq!(cfg.key, "PKTEST000");
            },
        );
    }
}
```

The `with_vars` closure receives `Option<&str>` — `None` unsets the variable for the duration.

### 2. HTTP — `wiremock`

```rust
// tests/client_tests.rs
use alpaca_trader_rs::{client::AlpacaClient, config::{AlpacaConfig, AlpacaEnv}};
use serde_json::json;
use wiremock::{MockServer, Mock, ResponseTemplate, matchers::{method, path, header}};

fn test_config(base_url: String) -> AlpacaConfig {
    AlpacaConfig {
        base_url,
        key: "PKTEST000".into(),
        secret: "secret000".into(),
        env: AlpacaEnv::Paper,
    }
}

#[tokio::test]
async fn get_account_deserializes_all_fields() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/account"))
        .and(header("APCA-API-KEY-ID", "PKTEST000"))
        .and(header("APCA-API-SECRET-KEY", "secret000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ACTIVE",
            "equity": "100000",
            "buying_power": "200000",
            "cash": "100000",
            "long_market_value": "0",
            "daytrade_count": 0,
            "pattern_day_trader": false,
            "currency": "USD"
        })))
        .mount(&server)
        .await;

    let client = AlpacaClient::new(test_config(server.uri()));
    let account = client.get_account().await.unwrap();

    assert_eq!(account.status, "ACTIVE");
    assert_eq!(account.equity, "100000");
    assert_eq!(account.buying_power, "200000");
    assert!(!account.pattern_day_trader);
}
```

`MockServer` drop verifies every mounted mock was satisfied.

### 3. App state — plain struct construction

```rust
fn make_test_app() -> App {
    App::new(
        AlpacaConfig {
            base_url: "http://localhost".into(),
            key: "k".into(),
            secret: "s".into(),
            env: AlpacaEnv::Paper,
        },
        Arc::new(tokio::sync::Notify::new()),
    )
}

fn make_order(id: &str, status: &str) -> Order {
    Order {
        id: id.into(),
        symbol: "AAPL".into(),
        side: "buy".into(),
        qty: Some("10".into()),
        notional: None,
        order_type: "limit".into(),
        limit_price: Some("180.00".into()),
        status: status.into(),
        submitted_at: None,
        filled_at: None,
        filled_qty: "0".into(),
        time_in_force: "day".into(),
    }
}
```

### 4. Key events — simulated keyboard input

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::events::Event;

fn key(code: KeyCode) -> Event {
    Event::Input(KeyEvent::new(code, KeyModifiers::NONE))
}

fn ctrl(code: KeyCode) -> Event {
    Event::Input(KeyEvent::new(code, KeyModifiers::CONTROL))
}
```

### 5. Async handler — channel output verification

```rust
#[tokio::test]
async fn poll_once_emits_account_event() {
    let server = MockServer::start().await;
    // mount all required endpoints...

    let client = Arc::new(AlpacaClient::new(test_config(server.uri())));
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);

    handlers::rest::poll_once(tx, client).await;

    let mut events = vec![];
    while let Ok(e) = rx.try_recv() { events.push(e); }

    assert!(events.iter().any(|e| matches!(e, Event::AccountUpdated(_))));
}
```

---

## Test Case Inventory

### `types.rs` — 9 tests

| Test | What it checks |
|---|---|
| `order_side_buy_str` | `OrderSide::Buy.as_str() == "buy"` |
| `order_side_sell_str` | `OrderSide::Sell.as_str() == "sell"` |
| `order_type_market_str` | `OrderType::Market.as_str() == "market"` |
| `order_type_limit_str` | `OrderType::Limit.as_str() == "limit"` |
| `time_in_force_day_str` | `TimeInForce::Day.as_str() == "day"` |
| `time_in_force_gtc_str` | `TimeInForce::Gtc.as_str() == "gtc"` |
| `account_info_deserializes` | Full JSON → `AccountInfo`, all fields correct |
| `order_notional_qty_null` | Order with `qty: null` deserializes without error; `qty` is `None` |
| `watchlist_empty_assets_default` | `Watchlist` with no `assets` key deserializes to empty `Vec` |

### `config.rs` — 7 tests (all using `temp-env`)

| Test | What it checks |
|---|---|
| `from_env_paper_selects_paper_vars` | Correct `base_url`, `key`, `secret`, `env == Paper` |
| `from_env_live_appends_v2` | Live endpoint without `/v2` gets it appended; no double slash |
| `from_env_paper_trailing_slash_stripped` | `https://paper-api.alpaca.markets/v2/` → no trailing slash |
| `from_env_unknown_env_errors` | `ALPACA_ENV=staging` → `Err` |
| `from_env_missing_key_errors` | `PAPER_ALPACA_KEY` unset → `Err` |
| `from_env_defaults_to_paper` | `ALPACA_ENV` unset → selects paper vars |
| `env_label_paper` / `env_label_live` | `env_label()` returns correct static str |

### `client.rs` — 14 tests (all in `tests/client_tests.rs`)

| Test | Endpoint | What it checks |
|---|---|---|
| `get_account_200` | GET /account | Full deserialization |
| `get_account_401_errors` | GET /account | `Err` on 401 |
| `get_positions_empty` | GET /positions | Empty array → `Vec::new()` |
| `get_positions_populated` | GET /positions | Position fields deserialized |
| `get_orders_query_param` | GET /orders | Request includes `status=all` query param |
| `get_orders_notional` | GET /orders | Order with `qty: null` handled |
| `submit_order_post_body` | POST /orders | Body contains `"type"` key (serde rename from `order_type`) |
| `cancel_order_delete_path` | DELETE /orders/{id} | Correct path constructed |
| `get_clock_closed` | GET /clock | `is_open: false` parsed |
| `list_watchlists_no_assets` | GET /watchlists | Summary objects have no `assets` field |
| `get_watchlist_with_assets` | GET /watchlists/{id} | `assets` array deserialized |
| `add_to_watchlist_body` | POST /watchlists/{id} | Body is `{"symbol":"AAPL"}` |
| `remove_from_watchlist_path` | DELETE /watchlists/{id}/{symbol} | Path contains symbol |
| `auth_headers_present` | Any endpoint | Both `APCA-API-KEY-ID` and `APCA-API-SECRET-KEY` in every request |

### `app.rs` — 16 tests

| Test | What it checks |
|---|---|
| `tab_next_wraps` | Account→Watchlist→Positions→Orders→Account |
| `tab_prev_wraps` | Orders→Positions→Watchlist→Account→Orders |
| `tab_from_index_all` | All 4 indices map to correct variant |
| `order_field_next_full_cycle` | Symbol→Side→OrderType→Qty→Price→Submit→Symbol |
| `order_field_prev_full_cycle` | Reverse cycle |
| `filtered_orders_open_statuses` | accepted, pending_new, partially_filled included; filled/canceled excluded |
| `filtered_orders_filled` | Only "filled" status |
| `filtered_orders_cancelled` | canceled, expired, rejected, replaced included |
| `filtered_orders_empty` | No orders → empty vec |
| `push_equity_parses_and_appends` | "100000" → 10000000u64 in history |
| `push_equity_caps_at_120` | 121st push removes first entry |
| `push_equity_ignores_bad_string` | Non-numeric equity → no panic, history unchanged |
| `selected_watchlist_symbol_no_search` | Returns symbol at selected index |
| `selected_watchlist_symbol_with_search` | Filtered list — index into filtered subset |
| `selected_watchlist_symbol_none_selected` | `None` when `TableState` has no selection |
| `selected_order_id_matches_filtered` | Returns id from filtered orders, not raw orders |

### `update.rs` — 38 tests

**Data events:**

| Test | Event | Expected state change |
|---|---|---|
| `account_updated_sets_account` | `AccountUpdated` | `app.account == Some(...)` |
| `account_updated_calls_push_equity` | `AccountUpdated` | `equity_history` grows by 1 |
| `positions_updated_empty_no_select` | `PositionsUpdated([])` | `positions_state.selected() == None` |
| `positions_updated_non_empty_selects_zero` | `PositionsUpdated([p])` | `positions_state.selected() == Some(0)` |
| `orders_updated_auto_selects` | `OrdersUpdated([o])` | `orders_state.selected() == Some(0)` |
| `watchlist_updated_auto_selects` | `WatchlistUpdated(w)` | `watchlist_state.selected() == Some(0)` |
| `trade_update_existing_replaces` | `TradeUpdate(o)` where id exists | Order updated in-place |
| `trade_update_new_prepends` | `TradeUpdate(o)` where id is new | Order inserted at index 0 |
| `market_quote_inserted` | `MarketQuote(q)` | `app.quotes["AAPL"]` exists |
| `quit_event_sets_flag` | `Quit` | `app.should_quit == true` |
| `status_msg_updated` | `StatusMsg("hello")` | `app.status_msg == "hello"` |
| `tick_is_noop` | `Tick` | No state change |

**Global key events:**

| Test | Key | Expected |
|---|---|---|
| `key_q_quits` | `q` | `should_quit` |
| `key_ctrl_c_quits` | `Ctrl-C` | `should_quit` |
| `key_question_opens_help` | `?` | `modal == Some(Modal::Help)` |
| `key_1_switches_account` | `1` | `active_tab == Tab::Account` |
| `key_4_switches_orders` | `4` | `active_tab == Tab::Orders` |
| `key_tab_cycles_forward` | `Tab` | tab advances |
| `key_backtab_cycles_back` | `BackTab` | tab retreats |
| `key_esc_closes_modal` | `Esc` with modal open | `modal == None` |
| `key_r_notifies_and_sets_status` | `r` | `status_msg == "Refreshing…"` |

**Watchlist panel keys:**

| Test | Key | Expected |
|---|---|---|
| `watchlist_j_moves_down` | `j` | selected increments |
| `watchlist_j_clamps_at_end` | `j` at last row | selected stays at `len-1` |
| `watchlist_k_moves_up` | `k` | selected decrements |
| `watchlist_k_clamps_at_zero` | `k` at row 0 | selected stays at 0 |
| `watchlist_g_jumps_top` | `g` | selected == 0 |
| `watchlist_G_jumps_bottom` | `G` | selected == len-1 |
| `watchlist_enter_opens_detail` | `Enter` with selection | `modal == Some(Modal::SymbolDetail(...))` |
| `watchlist_o_opens_order_entry` | `o` | `modal == Some(Modal::OrderEntry(...))` with symbol |
| `watchlist_a_opens_add_symbol` | `a` | `modal == Some(Modal::AddSymbol{...})` |
| `watchlist_d_opens_confirm` | `d` | `modal == Some(Modal::Confirm{...})` |
| `watchlist_slash_starts_search` | `/` | `app.searching == true` |

**Modal key events:**

| Test | Key | Expected |
|---|---|---|
| `modal_tab_advances_field` | `Tab` in OrderEntry | `focused_field` advances |
| `modal_left_right_toggles_side` | `←`/`→` on Side field | `side_buy` toggled |
| `modal_char_appends_to_symbol` | `A` on Symbol field | symbol grows |
| `modal_digit_appends_to_qty` | `5` on Qty field | qty_input grows |
| `modal_non_digit_ignored_in_qty` | `x` on Qty field | qty_input unchanged |
| `modal_backspace_removes_char` | `Backspace` on Symbol | symbol shrinks |
| `search_char_appends` | `A` while searching | `search_query` grows |
| `search_esc_exits` | `Esc` while searching | `searching == false` |
| `search_enter_exits` | `Enter` while searching | `searching == false` |

### `handlers/rest.rs` — 4 tests

| Test | What it checks |
|---|---|
| `poll_once_sends_five_event_types` | AccountUpdated, PositionsUpdated, OrdersUpdated, ClockUpdated, WatchlistUpdated all sent |
| `poll_once_account_error_sends_status_msg` | 500 from /account → StatusMsg sent, no panic |
| `poll_once_empty_watchlist_list_skips` | Empty array from /watchlists → no WatchlistUpdated sent |
| `run_cancels_cleanly` | CancellationToken cancelled → task exits, no hang |

---

## Test Helper Utilities

Place these in a `tests/helpers.rs` or inline in each test module:

```rust
// Minimal App for testing (no real network, no terminal)
pub fn make_test_app() -> App {
    App::new(
        AlpacaConfig {
            base_url: "http://localhost".into(),
            key: "k".into(),
            secret: "s".into(),
            env: AlpacaEnv::Paper,
        },
        Arc::new(tokio::sync::Notify::new()),
    )
}

// Minimal Order for filtering tests
pub fn make_order(id: &str, status: &str) -> Order {
    Order {
        id: id.into(),
        symbol: "AAPL".into(),
        side: "buy".into(),
        qty: Some("10".into()),
        notional: None,
        order_type: "limit".into(),
        limit_price: None,
        status: status.into(),
        submitted_at: None,
        filled_at: None,
        filled_qty: "0".into(),
        time_in_force: "day".into(),
    }
}

// Minimal Watchlist for app tests
pub fn make_watchlist(symbols: &[&str]) -> Watchlist {
    Watchlist {
        id: "11111111-1111-1111-1111-111111111111".into(),
        name: "Test".into(),
        assets: symbols.iter().map(|s| Asset {
            id: "33333333-3333-3333-3333-333333333333".into(),
            symbol: s.to_string(),
            name: format!("{} Corp", s),
            exchange: "NASDAQ".into(),
            asset_class: "us_equity".into(),
            tradable: true,
            shortable: true,
            fractionable: true,
            easy_to_borrow: true,
        }).collect(),
    }
}

// wiremock test config factory
pub fn test_config(base_url: String) -> AlpacaConfig {
    AlpacaConfig { base_url, key: "PKTEST000".into(), secret: "secret000".into(), env: AlpacaEnv::Paper }
}

// Key event factory
pub fn key(code: KeyCode) -> Event {
    Event::Input(KeyEvent::new(code, KeyModifiers::NONE))
}
pub fn ctrl(code: KeyCode) -> Event {
    Event::Input(KeyEvent::new(code, KeyModifiers::CONTROL))
}
```

---

## Running Tests

```bash
# All tests
cargo test

# Only unit tests (no network)
cargo test --lib

# Only integration tests (requires wiremock server — no real network)
cargo test --test client_tests

# Single test by name
cargo test tab_next_wraps

# With output
cargo test -- --nocapture
```

---

## Notes for Implementation

- `App::new()` requires `Arc<Notify>` — tests should pass `Arc::new(Notify::new())` and ignore it
- `AlpacaConfig` fields are all `pub` — construct directly in tests, no need for a builder
- `update()` takes `&mut App` and `Event` — easy to test in isolation with no async
- `handlers/rest::poll_once` is `pub async fn` — directly awaitable in `#[tokio::test]`
- `handlers/rest::run` loops forever — test cancellation with a short `timeout` or immediate `token.cancel()`
- The `events::Event` enum does not implement `Debug` — add `#[derive(Debug)]` before writing tests that use `assert!(... matches!(e, ...))` with a failure message
