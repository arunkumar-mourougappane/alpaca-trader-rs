# Architecture

## Overview

`alpaca-trader-rs` is two things in one crate:

- **Library** (`src/lib.rs`): a public async API for Alpaca Markets — typed REST client, WebSocket streams, and shared domain types. Intended to be embedded in other Rust applications.
- **Binary** (`src/main.rs`): a full terminal UI (TUI) trading dashboard built on top of the library using the Elm Architecture (TEA).

The library/app boundary is explicit: `client`, `config`, `types`, `events`, and `stream` modules are public and form the library surface. `app`, `update`, and `ui` are app-only and not re-exported from `lib.rs`.

---

## Technology Stack

| Layer | Crate | Scope | Purpose |
|---|---|---|---|
| Async runtime | `tokio` 1.x | Library + App | Async tasks, timers, channels |
| HTTP client | `apca` 0.30 | Library | Typed Alpaca REST client |
| HTTP fallback | `reqwest` 0.13 | Library | Endpoints not covered by `apca` |
| WebSocket | `tokio-tungstenite` 0.26 | Library | Market data and account streams |
| Serialization | `serde` + `serde_json` | Library | JSON encode/decode |
| Config | `dotenvy` | Library + App | `.env` loading |
| TUI rendering | `ratatui` 0.30 | App only | Immediate-mode terminal widgets |
| Terminal backend | `crossterm` 0.29 | App only | Cross-platform raw-mode I/O |
| Text input | `ratatui-textarea` 0.9 | App only | Symbol, Qty, Price input fields |

### Why `apca` over raw `reqwest`?

`apca` mirrors the Alpaca API with typed request/response structs and built-in async support, eliminating hand-written JSON deserialization for common operations. `reqwest` is retained for endpoints and streaming patterns not covered by `apca`.

### Why `ratatui` + `crossterm`?

`ratatui` is the actively maintained successor to the archived `tui-rs`. Its immediate-mode render model (full frame redraw each tick) keeps rendering logic simple and stateless. `crossterm` is cross-platform; `termion` is lighter but Unix-only.

---

## Directory Layout

```
alpaca-trader-rs/
├── src/
│   │
│   │   ── LIBRARY (public, re-exported from lib.rs) ──────────────────
│   ├── lib.rs              # Public API surface — re-exports library modules
│   ├── config.rs           # AlpacaConfig: env resolution, endpoint selection
│   ├── client.rs           # AlpacaClient: all REST methods
│   ├── types.rs            # AccountInfo, Position, Order, Quote, Bar, etc.
│   ├── events.rs           # Event enum (shared by library streams and the app)
│   └── stream/
│       ├── mod.rs
│       ├── market.rs       # MarketStream: real-time quotes and bars
│       └── account.rs      # AccountStream: order fills and account updates
│
│       ── APP ONLY (not exposed by lib.rs) ────────────────────────────
│   ├── main.rs             # Binary entry point: runtime setup, main loop
│   ├── app.rs              # App state — the TEA Model
│   ├── update.rs           # update(state, event) — the TEA Update function
│   └── ui/
│       ├── mod.rs          # render(frame, app) — the TEA View function
│       ├── dashboard.rs    # Header, tab bar, status bar layout
│       ├── account.rs      # Account summary + sparkline
│       ├── watchlist.rs    # Watchlist table
│       ├── positions.rs    # Positions table
│       ├── orders.rs       # Orders table + sub-tabs
│       ├── modals.rs       # Order entry, symbol detail, help, confirmation
│       └── theme.rs        # Color palette and styles
│
├── docs/                   # Full documentation
├── .env.example
├── Cargo.toml
├── LICENSE.md
└── README.md
```

---

## Library Public API

`src/lib.rs` re-exports:

```rust
pub mod config;   // AlpacaConfig
pub mod client;   // AlpacaClient
pub mod types;    // All domain types
pub mod events;   // Event enum
pub mod stream;   // MarketStream, AccountStream
```

### `AlpacaConfig`

Resolves credentials from environment at startup:

```rust
pub struct AlpacaConfig {
    pub base_url: String,
    pub key: String,
    pub secret: String,
    pub env: AlpacaEnv,   // Paper | Live
}

impl AlpacaConfig {
    pub fn from_env() -> Result<Self, ConfigError>;  // reads ALPACA_ENV + prefixed vars
    pub fn paper(key: &str, secret: &str) -> Self;
    pub fn live(key: &str, secret: &str) -> Self;
}
```

### `AlpacaClient`

Async REST client. All methods return `Result<T, ClientError>`:

```rust
impl AlpacaClient {
    pub fn new(config: AlpacaConfig) -> Self;

    pub async fn get_account(&self) -> Result<AccountInfo>;
    pub async fn get_positions(&self) -> Result<Vec<Position>>;
    pub async fn get_orders(&self, status: OrderStatus) -> Result<Vec<Order>>;
    pub async fn submit_order(&self, req: OrderRequest) -> Result<Order>;
    pub async fn cancel_order(&self, order_id: &str) -> Result<()>;
    pub async fn get_clock(&self) -> Result<MarketClock>;
    pub async fn get_asset(&self, symbol: &str) -> Result<Asset>;

    pub async fn list_watchlists(&self) -> Result<Vec<WatchlistSummary>>;
    pub async fn get_watchlist(&self, id: &str) -> Result<Watchlist>;
    pub async fn get_watchlist_by_name(&self, name: &str) -> Result<Watchlist>;
    pub async fn add_to_watchlist(&self, id: &str, symbol: &str) -> Result<Watchlist>;
    pub async fn remove_from_watchlist(&self, id: &str, symbol: &str) -> Result<Watchlist>;
    pub async fn replace_watchlist(&self, id: &str, name: &str, symbols: &[&str]) -> Result<Watchlist>;
}
```

### Stream Types

```rust
// Market data — quotes and bars
pub struct MarketStream { ... }
impl MarketStream {
    pub async fn connect(config: &AlpacaConfig) -> Result<Self>;
    pub async fn subscribe(&mut self, symbols: &[&str]) -> Result<()>;
    pub async fn next(&mut self) -> Option<Result<Event>>;
}

// Account updates — order fills, status changes
pub struct AccountStream { ... }
impl AccountStream {
    pub async fn connect(config: &AlpacaConfig) -> Result<Self>;
    pub async fn next(&mut self) -> Option<Result<Event>>;
}
```

---

## App Architecture (TEA)

The TUI app follows the Elm Architecture with three pure functions:

```
Event → update(App, Event) → App → render(Frame, App) → Terminal
```

### Data Flow

```
  ┌─────────────────────────────────────────────────────────────────┐
  │                        tokio runtime                            │
  │                                                                 │
  │  ┌──────────────┐   Event::Input   ┌────────────────────────┐  │
  │  │  input task  │ ──────────────►  │                        │  │
  │  │  (crossterm  │                  │      main loop         │  │
  │  │  EventStream)│                  │                        │  │
  │  └──────────────┘                  │  tokio::select! on     │  │
  │                                    │  mpsc::Receiver<Event> │  │
  │  ┌──────────────┐   Event::Data    │                        │  │
  │  │  REST poller │ ──────────────►  │  1. update(app, evt)   │  │
  │  │  AlpacaClient│                  │  2. terminal.draw(ui)  │  │
  │  └──────────────┘                  │                        │  │
  │                                    └────────────────────────┘  │
  │  ┌──────────────┐   Event::Market                              │
  │  │ MarketStream │ ──────────────►  (tx clone → same receiver)  │
  │  └──────────────┘                                              │
  │                                                                 │
  │  ┌──────────────┐   Event::Account                             │
  │  │AccountStream │ ──────────────►  (tx clone → same receiver)  │
  │  └──────────────┘                                              │
  └─────────────────────────────────────────────────────────────────┘
```

All producers clone a `tokio::sync::mpsc::Sender<Event>`. The main loop holds the single `Receiver<Event>`.

### State Model (`app.rs`)

```rust
pub struct App {
    pub account: Option<AccountInfo>,
    pub positions: Vec<Position>,
    pub orders: Vec<Order>,
    pub quotes: HashMap<String, Quote>,
    pub watchlist: Option<Watchlist>,
    pub clock: Option<MarketClock>,
    pub active_tab: Tab,
    pub selected_row: usize,
    pub modal: Option<Modal>,
    pub status: StatusMessage,
    pub should_quit: bool,
}
```

State is owned exclusively by the main loop. Background tasks never mutate it — they emit `Event` values.

### Event Enum (`events.rs`)

```rust
pub enum Event {
    // Terminal input (app only)
    Input(crossterm::event::KeyEvent),
    Mouse(crossterm::event::MouseEvent),
    Resize(u16, u16),

    // REST poll results
    AccountUpdated(AccountInfo),
    PositionsUpdated(Vec<Position>),
    OrdersUpdated(Vec<Order>),
    ClockUpdated(MarketClock),
    WatchlistUpdated(Watchlist),

    // WebSocket streaming
    MarketQuote(Quote),
    MarketBar(Bar),
    TradeUpdate(TradeUpdate),

    // Control
    Tick,   // 250 ms UI refresh tick
    Quit,
}
```

### Update Function (`update.rs`)

```rust
pub fn update(app: &mut App, event: Event) {
    match event {
        Event::Input(key) => handle_input(app, key),
        Event::Mouse(m)   => handle_mouse(app, m),
        Event::AccountUpdated(a)   => app.account = Some(a),
        Event::PositionsUpdated(p) => app.positions = p,
        Event::OrdersUpdated(o)    => app.orders = o,
        Event::WatchlistUpdated(w) => app.watchlist = Some(w),
        Event::MarketQuote(q) => { app.quotes.insert(q.symbol.clone(), q); }
        Event::TradeUpdate(u) => apply_trade_update(app, u),
        Event::Quit => app.should_quit = true,
        _ => {}
    }
}
```

No I/O, no async — pure and unit-testable.

### View Function (`ui/mod.rs`)

```rust
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header + market clock
            Constraint::Min(0),     // active panel
            Constraint::Length(1),  // status bar
        ])
        .split(frame.area());

    render_header(frame, chunks[0], app);
    render_panel(frame, chunks[1], app);
    render_status(frame, chunks[2], app);

    if let Some(modal) = &app.modal {
        render_modal(frame, frame.area(), modal, app);
    }
}
```

Pure read of `&App`, writes only to `Frame` — no side effects.

---

## Paper vs Live Trading

Controlled entirely by `ALPACA_ENV`. The library resolves the active config at startup; all downstream code is identical between environments.

| Config point | Paper | Live |
|---|---|---|
| `ALPACA_ENV` | `paper` | `live` |
| REST base URL | `https://paper-api.alpaca.markets/v2` | `https://api.alpaca.markets/v2` |
| Account WebSocket | `wss://paper-api.alpaca.markets/stream` | `wss://api.alpaca.markets/stream` |
| Market data WebSocket | `wss://stream.data.alpaca.markets/v2/iex` | Same (or `/v2/sip` with subscription) |
| TUI header badge | `[PAPER]` cyan | `[LIVE]` red |

---

## Alpaca REST Endpoints

| Method | Endpoint | Client Method |
|---|---|---|
| GET | `/v2/account` | `get_account()` |
| GET | `/v2/positions` | `get_positions()` |
| GET | `/v2/orders` | `get_orders()` |
| POST | `/v2/orders` | `submit_order()` |
| DELETE | `/v2/orders/{id}` | `cancel_order()` |
| GET | `/v2/clock` | `get_clock()` |
| GET | `/v2/assets/{symbol}` | `get_asset()` |
| GET | `/v2/watchlists` | `list_watchlists()` |
| GET | `/v2/watchlists/{id}` | `get_watchlist()` |
| GET | `/v2/watchlists:by_name` | `get_watchlist_by_name()` |
| POST | `/v2/watchlists/{id}` | `add_to_watchlist()` |
| DELETE | `/v2/watchlists/{id}/{symbol}` | `remove_from_watchlist()` |
| PUT | `/v2/watchlists/{id}` | `replace_watchlist()` |

---

## Graceful Shutdown

A `tokio_util::sync::CancellationToken` is created at startup and cloned into every background task. On `Event::Quit` (or `Ctrl-C`), the main loop calls `token.cancel()`. All tasks check `token.cancelled()` in their `select!` arms and exit cleanly.

Terminal raw mode is restored via a `Drop` guard wrapping the `ratatui::Terminal` in `main.rs`.

---

## Testing Against Paper Trading

1. Set `ALPACA_ENV=paper` in `.env`.
2. Run `cargo run --bin alpaca-trader`.
3. The header shows **[PAPER]** — no real money at risk.
4. Place orders through the UI; fills appear in the Orders panel via the account WebSocket stream within seconds.
5. Paper positions reset daily at market open.
