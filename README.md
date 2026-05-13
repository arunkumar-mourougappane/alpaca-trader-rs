# alpaca-trader-rs

[![CI](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/ci.yml)
[![Release](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/release.yml/badge.svg)](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/release.yml)
[![Security audit](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/security.yml/badge.svg)](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/security.yml)
[![codecov](https://codecov.io/gh/arunkumar-mourougappane/alpaca-trader-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/arunkumar-mourougappane/alpaca-trader-rs)
![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

An Alpaca Markets trading toolkit for Rust — ships as both an **integratable library** and a **standalone TUI trading app**.

- **Library** (`alpaca_trader_rs` crate): typed async REST client, shared domain types, and WebSocket streaming primitives — embed it in your own Rust application.
- **App** (`alpaca-trader` binary): a full interactive terminal dashboard built on the library, with live account data, positions, orders, watchlist management, and order entry.

---

## Running the App

### Prerequisites

- Rust 1.88+ (`rustup update stable`)
- An Alpaca Markets account — paper trading is free at [alpaca.markets](https://alpaca.markets)

### Installation

```bash
git clone https://github.com/arunkumar-mourougappane/alpaca-trader-rs
cd alpaca-trader-rs
```

### Credential Setup

The app resolves credentials in priority order (highest wins):

| Priority | Method | When to use |
|---|---|---|
| 1 | `ALPACA_API_KEY` + `ALPACA_API_SECRET` env vars | CI, Docker, systemd — single pair for both environments |
| 2 | `LIVE_ALPACA_KEY`/`SECRET` or `PAPER_ALPACA_KEY`/`SECRET` env vars | Per-environment `.env` file on developer machines |
| 3 | OS-native keychain | Desktop users — keys are saved once and reused |
| 4 | Interactive TTY prompt (first run) | No credentials configured yet — app prompts and offers to save to keychain |

**Option A — First run (interactive, recommended for desktop):**

Just run the app. If no credentials are found, it prompts for your API key and secret, then
offers to save them to the OS keychain (macOS Keychain, Windows Credential Store, or Linux keyutils).

```bash
./run.sh --paper   # prompted for paper keys on first run
./run.sh           # prompted for live keys on first run
```

**Option B — `.env` file (recommended for development):**

```bash
cp .env.example .env
# Edit .env and fill in your API keys — see docs/credentials-setup.md
```

**Option C — Environment variables (CI / containers):**

```bash
export ALPACA_API_KEY=your-key-id
export ALPACA_API_SECRET=your-secret-key
./run.sh --paper
```

> See [docs/credentials-setup.md](docs/credentials-setup.md) for obtaining keys from the Alpaca dashboard.

### Run

```bash
./run.sh           # live trading (real money — default)
./run.sh --paper   # paper trading (simulated funds)
./run.sh --live    # same as default; accepted for backwards compatibility
```

The header badge shows **[PAPER]** in cyan or **[LIVE]** in red at all times.

### Managing Stored Credentials

To clear credentials saved in the OS keychain:

```bash
alpaca-trader --reset paper   # remove paper keychain entries
alpaca-trader --reset live    # remove live keychain entries
```

If credentials were loaded from a `.env` file or environment variable instead, the command
prints the variable names to unset and the file to edit.

### Key Bindings

| Key | Action |
|-----|--------|
| `1` / `2` / `3` | Switch panel (Account / Watchlist / Positions) — or switch Orders sub-tab when on Orders panel |
| `4` | Switch to Orders panel |
| `Tab` / `Shift-Tab` | Cycle panels forward / backward |
| `j` / `k` or `↑` / `↓` | Navigate rows |
| `g` / `G` | Jump to first / last row |
| `Enter` | Open symbol detail |
| `o` | New order (pre-fills selected symbol) |
| `s` | SELL SHORT order (Positions panel) |
| `c` | Cancel selected order |
| `a` | Add symbol to watchlist |
| `d` | Remove symbol from watchlist |
| `/` | Search / filter watchlist |
| `r` | Force refresh |
| `?` | Help overlay |
| `A` | About overlay |
| `Esc` | Close modal |
| `q` / `Ctrl-C` | Quit |

Full interaction spec (including mouse): [docs/ui-mockups.md](docs/ui-mockups.md)

---

## Library Usage

Add to your `Cargo.toml` (requires a Collaboration Agreement — see [Licensing](#licensing)):

```toml
[dependencies]
alpaca-trader-rs = { git = "https://github.com/arunkumar-mourougappane/alpaca-trader-rs" }
tokio = { version = "1", features = ["full"] }
dotenvy = "0.15"
```

### Fetch account info

```rust
use alpaca_trader_rs::client::AlpacaClient;
use alpaca_trader_rs::config::AlpacaConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let config = AlpacaConfig::from_env()?;
    let client = AlpacaClient::new(config);

    let account = client.get_account().await?;
    println!("Equity:        ${}", account.equity);
    println!("Buying power:  ${}", account.buying_power);

    Ok(())
}
```

### Place an order

```rust
use alpaca_trader_rs::types::{OrderRequest, OrderSide, OrderType, TimeInForce};

let order = client.submit_order(&OrderRequest {
    symbol: "AAPL".into(),
    qty: Some("10".into()),
    notional: None,
    side: OrderSide::Buy.as_str().into(),
    order_type: OrderType::Limit.as_str().into(),
    time_in_force: TimeInForce::Day.as_str().into(),
    limit_price: Some("185.00".into()),
}).await?;

println!("Order submitted: {}", order.id);
```

### Manage watchlists

```rust
let summaries = client.list_watchlists().await?;
let wl = client.get_watchlist(&summaries[0].id).await?;

for asset in &wl.assets {
    println!("{} — {} ({})", asset.symbol, asset.name, asset.exchange);
}

client.add_to_watchlist(&wl.id, "NVDA").await?;
client.remove_from_watchlist(&wl.id, "TLRY").await?;
```

### Public Library API

| Module | Exposed items |
|---|---|
| `config` | `AlpacaConfig`, `AlpacaEnv` |
| `client` | `AlpacaClient` — `get_account()`, `get_positions()`, `get_orders()`, `submit_order()`, `cancel_order()`, `get_clock()`, `list_watchlists()`, `get_watchlist()`, `add_to_watchlist()`, `remove_from_watchlist()` |
| `types` | `AccountInfo`, `Position`, `Order`, `OrderRequest`, `OrderSide`, `OrderType`, `TimeInForce`, `Quote`, `MarketClock`, `Watchlist`, `WatchlistSummary`, `Asset` |
| `events` | `Event` — unified event enum consumed by the TUI app |

> `stream::MarketStream` and `stream::AccountStream` (WebSocket live data) are Phase 2.

---

## Crate Structure

```
alpaca-trader-rs/
├── src/
│   ├── lib.rs              # Library root — public API
│   ├── main.rs             # Binary entry point — TUI app
│   ├── credentials.rs      # Credential resolution: env vars → keychain → TTY prompt  [app-only]
│   ├── config.rs           # AlpacaConfig: env resolution, paper/live selection
│   ├── client.rs           # AlpacaClient: all REST methods
│   ├── types.rs            # Shared domain types (serde-deserializable)
│   ├── events.rs           # Event enum
│   ├── app.rs              # App state — TEA Model          [app-only]
│   ├── update.rs           # update(state, event) + key routing  [app-only]
│   ├── input/              # Per-panel keyboard input handlers    [app-only]
│   │   ├── mod.rs          # send_command() + pub re-exports
│   │   ├── watchlist.rs    # handle_watchlist_key()
│   │   ├── positions.rs    # handle_positions_key()
│   │   ├── orders.rs       # handle_orders_key()
│   │   ├── modal.rs        # handle_modal_key()
│   │   └── search.rs       # handle_search_key()
│   ├── handlers/
│   │   ├── input.rs        # crossterm EventStream → Event  [app-only]
│   │   └── rest.rs         # Periodic REST polling task     [app-only]
│   └── ui/                 #                                [app-only]
│       ├── mod.rs          # render(frame, app) + popup_area()
│       ├── dashboard.rs    # Header, tab bar, status bar
│       ├── account.rs      # Account panel + sparkline
│       ├── watchlist.rs    # Watchlist table + search
│       ├── positions.rs    # Positions table + totals
│       ├── orders.rs       # Orders table + sub-tabs
│       ├── modals.rs       # Order entry, detail, help, confirm modals
│       └── theme.rs        # Colours and styles
├── tests/
│   └── client_tests.rs     # AlpacaClient integration tests (wiremock)
├── docs/                   # Full documentation
├── .env.example            # Credential template
├── run.sh                  # Run script (--paper / --live)
├── Cargo.toml
├── LICENSE.md
├── LICENSE-MIT
├── LICENSE-APACHE
└── README.md
```

---

## Environment Variables

Credentials are loaded from the environment (or a `.env` file via `dotenvy`). Only the
variables for the active environment are used — the opposing set is ignored.

> ⚠️ **Breaking change from v0.2.0**: the default environment is now **live**.
> Users who previously relied on the paper default must pass `--paper` explicitly.
> `ALPACA_ENV` is no longer read.

### Unified pair (highest priority)

| Variable | Description |
|---|---|
| `ALPACA_API_KEY` | API key ID — used for whichever environment (`--paper` or live) is active |
| `ALPACA_API_SECRET` | API secret key — paired with `ALPACA_API_KEY` |

### Per-environment variables

| Variable | Description |
|---|---|
| `PAPER_ALPACA_ENDPOINT` | `https://paper-api.alpaca.markets/v2` — optional override |
| `PAPER_ALPACA_KEY` | Paper API key ID |
| `PAPER_ALPACA_SECRET` | Paper API secret key |
| `LIVE_ALPACA_ENDPOINT` | `https://api.alpaca.markets` — optional override |
| `LIVE_ALPACA_KEY` | Live API key ID |
| `LIVE_ALPACA_SECRET` | Live API secret key |

---

## Status

| Feature | Status |
|---|---|
| REST client (`AlpacaClient`) | Done |
| TUI shell — header, tabs, status bar | Done |
| Account panel + equity sparkline | Done |
| Watchlist panel + live search | Done |
| Positions panel + totals footer | Done |
| Orders panel + Open/Filled/Cancelled sub-tabs | Done |
| Order Entry modal | Done |
| Symbol Detail modal | Done |
| Help overlay | Done |
| Paper / Live switching (`run.sh --paper/--live`) | Done |
| Clippy clean | Done |
| Test strategy documented | Done |
| Unit + integration tests (327 tests) | Done |
| Orders panel 1/2/3 sub-tab key fix | Done |
| GitHub Actions CI + security audit | Done |
| Code coverage with cargo-llvm-cov + Codecov | Done |
| WebSocket integration tests (auth, cancel, reconnect) | Done |
| Release workflow — pre-compiled binaries on tag push | Done |
| Status message auto-dismiss (3 s TTL) | Done |
| WebSocket stream connection status in header | Done |
| Order Entry: Time-in-Force (DAY/GTC) selection | Done |
| Order Entry: market-closed warning + DAY order block | Done |
| Equity sparkline pre-populated from portfolio history API | Done |
| WebSocket market data streaming | Done |
| WebSocket account/trade stream | Done |
| Live order submission | Done |
| Watchlist add/remove (wired to REST) | Done |
| OS-native keychain credential storage (macOS / Windows / Linux) | Done |
| Interactive first-run credential prompt with keychain save offer | Done |
| `ALPACA_API_KEY` / `ALPACA_API_SECRET` unified env vars | Done |
| `--reset <paper\|live>` CLI flag to clear stored keychain credentials | Done |
| Account panel Day P&L, Open P&L, Account # | Done |
| Watchlist Volume + Change% columns (replacing Ask/Bid) | Done |
| Header PRE-MARKET / AFTER-HOURS state detection | Done |
| Symbol Detail OHLCV + intraday sparkline + watchlist toggle (`w`) | Done |
| Fix: intraday sparkline stuck on "Loading…" | Done |
| `s` key SELL SHORT from Positions panel | Done |
| ↑/↓ arrow keys in Order Entry dropdowns | Done |
| About modal (`A` key) | Done |

---

## Documentation

| Document | Description |
|---|---|
| [docs/architecture.md](docs/architecture.md) | System design, library/app boundary, data flow, crate choices |
| [docs/credentials-setup.md](docs/credentials-setup.md) | Obtaining and configuring Alpaca API keys |
| [docs/ui-mockups.md](docs/ui-mockups.md) | ASCII mockups and full keyboard/mouse interaction spec |
| [docs/api-research.md](docs/api-research.md) | REST endpoint shapes and live test results |
| [docs/testing.md](docs/testing.md) | Testing strategy: mock patterns, crate rationale, full test case inventory |
| [docs/licensing.md](docs/licensing.md) | License overview and contribution terms |

---

## Licensing

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0
- [MIT license](LICENSE-MIT) or http://opensource.org/licenses/MIT

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
