# Architecture

## Overview

`alpaca-trader-rs` is two things in one crate:

- **Library** (`src/lib.rs`): a public async API for Alpaca Markets — typed REST client, WebSocket streams, shared domain types, and app infrastructure (prefs, commands, logging). Intended to be embedded in other Rust applications.
- **Binary** (`src/main.rs`): a full terminal UI (TUI) trading dashboard built on top of the library using the Elm Architecture (TEA).

The library exports `client`, `config`, `types`, `events`, `stream`, `commands`, `prefs`, `logging`, and `clipboard`. App-only code (`app`, `update`, `input`, `ui`) lives in the binary and is not re-exported from `lib.rs`.

---

## Technology Stack

| Layer | Crate | Version | Scope | Purpose |
|---|---|---|---|---|
| Async runtime | `tokio` | 1.x | Library + App | Async tasks, timers, channels, `select!` |
| HTTP client | `reqwest` | 0.13 | Library | All Alpaca REST calls |
| WebSocket | `tokio-tungstenite` | 0.29 | Library | Market data and account streams |
| Serialization | `serde` + `serde_json` | 1 | Library | JSON encode/decode |
| Config | `dotenvy` | 0.15 | Library + App | `.env` loading |
| Date/time | `chrono` | 0.4 | Library + App | Timestamps, date formatting |
| Async utils | `tokio-util` | 0.7 | App | `CancellationToken` for graceful shutdown |
| TUI rendering | `ratatui` | 0.30 | App only | Immediate-mode terminal widgets |
| Terminal backend | `crossterm` | 0.29 | App only | Cross-platform raw-mode I/O, mouse support |
| CLI args | `clap` | 4 | App only | `--paper`, `--dry-run` flags |
| Preferences | `toml` | 1.1 | App only | Serialize/deserialize `config.toml` |
| Credentials | `keyring` | 3 | App only | Native OS keychain (macOS/Windows/Linux) |
| Credentials | `rpassword` | 7 | App only | Interactive secure password prompt |
| Clipboard | `arboard` | 3 | App only | Cross-platform clipboard write (`c` key) |
| Logging | `tracing` + `tracing-subscriber` + `tracing-appender` | 0.1 / 0.3 / 0.2 | App only | Structured file + syslog logging |
| Paths | `dirs` | 6 | App only | Platform config/data directory resolution |

### Why `reqwest` directly (no wrapper library)?

`reqwest` gives full control over request shape, headers, and error handling. Using raw `reqwest` lets the codebase match Alpaca's API surface exactly without an intermediate abstraction layer that could lag behind API changes. All endpoints are unit-tested via `wiremock`.

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
│   ├── commands.rs         # Command enum: mutation requests from input to REST handler
│   ├── prefs.rs            # AppPrefs: user preferences persisted to config.toml
│   ├── credentials.rs      # 4-tier credential resolution (env → keychain → prompt)
│   ├── clipboard.rs        # Cross-platform clipboard write helper
│   ├── logging.rs          # File + syslog tracing subscriber setup
│   ├── input/
│   │   ├── mod.rs          # Shared nav helper (j/k/g/G) and key() / ctrl() factories
│   │   ├── modal.rs        # Key handler for all modal states
│   │   ├── mouse.rs        # Mouse event → hit-area dispatch
│   │   ├── orders.rs       # handle_orders_key: sort, filter, sub-tabs, cancel
│   │   ├── positions.rs    # handle_positions_key: sort, order entry, detail modal
│   │   ├── search.rs       # Inline watchlist search and global search modal
│   │   ├── validation.rs   # Pre-submit order validation (qty, price, buying power)
│   │   └── watchlist.rs    # handle_watchlist_key: add, remove, navigate
│   └── ui/
│       ├── mod.rs          # render(frame, app) — the TEA View function
│       ├── account.rs      # Account summary + equity sparkline + daytrade/PDT info
│       ├── charts.rs       # Shared braille line-chart renderer and crosshair logic
│       ├── dashboard.rs    # Header, tab bar, status bar layout
│       ├── formatting.rs   # Currency, percentage, and volume formatting helpers
│       ├── modals.rs       # All modals: OrderEntry, SymbolDetail, PositionDetail, Help, etc.
│       ├── orders.rs       # Orders table + sub-tabs + sort/filter indicators
│       ├── positions.rs    # Positions table + P&L footer + sort indicators
│       ├── test_helpers.rs # Shared render-test utilities (terminal fixture, etc.)
│       ├── theme.rs        # Color palette and styles (default / dark / high-contrast)
│       └── watchlist.rs    # Watchlist table + inline search bar
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
pub mod client;    // AlpacaClient
pub mod clipboard; // cross-platform clipboard write
pub mod commands;  // Command enum (mutation requests)
pub mod config;    // AlpacaConfig
pub mod events;    // Event enum
pub mod logging;   // tracing subscriber setup
pub mod prefs;     // AppPrefs (config.toml persistence)
pub mod stream;    // MarketStream, AccountStream
pub mod types;     // All domain types
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
    pub fn from_env(env: AlpacaEnv) -> Result<Self>;  // reads LIVE_* or PAPER_* vars for the given env
}
```

### `AlpacaClient`

Async REST client. All methods return `Result<T>` (`anyhow::Result`):

```rust
impl AlpacaClient {
    pub fn new(config: AlpacaConfig) -> Self;
    pub fn is_paper(&self) -> bool;

    // Account & portfolio
    pub async fn get_account(&self) -> Result<AccountInfo>;
    pub async fn get_portfolio_history(&self, period: &str, timeframe: &str) -> Result<Vec<f64>>;

    // Positions & orders
    pub async fn get_positions(&self) -> Result<Vec<Position>>;
    pub async fn get_orders(&self, status: &str) -> Result<Vec<Order>>;
    pub async fn submit_order(&self, req: &OrderRequest) -> Result<Order>;
    pub async fn cancel_order(&self, order_id: &str) -> Result<()>;

    // Market data
    pub async fn get_clock(&self) -> Result<MarketClock>;
    pub async fn get_snapshots(&self, symbols: &[String]) -> Result<HashMap<String, Snapshot>>;
    pub async fn get_intraday_bars(&self, symbol: &str) -> Result<Vec<MinuteBar>>;

    // Watchlists
    pub async fn list_watchlists(&self) -> Result<Vec<WatchlistSummary>>;
    pub async fn get_watchlist(&self, id: &str) -> Result<Watchlist>;
    pub async fn add_to_watchlist(&self, id: &str, symbol: &str) -> Result<Watchlist>;
    pub async fn remove_from_watchlist(&self, id: &str, symbol: &str) -> Result<Watchlist>;
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
  ┌─────────────────────────────────────────────────────────────────────┐
  │                          tokio runtime                              │
  │                                                                     │
  │  ┌──────────────┐  Event::Input   ┌─────────────────────────────┐  │
  │  │  input task  │ ─────────────►  │                             │  │
  │  │  (crossterm  │                 │         main loop           │  │
  │  │  EventStream)│                 │                             │  │
  │  └──────────────┘                 │  tokio::select! on          │  │
  │                                   │  mpsc::Receiver<Event>      │  │
  │  ┌──────────────┐  Event::*Data   │                             │  │
  │  │  REST poller │ ─────────────►  │  1. update(app, evt)        │  │
  │  │  AlpacaClient│                 │  2. terminal.draw(render)   │  │
  │  └──────▲───────┘                 │                             │  │
  │         │ Command                 └─────────────────────────────┘  │
  │  ┌──────┴───────┐                           │ Command              │
  │  │  command_tx  │ ◄─────────────────────────┘                      │
  │  │  (mpsc chan) │   input handlers send mutation requests           │
  │  └──────────────┘   (submit_order, cancel, watchlist mutations)     │
  │                                                                     │
  │  ┌──────────────┐  Event::MarketQuote                              │
  │  │ MarketStream │ ─────────────►  (tx clone → same receiver)       │
  │  └──────────────┘                                                   │
  │                                                                     │
  │  ┌──────────────┐  Event::TradeUpdate                              │
  │  │AccountStream │ ─────────────►  (tx clone → same receiver)       │
  │  └──────────────┘                                                   │
  └─────────────────────────────────────────────────────────────────────┘
```

All event producers clone a `tokio::sync::mpsc::Sender<Event>`. The main loop holds the single `Receiver<Event>`. Mutation commands (order submit, cancel, watchlist add/remove) flow through a separate `mpsc::Sender<Command>` → REST handler channel, keeping the event flow unidirectional.

### State Model (`app.rs`)

```rust
pub struct App {
    // Config & preferences
    pub config: AlpacaConfig,
    pub prefs: AppPrefs,
    pub current_theme: Theme,

    // Data state
    pub account: Option<AccountInfo>,
    pub positions: Vec<Position>,
    pub orders: Vec<Order>,
    pub quotes: HashMap<String, Quote>,
    pub watchlist: Option<Watchlist>,
    pub snapshots: HashMap<String, Snapshot>,
    pub clock: Option<MarketClock>,
    pub equity_history: Vec<u64>,
    pub equity_range: EquityRange,
    pub intraday_bars: HashMap<String, Vec<u64>>,

    // UI selection state
    pub active_tab: Tab,
    pub watchlist_state: TableState,
    pub positions_state: TableState,
    pub orders_state: TableState,
    pub orders_subtab: OrdersSubTab,
    pub modal: Option<Modal>,

    // Sort + filter state
    pub positions_sort: SortState<PositionSortCol>,
    pub orders_sort: SortState<OrderSortCol>,
    pub orders_symbol_filter: String,
    pub orders_filter_active: bool,

    // Search
    pub search_query: String,
    pub searching: bool,

    // Crosshair cursors
    pub equity_chart_cursor: Option<usize>,
    pub symbol_detail_crosshair: Option<usize>,

    // Status / control
    pub status_queue: VecDeque<StatusMessage>,
    pub should_quit: bool,
    pub last_updated: Option<DateTime<Local>>,
    pub spinner_tick: u8,

    // Channels
    pub command_tx: mpsc::Sender<Command>,
    pub symbol_tx: watch::Sender<Vec<String>>,
    pub refresh_notify: Arc<Notify>,
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
    WatchlistUnavailable,
    SnapshotsUpdated(HashMap<String, Snapshot>),
    PortfolioHistoryLoaded(Vec<f64>),
    IntradayBarsReceived { symbol: String, bars: Vec<u64> },
    FetchStarted,
    FetchComplete,

    // WebSocket streaming
    MarketQuote(Quote),
    TradeUpdate { order: Order, event_type: String },
    StreamConnected(StreamKind),
    StreamDisconnected(StreamKind),

    // Control
    Tick,   // 250 ms UI refresh tick
    Quit,
    StatusMsg(String),
}
```

### Update Function (`update.rs`)

```rust
pub fn update(app: &mut App, event: Event) {
    match event {
        Event::Input(key)  => handle_key(app, key),
        Event::Mouse(m)    => handle_mouse(app, m),
        Event::Resize(..)  => { app.needs_redraw = true; }

        Event::AccountUpdated(a) => {
            app.account = Some(a);
            app.push_equity();          // append to sparkline history
        }
        Event::PositionsUpdated(p)  => { app.positions = p; /* auto-select */ }
        Event::OrdersUpdated(o)     => { app.orders = o;    /* auto-select */ }
        Event::ClockUpdated(c)      => { app.clock = Some(c); }
        Event::WatchlistUpdated(w)  => {
            let _ = app.symbol_tx.send(/* symbol list for stream resubscription */);
            app.watchlist = Some(w);
        }
        Event::WatchlistUnavailable => { app.watchlist_unavailable = true; }
        Event::MarketQuote(q) => {
            app.quotes.insert(q.symbol.clone(), q);
            app.push_equity_from_quotes();  // stream equity samples between polls
        }
        Event::TradeUpdate { order: o, event_type } => {
            // Flash fill notification then upsert order into the list
            if let Some(msg) = fill_notification_text(&o, &event_type) {
                app.push_fill_notification(msg);
            }
            if let Some(existing) = app.orders.iter_mut().find(|x| x.id == o.id) {
                *existing = o;
            } else {
                app.orders.insert(0, o);
            }
        }
        Event::StatusMsg(msg)          => { app.push_status(StatusMessage::persistent(msg)); }
        Event::StreamConnected(kind)   => { /* set market_stream_ok / account_stream_ok */ }
        Event::StreamDisconnected(kind)=> { /* clear stream ok flag */ }
        Event::PortfolioHistoryLoaded(data) => {
            app.equity_history = data.into_iter().map(|v| (v * 100.0) as u64).collect();
        }
        Event::SnapshotsUpdated(s)     => { app.snapshots = s; }
        Event::IntradayBarsReceived { symbol, bars } => {
            app.intraday_bars.insert(symbol, bars);
        }
        Event::FetchStarted  => app.request_started(),
        Event::FetchComplete => app.request_finished(),
        Event::Tick => {
            app.tick_spinner();
            // expire timed status messages, schedule intraday refreshes, etc.
        }
        Event::Quit => { app.should_quit = true; }
    }
}
```

No I/O, no async — pure and unit-testable.

### View Function (`ui/mod.rs`)

```rust
pub fn render(frame: &mut Frame, app: &mut App) {
    // Reset hit areas so stale click rects are never used.
    app.hit_areas = HitAreas::default();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header + market clock
            Constraint::Length(1),  // tab bar
            Constraint::Min(0),     // active panel
            Constraint::Length(1),  // status bar
        ])
        .split(frame.area());

    app.hit_areas.tab_bar = chunks[1];

    dashboard::render_header(frame, chunks[0], app);
    dashboard::render_tabs(frame, chunks[1], app);

    match app.active_tab {
        Tab::Account   => account::render(frame, chunks[2], app),
        Tab::Watchlist => watchlist::render(frame, chunks[2], app),
        Tab::Positions => positions::render(frame, chunks[2], app),
        Tab::Orders    => orders::render(frame, chunks[2], app),
    }

    dashboard::render_status(frame, chunks[3], app);

    // Modals rendered last (always on top)
    if let Some(modal) = app.modal.clone() {
        modals::render(frame, frame.area(), &modal, app);
    }
}
```

`render` takes `&mut App` (not `&App`) because it records click hit-areas into `app.hit_areas` on each frame. Pure data read otherwise — no I/O, no side effects beyond hit-area tracking.

---

## Paper vs Live Trading

Controlled by the `--paper` CLI flag. The binary defaults to **live**. All downstream code is identical between environments; only `AlpacaConfig` and the TUI badge differ.

| Config point | Paper (`--paper`) | Live (default) |
|---|---|---|
| CLI flag | `--paper` | *(omit)* |
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
| GET | `/v2/orders?status=all` | `get_orders(status)` |
| POST | `/v2/orders` | `submit_order(req)` |
| DELETE | `/v2/orders/{id}` | `cancel_order(id)` |
| GET | `/v2/clock` | `get_clock()` |
| GET | `/v2/watchlists` | `list_watchlists()` |
| GET | `/v2/watchlists/{id}` | `get_watchlist(id)` |
| POST | `/v2/watchlists/{id}` | `add_to_watchlist(id, symbol)` |
| DELETE | `/v2/watchlists/{id}/{symbol}` | `remove_from_watchlist(id, symbol)` |
| GET | `/v2/account/portfolio/history` | `get_portfolio_history(period, timeframe)` |
| GET | `/v2/stocks/snapshots` | `get_snapshots(symbols)` |
| GET | `/v2/stocks/{symbol}/bars` | `get_intraday_bars(symbol)` |

---

## Graceful Shutdown

A `tokio_util::sync::CancellationToken` is created at startup and cloned into every background task. On `Event::Quit` (or `Ctrl-C`), the main loop calls `token.cancel()`. All tasks check `token.cancelled()` in their `select!` arms and exit cleanly.

Terminal raw mode is restored via a `Drop` guard wrapping the `ratatui::Terminal` in `main.rs`.

---

## Testing Against Paper Trading

1. Run `cargo run --bin alpaca-trader -- --paper`.
2. The header shows **[PAPER]** — no real money at risk.
3. Place orders through the UI; fills appear in the Orders panel via the account WebSocket stream within seconds.
4. Paper positions reset daily at market open.
