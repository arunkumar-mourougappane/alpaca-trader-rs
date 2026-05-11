# Credentials Setup

This guide covers how to obtain and configure Alpaca API credentials for both paper trading (safe, simulated) and live trading environments.

---

## 1. Create an Alpaca Account

1. Go to [https://alpaca.markets](https://alpaca.markets) and sign up.
2. Complete identity verification (required for live trading; paper trading works immediately).
3. After logging in, navigate to **Paper Trading** from the dashboard menu to start in simulation mode.

---

## 2. Generate API Keys

### Paper Trading Keys

1. Log in to [https://app.alpaca.markets](https://app.alpaca.markets).
2. In the top-right, switch the environment toggle to **Paper**.
3. Go to **Overview** → **Your API Keys** → **Regenerate** (or **View**).
4. Copy your **API Key ID** and **Secret Key** — the secret is shown only once.

### Live Trading Keys

1. Switch the toggle to **Live**.
2. Repeat the same steps above.

> Keep live and paper keys separate. Never commit either to version control.

---

## 3. Configure Credentials

The application reads credentials from a `.env` file at the project root. Both environments are stored in the same file using a `LIVE_` / `PAPER_` prefix so you can switch at runtime without editing the file.

### `.env` File Structure

```env
LIVE_ALPACA_ENDPOINT=https://api.alpaca.markets
LIVE_ALPACA_KEY=your-live-key-id
LIVE_ALPACA_SECRET=your-live-secret-key

PAPER_ALPACA_ENDPOINT=https://paper-api.alpaca.markets/v2
PAPER_ALPACA_KEY=your-paper-key-id
PAPER_ALPACA_SECRET=your-paper-secret-key
```

> `PAPER_ALPACA_ENDPOINT` already includes `/v2`. `LIVE_ALPACA_ENDPOINT` does not — append `/v2` in code when constructing request URLs.

Load before running:

```bash
set -a && source .env && set +a
cargo run
```

Or use the `dotenvy` crate (add to `Cargo.toml`) to load it automatically at startup.

### Selecting an Environment at Runtime

The environment is selected via the `--paper` CLI flag. The binary defaults to **live** when the flag is absent. Only the credentials for the active environment are read — the opposing set is ignored entirely.

```bash
alpaca-trader           # live trading (real money — default)
alpaca-trader --paper   # paper trading (simulated funds)
```

Using `run.sh`:

```bash
./run.sh           # live (default)
./run.sh --paper   # paper
```

In Rust, the environment is resolved in `src/main.rs` and passed directly to `AlpacaConfig::from_env`:

```rust
let env = if args.paper { AlpacaEnv::Paper } else { AlpacaEnv::Live };
AlpacaConfig::from_env(env)?;
```

---

## 4. API Base URLs

| Environment   | REST Base URL                           | WebSocket Account Stream                        |
|---------------|-----------------------------------------|-------------------------------------------------|
| Paper Trading | `https://paper-api.alpaca.markets`      | `wss://paper-api.alpaca.markets/stream`         |
| Live Trading  | `https://api.alpaca.markets`            | `wss://api.alpaca.markets/stream`               |

Market data streaming uses the same URL regardless of paper/live:

| Data Feed | WebSocket URL                                          | Notes                        |
|-----------|--------------------------------------------------------|------------------------------|
| IEX (free)| `wss://stream.data.alpaca.markets/v2/iex`              | Free tier, IEX exchange only |
| SIP       | `wss://stream.data.alpaca.markets/v2/sip`              | Requires paid data plan      |
| Options   | `wss://stream.data.alpaca.markets/v2/opt`              | Requires options subscription|

---

## 5. Authentication Headers

All REST API calls must include these headers:

```
APCA-API-KEY-ID: <your-key-id>
APCA-API-SECRET-KEY: <your-secret-key>
```

WebSocket authentication is done by sending a JSON message immediately after connecting:

```json
{
  "action": "auth",
  "key": "<your-key-id>",
  "secret": "<your-secret-key>"
}
```

The server must receive this within **10 seconds** of the connection opening.

---

## 6. Verify the Setup

Run a quick connectivity check after sourcing `.env`:

```bash
source .env

# Paper
curl -s \
  -H "APCA-API-KEY-ID: $PAPER_ALPACA_KEY" \
  -H "APCA-API-SECRET-KEY: $PAPER_ALPACA_SECRET" \
  "$PAPER_ALPACA_ENDPOINT/account" | jq .status

# Live
curl -s \
  -H "APCA-API-KEY-ID: $LIVE_ALPACA_KEY" \
  -H "APCA-API-SECRET-KEY: $LIVE_ALPACA_SECRET" \
  "$LIVE_ALPACA_ENDPOINT/v2/account" | jq .status
```

Expected response: `"ACTIVE"`

---

## 7. Security Checklist

- [ ] `.env` file is listed in `.gitignore`
- [ ] API keys are **never** hard-coded in source files
- [ ] Paper trading keys are used during all development and testing
- [ ] Live keys are stored in a secrets manager (e.g., macOS Keychain, 1Password, AWS Secrets Manager) rather than a plain `.env` file
- [ ] Live keys have IP allow-listing enabled in the Alpaca dashboard if your deployment has a static IP
