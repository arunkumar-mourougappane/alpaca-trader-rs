# alpaca-trader-rs

[![CI](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/ci.yml)
[![Security audit](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/security.yml/badge.svg)](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/security.yml)
![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)
![License: Proprietary](https://img.shields.io/badge/license-Proprietary-red.svg)

An Alpaca Markets trading toolkit for Rust — ships as both an **integratable library** and a **standalone TUI trading app**.

- **Library** (`alpaca_trader_rs` crate): typed async REST client, shared domain types, and WebSocket streaming primitives — embed it in your own Rust application.
- **App** (`alpaca-trader` binary): a full interactive terminal dashboard built on the library, with live account data, positions, orders, watchlist management, and order entry.

> **Proprietary software.** All use beyond viewing the source requires explicit written permission from the author. See [LICENSE.md](LICENSE.md) and [docs/licensing.md](docs/licensing.md).

---

## Running the App

### Prerequisites

- Rust 1.88+ (`rustup update stable`)
- An Alpaca Markets account — paper trading is free at [alpaca.markets](https://alpaca.markets)

### Setup

```bash
git clone https://github.com/arunkumar-mourougappane/alpaca-trader-rs
cd alpaca-trader-rs

cp .env.example .env
# Fill in your API keys — see docs/credentials-setup.md
```

### Run

```bash
./run.sh           # paper trading (default)
./run.sh --paper   # explicitly paper
./run.sh --live    # live trading (real money)
```

The header badge shows **[PAPER]** in cyan or **[LIVE]** in red at all times.

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
| `c` | Cancel selected order |
| `a` | Add symbol to watchlist |
| `d` | Remove symbol from watchlist |
| `/` | Search / filter watchlist |
| `r` | Force refresh |
| `?` | Help overlay |
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
└── README.md
```

---

## Environment Variables

Stored in `.env` with `LIVE_` / `PAPER_` prefixes. The `--paper` / `--live` flag to `run.sh` (or `ALPACA_ENV` in `.env`) selects which set is active.

| Variable | Description |
|---|---|
| `ALPACA_ENV` | `paper` (default) or `live` |
| `PAPER_ALPACA_ENDPOINT` | `https://paper-api.alpaca.markets/v2` |
| `PAPER_ALPACA_KEY` | Paper API key ID |
| `PAPER_ALPACA_SECRET` | Paper API secret key |
| `LIVE_ALPACA_ENDPOINT` | `https://api.alpaca.markets` |
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
| Unit + integration tests (101 tests) | Done |
| Orders panel 1/2/3 sub-tab key fix | Done |
| GitHub Actions CI + security audit | Done |
| WebSocket market data streaming | Phase 2 |
| WebSocket account/trade stream | Phase 2 |
| Live order submission | Phase 2 |
| Watchlist add/remove (wired to REST) | Phase 2 |

---

## Documentation

| Document | Description |
|---|---|
| [docs/architecture.md](docs/architecture.md) | System design, library/app boundary, data flow, crate choices |
| [docs/credentials-setup.md](docs/credentials-setup.md) | Obtaining and configuring Alpaca API keys |
| [docs/ui-mockups.md](docs/ui-mockups.md) | ASCII mockups and full keyboard/mouse interaction spec |
| [docs/api-research.md](docs/api-research.md) | REST endpoint shapes and live test results |
| [docs/testing.md](docs/testing.md) | Testing strategy: mock patterns, crate rationale, full test case inventory |
| [docs/licensing.md](docs/licensing.md) | License types, fees, and how to request a Collaboration Agreement |

---

## Licensing

This project is proprietary software. Source code is available for evaluation only.

- **Using the library or app** requires Explicit Permission from the author.
- **Commercial use** requires a paid Collaboration Agreement.
- **Forking** (public or private) requires Explicit Permission and may incur a fee.

To request a license: **arun.mylegend1990@gmail.com**

Full terms: [LICENSE.md](LICENSE.md)
