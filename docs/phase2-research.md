# Phase 2 Research

Implementation plan and protocol reference for the two work streams that make up Phase 2:
1. **Mutation operations** — wire order submission, order cancellation, and watchlist add/remove to the REST API
2. **WebSocket streaming** — real-time market quotes and account/trade event notifications

---

## Current Stubs (what is blocked)

All four mutation paths and both WebSocket streams are currently UI-only stubs. They display a status message but do not call the API.

| Location | Line | What is stubbed |
|---|---|---|
| `src/update.rs` | ~289 | Order submission — sets `"Order submission coming in Phase 2"` |
| `src/update.rs` | ~314 | `ConfirmAction::CancelOrder` — sets `"Cancelling order…"` then returns |
| `src/update.rs` | ~319 | `ConfirmAction::RemoveFromWatchlist` — sets `"Removing…"` then returns |
| `src/update.rs` | ~347 | `Modal::AddSymbol` Enter — sets `"Adding…"` then returns |
| `src/main.rs` | ~67 | No `MarketStream` task spawned |
| `src/main.rs` | ~67 | No `AccountStream` task spawned |
| `src/events.rs` | 15-16 | `MarketQuote(Quote)` and `TradeUpdate(Order)` defined but never emitted |

The `update()` handlers for `Event::MarketQuote` and `Event::TradeUpdate` already exist and are correct — they just have no producer.

---

## Part 1 — Mutation Operations

### Problem: `update()` is synchronous, client is async

`update(&mut App, Event)` is a synchronous function. It cannot `await` a `client.submit_order()` call directly. The solution is a **command channel**: `update()` sends a typed command onto a `Sender<Command>`, and a background task awaits it, calls the client, and triggers an immediate refresh via `refresh_notify`.

### New types needed

```rust
// src/commands.rs  (new file)
pub enum Command {
    SubmitOrder {
        symbol: String,
        side: String,
        order_type: String,
        qty: Option<String>,
        price: Option<String>,
        time_in_force: String,
    },
    CancelOrder(String),                              // order id
    AddToWatchlist { watchlist_id: String, symbol: String },
    RemoveFromWatchlist { watchlist_id: String, symbol: String },
}
```

### Changes to `App`

```rust
pub struct App {
    // existing fields …
    pub command_tx: tokio::sync::mpsc::Sender<Command>,  // add this
}
```

`App::new()` receives `command_tx` alongside `refresh_notify`. All four stub handlers in `update.rs` send on `command_tx` instead of setting a status string.

### New background task

```rust
// src/handlers/commands.rs  (new file)
pub async fn run(
    mut rx: Receiver<Command>,
    tx: Sender<Event>,
    client: Arc<AlpacaClient>,
    refresh_notify: Arc<Notify>,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            Some(cmd) = rx.recv() => {
                match cmd {
                    Command::SubmitOrder { .. } => {
                        let req = build_order_request(cmd);
                        match client.submit_order(&req).await {
                            Ok(_)  => { let _ = tx.send(Event::StatusMsg("Order submitted".into())).await; }
                            Err(e) => { let _ = tx.send(Event::StatusMsg(format!("Order error: {}", e))).await; }
                        }
                        refresh_notify.notify_one();
                    }
                    Command::CancelOrder(id) => {
                        match client.cancel_order(&id).await {
                            Ok(_)  => { let _ = tx.send(Event::StatusMsg("Order cancelled".into())).await; }
                            Err(e) => { let _ = tx.send(Event::StatusMsg(format!("Cancel error: {}", e))).await; }
                        }
                        refresh_notify.notify_one();
                    }
                    Command::AddToWatchlist { watchlist_id, symbol } => {
                        match client.add_to_watchlist(&watchlist_id, &symbol).await {
                            Ok(wl) => { let _ = tx.send(Event::WatchlistUpdated(wl)).await; }
                            Err(e) => { let _ = tx.send(Event::StatusMsg(format!("Watchlist error: {}", e))).await; }
                        }
                    }
                    Command::RemoveFromWatchlist { watchlist_id, symbol } => {
                        match client.remove_from_watchlist(&watchlist_id, &symbol).await {
                            Ok(wl) => { let _ = tx.send(Event::WatchlistUpdated(wl)).await; }
                            Err(e) => { let _ = tx.send(Event::StatusMsg(format!("Watchlist error: {}", e))).await; }
                        }
                    }
                }
            }
            _ = cancel.cancelled() => break,
        }
    }
}
```

`refresh_notify.notify_one()` after order operations triggers `handlers/rest.rs` to re-poll orders/positions immediately, so the UI reflects the change within one second.

### `update.rs` changes (four stubs replaced)

```rust
// Order Entry — Submit button pressed
if state.focused_field == OrderField::Submit {
    let _ = app.command_tx.try_send(Command::SubmitOrder {
        symbol: state.symbol.clone(),
        side: if state.side_buy { "buy" } else { "sell" }.into(),
        order_type: if state.market_order { "market" } else { "limit" }.into(),
        qty: if state.qty_input.is_empty() { None } else { Some(state.qty_input.clone()) },
        price: if state.market_order || state.price_input.is_empty() {
            None
        } else {
            Some(state.price_input.clone())
        },
        time_in_force: "day".into(),
    });
    app.status_msg = "Submitting order…".into();
    app.modal = None;
    return;
}

// Cancel order confirm
ConfirmAction::CancelOrder(id) => {
    let _ = app.command_tx.try_send(Command::CancelOrder(id.clone()));
    app.status_msg = format!("Cancelling {}…", &id[..id.len().min(8)]);
}

// Remove from watchlist confirm
ConfirmAction::RemoveFromWatchlist { watchlist_id, symbol } => {
    let _ = app.command_tx.try_send(Command::RemoveFromWatchlist {
        watchlist_id: watchlist_id.clone(),
        symbol: symbol.clone(),
    });
    app.status_msg = format!("Removing {}…", symbol);
}

// AddSymbol modal — Enter
KeyCode::Enter if !input.is_empty() => {
    let _ = app.command_tx.try_send(Command::AddToWatchlist {
        watchlist_id: watchlist_id.clone(),
        symbol: input.to_uppercase(),
    });
    app.status_msg = format!("Adding {}…", input);
    None  // close modal
}
```

---

## Part 2 — WebSocket Market Data Stream

### Endpoint

```
wss://stream.data.alpaca.markets/v2/iex     ← free tier (IEX exchange only)
wss://stream.data.alpaca.markets/v2/sip     ← requires paid subscription
```

The data URL is environment-independent (same for paper and live). Use IEX for development.

### Authentication flow

After connecting, send within **10 seconds** or the server disconnects:

```json
{ "action": "auth", "key": "PKTEST…", "secret": "secret…" }
```

Server responds:
```json
[{ "T": "success", "msg": "authenticated" }]
```

### Subscribe to quotes

```json
{
  "action": "subscribe",
  "quotes": ["AAPL", "TSLA", "AMD"],
  "trades": [],
  "bars": []
}
```

Resubscribe whenever the watchlist changes. To update subscriptions send a new subscribe message — it merges with the existing set.

### Quote message shape (`T="q"`)

```json
{
  "T": "q",
  "S": "AAPL",
  "ap": 185.50,
  "as": 100,
  "bp": 185.49,
  "bs": 150,
  "ax": "EDGX",
  "bx": "NASDAQ",
  "t":  "2026-05-10T15:30:45.123456789Z",
  "c":  "R",
  "z":  "C"
}
```

Map to existing `types::Quote`:

| JSON field | `Quote` field | Notes |
|---|---|---|
| `S` | `symbol` | ticker |
| `ap` | `ap` | ask price |
| `bp` | `bp` | bid price |
| `as` | `as_` | ask size (`as` is a Rust keyword) |
| `bs` | `bs` | bid size |

### New file: `src/stream/market.rs`

```rust
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use serde_json::{json, Value};

use crate::config::AlpacaConfig;
use crate::events::Event;
use crate::types::Quote;

pub async fn run(
    tx: Sender<Event>,
    cancel: CancellationToken,
    config: AlpacaConfig,
    symbols: Vec<String>,   // initial watchlist symbols
) {
    let url = "wss://stream.data.alpaca.markets/v2/iex";

    let (ws, _) = match connect_async(url).await {
        Ok(v) => v,
        Err(e) => {
            let _ = tx.send(Event::StatusMsg(format!("Market stream: {e}"))).await;
            return;
        }
    };
    let (mut write, mut read) = ws.split();

    // Authenticate
    let auth = json!({ "action": "auth", "key": config.key, "secret": config.secret });
    let _ = write.send(Message::Text(auth.to_string().into())).await;

    // Subscribe to quotes for all watchlist symbols
    let sub = json!({ "action": "subscribe", "quotes": symbols });
    let _ = write.send(Message::Text(sub.to_string().into())).await;

    loop {
        tokio::select! {
            Some(Ok(msg)) = read.next() => {
                if let Message::Text(text) = msg {
                    if let Ok(msgs) = serde_json::from_str::<Vec<Value>>(&text) {
                        for m in msgs {
                            if m["T"] == "q" {
                                let quote = Quote {
                                    symbol: m["S"].as_str().unwrap_or("").to_string(),
                                    ap: m["ap"].as_f64(),
                                    bp: m["bp"].as_f64(),
                                    as_: m["as"].as_u64(),
                                    bs: m["bs"].as_u64(),
                                };
                                let _ = tx.send(Event::MarketQuote(quote)).await;
                            }
                        }
                    }
                }
            }
            _ = cancel.cancelled() => break,
        }
    }
}
```

**Reconnection**: wrap the connect/auth/subscribe/loop in a `loop { ... }` with an exponential backoff sleep (starting 1 s, capping at 60 s) so the stream recovers from network drops automatically.

**Resubscription**: when `Event::WatchlistUpdated` arrives, the main loop can send a new symbol list to the stream task via a `watch::Sender<Vec<String>>` channel. The task selects on both the read stream and the symbol watch channel.

---

## Part 3 — Account/Trade Updates Stream

### Endpoint

```
wss://paper-api.alpaca.markets/stream    ← paper trading
wss://api.alpaca.markets/stream          ← live trading
```

This is the `config.base_url` with `https://` replaced by `wss://` and `/v2` stripped.

### Authentication + listen

```json
{ "action": "auth", "key": "PKTEST…", "secret": "secret…" }
```

Then:
```json
{ "action": "listen", "data": { "streams": ["trade_updates"] } }
```

### Trade update message shape

```json
{
  "stream": "trade_updates",
  "data": {
    "event": "fill",
    "order": {
      "id":              "e8a19a22-…",
      "symbol":          "AAPL",
      "side":            "buy",
      "qty":             "10",
      "filled_qty":      "10",
      "filled_avg_price":"185.50",
      "status":          "filled",
      "order_type":      "limit",
      "limit_price":     "185.00",
      "time_in_force":   "day",
      "submitted_at":    "2026-05-10T15:30:45Z",
      "filled_at":       "2026-05-10T15:30:47Z"
    }
  }
}
```

**Event types**: `new`, `partial_fill`, `fill`, `canceled`, `expired`, `replaced`, `rejected`, `pending_new`, `accepted`

The `order` object maps directly to `types::Order`. Deserialize it and emit `Event::TradeUpdate(order)`.

### New file: `src/stream/account.rs`

```rust
pub async fn run(
    tx: Sender<Event>,
    cancel: CancellationToken,
    config: AlpacaConfig,
) {
    // Derive WebSocket URL from REST base_url
    let ws_url = config.base_url
        .replace("https://", "wss://")
        .replace("/v2", "")
        + "/stream";

    let (ws, _) = connect_async(&ws_url).await.unwrap();
    let (mut write, mut read) = ws.split();

    let auth = json!({ "action": "auth", "key": config.key, "secret": config.secret });
    let _ = write.send(Message::Text(auth.to_string().into())).await;

    let listen = json!({ "action": "listen", "data": { "streams": ["trade_updates"] } });
    let _ = write.send(Message::Text(listen.to_string().into())).await;

    loop {
        tokio::select! {
            Some(Ok(msg)) = read.next() => {
                if let Message::Text(text) = msg {
                    if let Ok(v) = serde_json::from_str::<Value>(&text) {
                        if v["stream"] == "trade_updates" {
                            if let Ok(order) = serde_json::from_value::<Order>(
                                v["data"]["order"].clone()
                            ) {
                                let _ = tx.send(Event::TradeUpdate(order)).await;
                            }
                        }
                    }
                }
            }
            _ = cancel.cancelled() => break,
        }
    }
}
```

---

## Files to Create / Modify

| File | Action | Purpose |
|---|---|---|
| `src/commands.rs` | **Create** | `Command` enum for async mutation operations |
| `src/stream/mod.rs` | **Create** | module declarations |
| `src/stream/market.rs` | **Create** | Market data WebSocket task |
| `src/stream/account.rs` | **Create** | Account/trade updates WebSocket task |
| `src/handlers/commands.rs` | **Create** | Background task that awaits commands and calls client |
| `src/handlers/mod.rs` | **Update** | Add `pub mod commands` |
| `src/lib.rs` | **Update** | Add `pub mod commands; pub mod stream` |
| `src/app.rs` | **Update** | Add `command_tx: Sender<Command>` field |
| `src/update.rs` | **Update** | Replace 4 stubs with `command_tx.try_send(...)` |
| `src/main.rs` | **Update** | Create command channel, spawn 2 stream tasks + command handler |

---

## Implementation Order

1. `src/commands.rs` — define `Command` enum (no deps)
2. `src/handlers/commands.rs` — command handler task
3. `src/app.rs` — add `command_tx` field
4. `src/update.rs` — replace 4 stubs with command sends
5. `src/stream/market.rs` — market data WebSocket task
6. `src/stream/account.rs` — account stream WebSocket task
7. `src/lib.rs` + `src/handlers/mod.rs` — wire up module declarations
8. `src/main.rs` — create channel + spawn 3 new tasks

---

## Alpaca API Rate Limits

- Market data WebSocket: **1 concurrent connection** per account on the free (IEX) tier
- Account stream: **1 concurrent connection** per account
- REST orders POST: no hard limit documented, but Alpaca recommends not exceeding a few per second
- `cancel_order` on an already-filled order returns a 422 — handle gracefully

---

## Testing Phase 2

### Mutation operations
- Mock the `AlpacaClient` methods via wiremock in `tests/`
- Verify `Command` is sent (use a `mpsc::channel` in place of the real command task)
- Verify the status message updates correctly on success and error paths

### WebSocket streams
- Use `tokio_tungstenite::accept_async` to spin up a local WebSocket server in tests
- Send canned quote/trade messages and assert `Event::MarketQuote`/`Event::TradeUpdate` arrive on the channel
- Test cancellation: cancel the token and verify the task exits within 1 s
