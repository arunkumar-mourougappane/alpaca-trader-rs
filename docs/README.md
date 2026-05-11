# alpaca-trader-rs

An Alpaca Markets trading toolkit for Rust — ships as both an **integratable library** and a **standalone TUI trading app**.

- **Library**: typed async client for Alpaca REST and WebSocket APIs, shared domain types, and streaming primitives — embed it in your own Rust application.
- **App** (`alpaca-trader` binary): a full terminal UI dashboard built on the library, with live positions, orders, watchlist, market data streaming, and order entry.

---

## Features

### Library
- Typed async `AlpacaClient` covering all major REST endpoints
- `MarketStream` for real-time quotes and bars via WebSocket
- `AccountStream` for order fills and account updates
- `AlpacaConfig` with paper/live environment resolution from `.env`
- Shared domain types: `AccountInfo`, `Position`, `Order`, `Quote`, `Bar`, `Watchlist`, `Asset`, and more

### TUI App
- Real-time account equity, buying power, and P&L display
- Live positions table with unrealized gain/loss
- Orders panel with Open / Filled / Cancelled sub-tabs
- Watchlist panel with live price streaming
- Market data via WebSocket (IEX free tier or SIP)
- Instant order fill notifications from the account event stream
- Market clock status (pre-market / open / after-hours / closed)
- Paper trading mode clearly indicated in the UI header
- Full keyboard navigation and mouse support

---

## Quick Start

### Prerequisites

- Rust 1.85+ (`rustup update stable`)
- An Alpaca Markets account ([alpaca.markets](https://alpaca.markets)) — paper trading is free and works immediately

### 1. Clone and build

```bash
git clone <repo-url>
cd alpaca-trader-rs
cargo build --release
```

### 2. Set credentials

```bash
cp .env.example .env
# Edit .env with your paper trading API keys
```

See [credentials-setup.md](credentials-setup.md) for step-by-step instructions on obtaining keys.

### 3. Run against paper trading

```bash
set -a && source .env && set +a
cargo run --release --bin alpaca-trader
```

The TUI header will show **[PAPER]** to confirm you are in simulation mode.

---

## Documentation

| Document | Description |
|---|---|
| [credentials-setup.md](credentials-setup.md) | How to get and configure Alpaca API keys for paper and live trading |
| [architecture.md](architecture.md) | System design, data flow, crate choices, and module layout |
| [api-research.md](api-research.md) | Live API test results: watchlist endpoints, response shapes, auth notes |
| [ui-mockups.md](ui-mockups.md) | ASCII mockups for all panels and modals, keyboard/mouse interaction spec, ratatui widget mapping |
| [testing.md](testing.md) | Testing strategy: mock patterns, dev-dependency rationale, full test case inventory per module |
| [github-actions.md](github-actions.md) | GitHub Actions reference: CI, security, coverage, releases, matrix builds, caching |
| [phase2-research.md](phase2-research.md) | Phase 2 implementation plan: WebSocket streaming, mutation operations, command channel pattern |
| [phase2-logging.md](phase2-logging.md) | Phase 2 logging design: tracing + file + syslog, no stdout, platform log paths |
| [licensing.md](licensing.md) | License types, fee structure, and how to request a Collaboration Agreement |

---

## Key Bindings

| Key | Action |
|-----|--------|
| `1` / `2` / `3` / `4` | Switch to Account / Watchlist / Positions / Orders |
| `Tab` / `Shift-Tab` | Cycle panels forward / backward |
| `j` / `k` or `↑` / `↓` | Navigate rows |
| `g` / `G` | Jump to first / last row |
| `Enter` | Open symbol detail modal |
| `o` | New order (pre-fills selected symbol) |
| `c` | Cancel selected order |
| `a` | Add symbol to watchlist |
| `d` | Remove symbol from watchlist |
| `/` | Search / filter |
| `r` | Force refresh |
| `?` | Show help overlay |
| `Esc` | Close modal |
| `q` / `Ctrl-C` | Quit |

See [ui-mockups.md](ui-mockups.md) for the full per-panel and per-modal keyboard and mouse interaction spec.

---

## Environment Variables

The `.env` file holds credentials for both environments under separate prefixes:

| Variable | Description |
|---|---|
| `LIVE_ALPACA_ENDPOINT` | `https://api.alpaca.markets` |
| `LIVE_ALPACA_KEY` | Live trading API key ID |
| `LIVE_ALPACA_SECRET` | Live trading API secret key |
| `PAPER_ALPACA_ENDPOINT` | `https://paper-api.alpaca.markets/v2` |
| `PAPER_ALPACA_KEY` | Paper trading API key ID |
| `PAPER_ALPACA_SECRET` | Paper trading API secret key |

Environment variables are optional — on first launch the app will prompt interactively and offer to save credentials to the OS keychain.

See [credentials-setup.md](credentials-setup.md) for setup details.

---

## Paper vs Live Trading

Run with `--paper` to trade against Alpaca's simulation environment — no real money involved. Omit the flag (the default) to use the live account. The TUI header shows **[PAPER]** in cyan or **[LIVE]** in red so the active environment is always visible.

> Always develop and test with `--paper`. Paper trading resets positions daily at market open.

---

## Crate Dependencies

**Library**

| Crate | Purpose |
|---|---|
| `tokio` | Async runtime |
| `apca` | Typed Alpaca REST/WS client |
| `reqwest` | HTTP client for supplemental REST calls |
| `tokio-tungstenite` | WebSocket market data and account streaming |
| `serde` / `serde_json` | JSON serialization |
| `dotenvy` | `.env` loading |

**App only**

| Crate | Purpose |
|---|---|
| `ratatui` | TUI rendering framework |
| `ratatui-textarea` | Text input widget (Symbol, Qty, Price fields) |
| `crossterm` | Cross-platform terminal backend |
