# Library Usage

Full API reference and usage examples for the `alpaca-trader-rs` crate.

Add to your `Cargo.toml`:

```toml
[dependencies]
alpaca-trader-rs = "0.6"
tokio = { version = "1", features = ["full"] }
```

---

## Public API

| Module | Exposed items |
|---|---|
| `config` | `AlpacaConfig`, `AlpacaEnv` |
| `client` | `AlpacaClient` — `get_account()`, `get_positions()`, `get_orders()`, `submit_order()`, `cancel_order()`, `get_clock()`, `list_watchlists()`, `get_watchlist()`, `add_to_watchlist()`, `remove_from_watchlist()` |
| `types` | `AccountInfo`, `Position`, `Order`, `OrderRequest`, `OrderSide`, `OrderType`, `TimeInForce`, `Quote`, `MarketClock`, `Watchlist`, `WatchlistSummary`, `Asset` |
| `events` | `Event` — unified event enum consumed by the TUI app |
| `stream` | `MarketStream`, `AccountStream` — WebSocket live data |

---

## Examples

### Fetch account info

```rust
use alpaca_trader_rs::{client::AlpacaClient, config::AlpacaConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let client = AlpacaClient::new(AlpacaConfig::from_env()?);

    let account = client.get_account().await?;
    println!("Equity:        ${}", account.equity);
    println!("Buying power:  ${}", account.buying_power);
    println!("Day trades:    {} / 3", account.daytrade_count);
    println!("PDT:           {}", account.pattern_day_trader);
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

### Cancel an order

```rust
client.cancel_order(&order.id).await?;
```

### Fetch positions

```rust
let positions = client.get_positions().await?;
for p in &positions {
    println!("{}: {} shares @ ${} — P&L: ${}", p.symbol, p.qty, p.avg_entry_price, p.unrealized_pl);
}
```

### Fetch open orders

```rust
let orders = client.get_orders("open").await?;
for o in &orders {
    println!("{} {} {} @ {} — status: {}", o.side, o.qty, o.symbol, o.limit_price.as_deref().unwrap_or("mkt"), o.status);
}
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

### Check market clock

```rust
let clock = client.get_clock().await?;
println!("Market is open: {} | Next open: {}", clock.is_open, clock.next_open);
```

---

## Error Handling

All client methods return `anyhow::Result<T>`. HTTP errors (4xx/5xx) are surfaced as
`anyhow::Error` with the response body included in the message. Build your own error
handling on top with `anyhow::Context` or map to your crate's error type as needed.

```rust
use anyhow::Context;

let account = client
    .get_account()
    .await
    .context("failed to fetch account")?;
```

---

## Configuration

`AlpacaConfig::from_env()` reads credentials from environment variables using the
four-tier priority chain described in [credentials-setup.md](credentials-setup.md).
To construct a config manually:

```rust
use alpaca_trader_rs::config::{AlpacaConfig, AlpacaEnv};

let config = AlpacaConfig {
    base_url: "https://paper-api.alpaca.markets/v2".into(),
    api_key: "your-key".into(),
    api_secret: "your-secret".into(),
    env: AlpacaEnv::Paper,
};
let client = AlpacaClient::new(config);
```
