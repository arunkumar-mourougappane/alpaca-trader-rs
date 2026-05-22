# alpaca-trader-rs

[![CI](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/ci.yml)
[![Release](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/release.yml/badge.svg)](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/release.yml)
[![Security audit](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/security.yml/badge.svg)](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/security.yml)
[![codecov](https://codecov.io/gh/arunkumar-mourougappane/alpaca-trader-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/arunkumar-mourougappane/alpaca-trader-rs)
[![Crates.io](https://img.shields.io/crates/v/alpaca-trader-rs.svg)](https://crates.io/crates/alpaca-trader-rs)
![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

An Alpaca Markets trading toolkit for Rust — ships as both an **integratable library** and a **standalone TUI trading app**.

- **Library** (`alpaca_trader_rs` crate): typed async REST client, shared domain types, and WebSocket streaming primitives — embed it in your own Rust application.
- **App** (`alpaca-trader` binary): a full interactive terminal dashboard built on the library, with live account data, positions, orders, watchlist management, and order entry.

---

## Installing the App

### From crates.io (recommended)

```bash
cargo install alpaca-trader-rs
```

This compiles and installs the `alpaca-trader` binary to `~/.cargo/bin/`. Requires Rust 1.88+.

### Pre-compiled binaries

Download the latest binary for your platform from the [Releases page](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases).

### From source

```bash
git clone https://github.com/arunkumar-mourougappane/alpaca-trader-rs
cd alpaca-trader-rs
cargo build --release
# binary at: target/release/alpaca-trader
```

---

## Credential Setup

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
alpaca-trader --paper   # prompted for paper keys on first run
alpaca-trader           # prompted for live keys on first run
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
alpaca-trader --paper
```

> See [docs/credentials-setup.md](docs/credentials-setup.md) for obtaining keys from the Alpaca dashboard.

---

## Running

```bash
alpaca-trader           # live trading (real money — default)
alpaca-trader --paper   # paper trading (simulated funds)
alpaca-trader --dry-run # simulate order submissions (no real orders sent)
```

The header badge shows **[PAPER]** in cyan or **[LIVE]** in red at all times.

> If you installed from source, use `./run.sh --paper` / `./run.sh` instead of `alpaca-trader`.

### Managing Stored Credentials

```bash
alpaca-trader --reset paper   # remove paper keychain entries
alpaca-trader --reset live    # remove live keychain entries
```

---

## Key Bindings

### Global

| Key | Action |
|-----|--------|
| `1` / `2` / `3` | Switch panel (Account / Watchlist / Positions) — or switch Orders sub-tab when on Orders panel |
| `4` | Switch to Orders panel |
| `Tab` / `Shift-Tab` | Cycle panels forward / backward |
| `j` / `k` or `↑` / `↓` | Navigate rows |
| `gg` / `G` | Jump to first / last row |
| `Enter` | Open symbol / position detail |
| `o` | New order (pre-fills selected symbol) |
| `c` | Copy symbol to clipboard (Positions / Watchlist / Account) |
| `c` | Cancel selected order (Orders panel) |
| `a` | Add symbol to watchlist |
| `d` | Remove symbol from watchlist |
| `/` | Symbol filter on Watchlist; global symbol search on all other panels |
| `Ctrl-F` | Global symbol search (any panel) |
| `r` | Force refresh |
| `?` | Help overlay |
| `A` | About overlay |
| `Esc` | Close modal / clear filter |
| `T` | Toggle colour theme |
| `q` / `Ctrl-C` | Quit |

### Sorting (Positions & Orders panels)

| Key | Action |
|-----|--------|
| `s` | Cycle sort column |
| `S` | Toggle sort direction (asc / desc) |

### Orders panel

| Key | Action |
|-----|--------|
| `f` | Activate symbol filter (type to filter, Enter/Esc to clear) |

### Account panel — equity chart

| Key | Action |
|-----|--------|
| `←` / `→` or `h` / `l` | Move crosshair left / right |
| `p` | Cycle chart range: 1D → 1W → 1M → YTD |
| `Esc` | Clear crosshair |
| Mouse click | Set crosshair to clicked column |

### Mouse

| Action | Effect |
|--------|--------|
| Single click | Select row |
| Double-click row | Open detail modal (same as Enter) |
| Click outside modal | Dismiss Help / About / Symbol Detail / Position Detail |

Full interaction spec: [docs/ui-mockups.md](docs/ui-mockups.md)

---

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
alpaca-trader-rs = "0.6"
tokio = { version = "1", features = ["full"] }
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
| `stream` | `MarketStream`, `AccountStream` — WebSocket live data |

---

## Crate Structure

```
alpaca-trader-rs/
├── src/
│   ├── lib.rs              # Library root — public API
│   ├── main.rs             # Binary entry point — TUI app
│   ├── credentials.rs      # Credential resolution: env vars → keychain → TTY prompt
│   ├── config.rs           # AlpacaConfig: env resolution, paper/live selection
│   ├── client.rs           # AlpacaClient: all REST methods
│   ├── types.rs            # Shared domain types (serde-deserializable)
│   ├── events.rs           # Event enum
│   ├── app.rs              # App state — TEA Model
│   ├── update.rs           # update(state, event) + key routing
│   ├── input/              # Per-panel keyboard input handlers
│   │   ├── mod.rs
│   │   ├── watchlist.rs
│   │   ├── positions.rs
│   │   ├── orders.rs
│   │   ├── modal.rs
│   │   └── search.rs
│   ├── handlers/
│   │   ├── input.rs        # crossterm EventStream → Event
│   │   └── rest.rs         # Periodic REST polling task
│   └── ui/
│       ├── mod.rs          # render(frame, app) + popup_area()
│       ├── dashboard.rs    # Header, tab bar, status bar
│       ├── account.rs      # Account panel + sparkline
│       ├── watchlist.rs    # Watchlist table + search
│       ├── positions.rs    # Positions table + totals
│       ├── orders.rs       # Orders table + sub-tabs
│       ├── modals.rs       # Order entry, detail, help, about, confirm modals
│       └── theme.rs        # Colours and styles
├── tests/
│   └── client_tests.rs     # AlpacaClient integration tests (wiremock)
├── docs/                   # Full documentation
├── .env.example            # Credential template
├── run.sh                  # Run script (--paper / --live)
├── Cargo.toml
├── LICENSE-MIT
├── LICENSE-APACHE
└── README.md
```

---

## Environment Variables

Credentials are loaded from the environment (or a `.env` file via `dotenvy`). Only the
variables for the active environment are used — the opposing set is ignored.

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

## Features

| Feature | Status |
|---|---|
| Typed async REST client (`AlpacaClient`) | ✅ |
| TUI — header, tabs, status bar, braille line charts | ✅ |
| Account panel — equity, buying power, cash, long/short market value | ✅ |
| Account panel — Day P&L, Open P&L, day-trade count (X/3), PDT flag | ✅ |
| Equity chart crosshair — keyboard navigation + mouse click | ✅ |
| Equity chart range toggle: 1D / 1W / 1M / YTD (`p` key) | ✅ |
| Watchlist panel — Volume, Change%, live search | ✅ |
| Positions panel with totals footer and column sorting | ✅ |
| Position detail modal — OHLCV stats + intraday chart | ✅ |
| Orders panel — Open / Filled / Cancelled sub-tabs with column sorting | ✅ |
| Orders panel — symbol filter (`f` key) | ✅ |
| Order Entry modal with Side, Type, TIF dropdowns (↑/↓) | ✅ |
| Symbol Detail modal — OHLCV, intraday chart, watchlist toggle | ✅ |
| Global symbol search modal (Ctrl-F / `/`) | ✅ |
| Help and About overlays | ✅ |
| Mouse support — row selection, double-click to open detail, outside-click to dismiss | ✅ |
| Paper / Live switching (`--paper` / `--live`) | ✅ |
| `--dry-run` mode — simulate orders without sending to Alpaca | ✅ |
| Persistent user preferences (TOML config file) | ✅ |
| Runtime colour theme switching (`T` key) | ✅ |
| Header market-session state (PRE-MARKET / OPEN / AFTER-HOURS / CLOSED) | ✅ |
| Instant UI redraw on terminal resize | ✅ |
| WebSocket market data + account/trade streaming | ✅ |
| Live order submission and cancellation | ✅ |
| Watchlist add / remove (wired to REST) | ✅ |
| OS-native keychain credential storage | ✅ |
| Interactive first-run credential prompt | ✅ |
| Windows, macOS, and Linux support | ✅ |
| GitHub Actions CI, security audit, Codecov (Linux + Windows), release builds | ✅ |
| 800 tests (unit + integration) | ✅ |

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

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines and our [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community standards.

---

## Licensing

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0
- [MIT license](LICENSE-MIT) or http://opensource.org/licenses/MIT

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
