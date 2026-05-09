# alpaca-trader-rs

An Alpaca Markets trading toolkit for Rust — ships as both an **integratable library** and a **standalone TUI trading app**.

- **Library** (`alpaca-trader-rs` crate): typed async client for Alpaca REST and WebSocket APIs, shared domain types, and streaming primitives — embed it in your own Rust application.
- **App** (`alpaca-trader` binary): a full terminal UI dashboard built on top of the library, with live positions, orders, watchlist, market data streaming, and order entry.

> This software is proprietary. All use beyond viewing the source requires explicit written permission from the author. See [LICENSE.md](LICENSE.md) and [docs/licensing.md](docs/licensing.md).

---

## Library Usage

Add to your `Cargo.toml` (requires a Collaboration Agreement — see [Licensing](#licensing)):

```toml
[dependencies]
alpaca-trader-rs = { git = "https://github.com/amouroug/alpaca-trader-rs" }
tokio = { version = "1", features = ["full"] }
```

### Fetch account info

```rust
use alpaca_trader_rs::client::AlpacaClient;
use alpaca_trader_rs::config::AlpacaConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AlpacaConfig::from_env()?; // reads ALPACA_ENV + prefixed vars
    let client = AlpacaClient::new(config);

    let account = client.get_account().await?;
    println!("Equity: ${}", account.equity);
    println!("Buying power: ${}", account.buying_power);

    Ok(())
}
```

### Stream real-time market quotes

```rust
use alpaca_trader_rs::stream::MarketStream;
use alpaca_trader_rs::types::Quote;

let mut stream = MarketStream::connect(&config).await?;
stream.subscribe(&["AAPL", "TSLA", "NVDA"]).await?;

while let Some(event) = stream.next().await {
    match event? {
        Quote { symbol, ask_price, bid_price, .. } => {
            println!("{symbol}: bid={bid_price} ask={ask_price}");
        }
        _ => {}
    }
}
```

### Place an order

```rust
use alpaca_trader_rs::client::AlpacaClient;
use alpaca_trader_rs::types::{OrderRequest, OrderSide, OrderType, TimeInForce};

let order = client.submit_order(OrderRequest {
    symbol: "AAPL".into(),
    qty: Some(10),
    notional: None,
    side: OrderSide::Buy,
    order_type: OrderType::Limit,
    time_in_force: TimeInForce::Day,
    limit_price: Some(185.00),
}).await?;

println!("Order submitted: {}", order.id);
```

### Manage watchlists

```rust
let watchlists = client.list_watchlists().await?;
let wl = client.get_watchlist(&watchlists[0].id).await?;

for asset in &wl.assets {
    println!("{} — {} ({})", asset.symbol, asset.name, asset.exchange);
}

client.add_to_watchlist(&wl.id, "NVDA").await?;
```

### Public Library API

| Module | Types / Functions |
|---|---|
| `client::AlpacaClient` | `get_account()`, `get_positions()`, `get_orders()`, `submit_order()`, `cancel_order()`, `get_clock()`, `list_watchlists()`, `get_watchlist()`, `add_to_watchlist()`, `remove_from_watchlist()` |
| `config::AlpacaConfig` | `from_env()`, `paper()`, `live()` |
| `types` | `AccountInfo`, `Position`, `Order`, `OrderRequest`, `OrderSide`, `OrderType`, `TimeInForce`, `Quote`, `Bar`, `MarketClock`, `Watchlist`, `Asset` |
| `stream::MarketStream` | `connect()`, `subscribe()`, `next()` — real-time quotes and bars |
| `stream::AccountStream` | `connect()`, `next()` — order fills and account updates |
| `events::Event` | Unified event enum used by both the stream types and the TUI app |

---

## TUI App

A full interactive terminal dashboard built on the library.

### Prerequisites

- Rust 1.85+ (`rustup update stable`)
- An Alpaca Markets account — paper trading is free and available immediately at [alpaca.markets](https://alpaca.markets)

### Quick Start

```bash
git clone https://github.com/amouroug/alpaca-trader-rs
cd alpaca-trader-rs

cp .env.example .env
# Fill in your paper trading keys — see docs/credentials-setup.md

set -a && source .env && set +a
cargo run --release --bin alpaca-trader
```

The header shows **[PAPER]** in cyan when running against the paper environment.

### Key Bindings

| Key | Action |
|-----|--------|
| `1` / `2` / `3` / `4` | Switch panel: Account / Watchlist / Positions / Orders |
| `Tab` / `Shift-Tab` | Cycle panels |
| `j` / `k` or `↑` / `↓` | Navigate rows |
| `g` / `G` | Jump to first / last row |
| `Enter` | Open symbol detail |
| `o` | New order |
| `c` | Cancel selected order |
| `a` | Add symbol to watchlist |
| `d` | Remove symbol from watchlist |
| `/` | Search / filter |
| `r` | Force refresh |
| `?` | Help overlay |
| `Esc` | Close modal |
| `q` / `Ctrl-C` | Quit |

Full interaction spec (including mouse): [docs/ui-mockups.md](docs/ui-mockups.md)

### Screenshots

*Coming once the TUI is implemented.*

---

## Crate Structure

```
alpaca-trader-rs/
├── src/
│   ├── lib.rs              # Library root — public API surface
│   ├── main.rs             # Binary entry point — TUI app
│   ├── config.rs           # AlpacaConfig: env resolution, endpoint selection
│   ├── client.rs           # AlpacaClient: REST methods
│   ├── types.rs            # Shared domain types
│   ├── events.rs           # Event enum (shared by library and app)
│   ├── stream/
│   │   ├── mod.rs
│   │   ├── market.rs       # MarketStream: quotes, bars via WebSocket
│   │   └── account.rs      # AccountStream: order fills, account updates
│   ├── app.rs              # (app-only) App state — the TEA Model
│   ├── update.rs           # (app-only) update(state, event)
│   └── ui/
│       ├── mod.rs          # (app-only) render(frame, app)
│       ├── dashboard.rs
│       ├── account.rs
│       ├── watchlist.rs
│       ├── positions.rs
│       ├── orders.rs
│       ├── modals.rs
│       └── theme.rs
├── docs/                   # Full documentation
├── .env.example
├── Cargo.toml
├── LICENSE.md
└── README.md               # This file
```

The boundary is intentional: `client`, `config`, `types`, `events`, and `stream` are part of the public library. `app`, `update`, and `ui` are app-only and not exposed by `lib.rs`.

---

## Environment Variables

| Variable | Description |
|---|---|
| `ALPACA_ENV` | `paper` (default) or `live` |
| `LIVE_ALPACA_ENDPOINT` | `https://api.alpaca.markets` |
| `LIVE_ALPACA_KEY` | Live API key ID |
| `LIVE_ALPACA_SECRET` | Live API secret key |
| `PAPER_ALPACA_ENDPOINT` | `https://paper-api.alpaca.markets/v2` |
| `PAPER_ALPACA_KEY` | Paper API key ID |
| `PAPER_ALPACA_SECRET` | Paper API secret key |

---

## Documentation

| Document | Description |
|---|---|
| [docs/architecture.md](docs/architecture.md) | System design, library/app boundary, data flow, crate choices |
| [docs/credentials-setup.md](docs/credentials-setup.md) | Obtaining and configuring Alpaca API keys |
| [docs/ui-mockups.md](docs/ui-mockups.md) | ASCII mockups and keyboard/mouse interaction spec |
| [docs/api-research.md](docs/api-research.md) | REST endpoint shapes, WebSocket protocol, live test results |
| [docs/licensing.md](docs/licensing.md) | License types, fees, and how to request a Collaboration Agreement |

---

## Licensing

This project is proprietary software. Source code is available for evaluation only.

- **Using the library or app** requires Explicit Permission from the author.
- **Commercial use** requires a paid Collaboration Agreement.
- **Forking** (public or private) requires Explicit Permission and may incur a fee.

To request a license: **arun.mylegend1990@gmail.com**

Full terms: [LICENSE.md](LICENSE.md)
