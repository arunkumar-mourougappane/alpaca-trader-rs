# alpaca-trader-rs

[![CI](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/ci.yml)
[![Release](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/release.yml/badge.svg)](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/release.yml)
[![Security audit](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/security.yml/badge.svg)](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/actions/workflows/security.yml)
[![codecov](https://codecov.io/gh/arunkumar-mourougappane/alpaca-trader-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/arunkumar-mourougappane/alpaca-trader-rs)
[![Crates.io](https://img.shields.io/crates/v/alpaca-trader-rs.svg)](https://crates.io/crates/alpaca-trader-rs)
![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)

An Alpaca Markets trading toolkit for Rust — ships as both an **integratable library** and a **standalone TUI trading app**.

- **Library** (`alpaca_trader_rs` crate): typed async REST client, shared domain types, and WebSocket streaming primitives.
- **App** (`alpaca-trader` binary): full interactive terminal dashboard with live account data, positions, orders, watchlist management, and order entry.

---

## Contents

- [Features](#features)
- [Installing](#installing)
- [Credentials](#credentials)
- [Running](#running)
- [Key Bindings](#key-bindings)
- [Library Usage](#library-usage)
- [Crate Structure](#crate-structure)
- [Documentation](#documentation)
- [Contributing](#contributing)
- [Licensing](#licensing)

---

## Features

### TUI App

| Feature | |
|---|---|
| Account panel — equity, buying power, cash, long/short market value | ✅ |
| Account panel — Day P&L, Open P&L, day-trade count (X/3), PDT flag | ✅ |
| Equity chart with crosshair (keyboard + mouse) and range toggle (1D/1W/1M/YTD) | ✅ |
| Watchlist panel — Volume, Change%, live search | ✅ |
| Positions panel — totals footer, column sorting, position detail modal | ✅ |
| Orders panel — Open/Filled/Cancelled sub-tabs, column sorting, symbol filter | ✅ |
| Order Entry modal — Side, Type, TIF dropdowns | ✅ |
| Symbol Detail modal — OHLCV, intraday chart, watchlist toggle | ✅ |
| Global symbol search (Ctrl-F / `/`) | ✅ |
| Help and About overlays | ✅ |
| Mouse — row selection, double-click to open detail, click outside to dismiss | ✅ |
| Runtime colour theme switching (`T`) | ✅ |
| Header market-session state (PRE-MARKET / OPEN / AFTER-HOURS / CLOSED) | ✅ |
| Instant redraw on terminal resize | ✅ |

### Library

| Feature | |
|---|---|
| Typed async REST client (`AlpacaClient`) | ✅ |
| WebSocket market data + account/trade streaming | ✅ |
| Live order submission and cancellation | ✅ |
| Watchlist add / remove | ✅ |

### Infrastructure

| Feature | |
|---|---|
| Paper / Live switching (`--paper` / `--live`) | ✅ |
| `--dry-run` mode — simulate orders without sending to Alpaca | ✅ |
| OS-native keychain credential storage | ✅ |
| Interactive first-run credential prompt | ✅ |
| Persistent user preferences (TOML config file) | ✅ |
| Windows, macOS, and Linux support | ✅ |
| GitHub Actions CI, security audit, Codecov, release builds | ✅ |
| 800 tests (unit + integration) | ✅ |

---

## Installing

**From crates.io (recommended):**

```bash
cargo install alpaca-trader-rs
```

Installs the `alpaca-trader` binary to `~/.cargo/bin/`. Requires Rust 1.88+.

**Pre-compiled binaries:** Download from the [Releases page](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases).

**From source:**

```bash
git clone https://github.com/arunkumar-mourougappane/alpaca-trader-rs
cd alpaca-trader-rs
cargo build --release
# binary at: target/release/alpaca-trader
```

---

## Credentials

Credentials are resolved in priority order (highest wins):

| Priority | Source | When to use |
|---|---|---|
| 1 | `ALPACA_API_KEY` + `ALPACA_API_SECRET` | CI, Docker, systemd |
| 2 | `LIVE_ALPACA_KEY`/`SECRET` or `PAPER_ALPACA_KEY`/`SECRET` | Developer `.env` file |
| 3 | OS-native keychain | Desktop — saved once on first run |
| 4 | Interactive TTY prompt | No credentials found yet |

**First run:** Just run the app — it prompts and saves to the OS keychain automatically.

**`.env` file:**

```bash
cp .env.example .env   # fill in your keys
```

**Environment variables (CI):**

```bash
export ALPACA_API_KEY=your-key-id
export ALPACA_API_SECRET=your-secret-key
```

```bash
alpaca-trader --reset paper   # remove paper keychain entries
alpaca-trader --reset live    # remove live keychain entries
```

> Full credential setup guide: [docs/credentials-setup.md](docs/credentials-setup.md)

---

## Running

```bash
alpaca-trader           # live trading (real money — default)
alpaca-trader --paper   # paper trading (simulated funds)
alpaca-trader --dry-run # simulate order submissions, no real orders sent
```

The header badge shows **[PAPER]** in cyan or **[LIVE]** in red at all times.

```bash
alpaca-trader --reset paper   # remove paper keychain entries
alpaca-trader --reset live    # remove live keychain entries
```

> If installed from source, use `./run.sh --paper` / `./run.sh` instead.

---

## Key Bindings

| Key | Action |
|-----|--------|
| `1`–`4` / `Tab` / `Shift-Tab` | Switch panels |
| `j`/`k` · `↑`/`↓` · `gg`/`G` | Navigate · jump first/last |
| `Enter` | Open symbol / position detail |
| `o` | New order · `c` Copy symbol / Cancel order · `a`/`d` Add/remove from watchlist |
| `/` · `Ctrl-F` | Search (watchlist filter on Watchlist tab; global search on all others) |
| `s` / `S` | Cycle sort column / toggle direction (Positions & Orders) |
| `f` | Symbol filter (Orders panel) |
| `←`/`→` · `p` | Chart crosshair · cycle range 1D/1W/1M/YTD (Account panel) |
| `r` · `T` · `?`/`A` | Refresh · theme · Help/About |
| `Esc` | Dismiss modal / clear filter |
| `q` / `Ctrl-C` | Quit |

**Mouse:** click to select · double-click to open detail · click outside modal to dismiss.

> Full keyboard/mouse interaction spec: [docs/ui-mockups.md](docs/ui-mockups.md)

---

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
alpaca-trader-rs = "0.6"
tokio = { version = "1", features = ["full"] }
```

Quick example — fetch account info:

```rust
use alpaca_trader_rs::{client::AlpacaClient, config::AlpacaConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let client = AlpacaClient::new(AlpacaConfig::from_env()?);
    let account = client.get_account().await?;
    println!("Equity: ${} | Buying power: ${}", account.equity, account.buying_power);
    Ok(())
}
```

> Full API reference, order placement, and watchlist examples: [docs/library.md](docs/library.md)

---

## Crate Structure

```
src/
├── lib.rs / main.rs      # Library root + binary entry point
├── client.rs             # AlpacaClient — all REST methods
├── types.rs              # Shared domain types (serde)
├── app.rs / update.rs    # TEA Model state + event → state reducer
├── input/                # Per-panel keyboard handlers
├── handlers/             # crossterm event stream + REST polling tasks
└── ui/                   # render(frame, app) + per-panel renderers + theme
```

> Architecture deep-dive: [docs/architecture.md](docs/architecture.md)

---

## Documentation

| Document | Description |
|---|---|
| [docs/library.md](docs/library.md) | Full library API reference and usage examples |
| [docs/architecture.md](docs/architecture.md) | System design, library/app boundary, data flow, crate choices |
| [docs/credentials-setup.md](docs/credentials-setup.md) | Obtaining and configuring Alpaca API keys |
| [docs/ui-mockups.md](docs/ui-mockups.md) | ASCII mockups and full keyboard/mouse interaction spec |
| [docs/api-research.md](docs/api-research.md) | REST endpoint shapes and live test results |
| [docs/testing.md](docs/testing.md) | Testing strategy, mock patterns, test case inventory |
| [docs/licensing.md](docs/licensing.md) | License overview and contribution terms |

---

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community standards.

---

## Licensing

Licensed under the [MIT License](LICENSE-MIT).

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you shall be licensed under the MIT License, without any additional terms or conditions.

