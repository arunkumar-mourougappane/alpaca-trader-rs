# API Research & Testing Notes

Live test results against the Alpaca Markets API using credentials from `.env`.

---

## Environment

The `.env` file holds separate prefixed variables for each environment:

| Variable                  | Environment | Value                                      |
|---------------------------|-------------|--------------------------------------------|
| `LIVE_ALPACA_ENDPOINT`    | Live        | `https://api.alpaca.markets`               |
| `LIVE_ALPACA_KEY`         | Live        | `AKLIVE0EXAMPLEKEY00000`                   |
| `LIVE_ALPACA_SECRET`      | Live        | `liveExampleSecretKey000000000000000000`   |
| `PAPER_ALPACA_ENDPOINT`   | Paper       | `https://paper-api.alpaca.markets/v2`      |
| `PAPER_ALPACA_KEY`        | Paper       | `PKPAPR0EXAMPLEKEY00000`                   |
| `PAPER_ALPACA_SECRET`     | Paper       | `paperExampleSecretKey00000000000000000`   |

> Note: The `apca` crate expects `APCA_API_KEY_ID` / `APCA_API_SECRET_KEY`. At startup, resolve which environment is active and re-export under those names, or pass them explicitly via `ApiInfo`.

> The watchlist tests below were run against the **live** endpoint (`LIVE_ALPACA_*`).

---

## Watchlist API

Base path: `/v2/watchlists`

### Endpoints Tested

| Method | Path | Body | Purpose |
|--------|------|------|---------|
| GET | `/v2/watchlists` | — | List all watchlists |
| GET | `/v2/watchlists/{id}` | — | Get watchlist with full asset details |
| GET | `/v2/watchlists:by_name?name={name}` | — | Look up watchlist by name |
| POST | `/v2/watchlists/{id}` | `{"symbol":"AAPL"}` | Append a symbol |
| PUT | `/v2/watchlists/{id}` | `{"name":"...","symbols":[...]}` | Replace entire symbol list |
| DELETE | `/v2/watchlists/{id}/{symbol}` | — | Remove a single symbol |

### List Watchlists — `GET /v2/watchlists`

Returns an array of watchlist summary objects (no `assets` field):

```json
[
  {
    "id": "11111111-1111-1111-1111-111111111111",
    "account_id": "22222222-2222-2222-2222-222222222222",
    "created_at": "2026-05-09T22:13:59.441874Z",
    "updated_at": "2026-05-09T22:13:59.441874Z",
    "name": "Primary Watchlist"
  }
]
```

### Get Watchlist by ID — `GET /v2/watchlists/{id}`

Returns the watchlist with a full `assets` array. Each asset entry contains:

```json
{
  "id": "33333333-3333-3333-3333-333333333333",
  "class": "us_equity",
  "exchange": "NASDAQ",
  "symbol": "AAPL",
  "name": "Apple Inc. Common Stock",
  "status": "active",
  "tradable": true,
  "marginable": true,
  "maintenance_margin_requirement": 0,
  "margin_requirement_long": "0",
  "margin_requirement_short": "0",
  "shortable": true,
  "easy_to_borrow": true,
  "fractionable": true,
  "attributes": null
}
```

**Fields relevant to the TUI watchlist panel:**

| Field | Type | Notes |
|-------|------|-------|
| `symbol` | string | Ticker |
| `name` | string | Full company name |
| `exchange` | string | NASDAQ, NYSE, ARCA, etc. |
| `tradable` | bool | Whether orders can be placed |
| `shortable` | bool | Whether short selling is allowed |
| `easy_to_borrow` | bool | Shortable without locate |
| `fractionable` | bool | Supports fractional shares |
| `class` | string | `us_equity`, `crypto`, etc. |

### Get Watchlist by Name — `GET /v2/watchlists:by_name?name={name}`

Same response shape as by-ID. Useful when you know the name but not the UUID.

### Add Symbol — `POST /v2/watchlists/{id}`

```bash
curl -X POST \
  -H "APCA-API-KEY-ID: $LIVE_ALPACA_KEY" \
  -H "APCA-API-SECRET-KEY: $LIVE_ALPACA_SECRET" \
  -H "Content-Type: application/json" \
  -d '{"symbol":"AAPL"}' \
  "$LIVE_ALPACA_ENDPOINT/v2/watchlists/{id}"
```

Returns the full updated watchlist. Duplicate symbols are silently ignored.

### Replace All Symbols — `PUT /v2/watchlists/{id}`

```bash
curl -X PUT \
  -H "APCA-API-KEY-ID: $LIVE_ALPACA_KEY" \
  -H "APCA-API-SECRET-KEY: $LIVE_ALPACA_SECRET" \
  -H "Content-Type: application/json" \
  -d '{"name":"Primary Watchlist","symbols":["AAPL","TSLA","NVDA"]}' \
  "$LIVE_ALPACA_ENDPOINT/v2/watchlists/{id}"
```

Replaces the entire symbol list atomically. Also used to rename the watchlist. Returns the full updated watchlist.

### Remove Symbol — `DELETE /v2/watchlists/{id}/{symbol}`

```bash
curl -X DELETE \
  -H "APCA-API-KEY-ID: $LIVE_ALPACA_KEY" \
  -H "APCA-API-SECRET-KEY: $LIVE_ALPACA_SECRET" \
  "$ALPACA_ENDPOINT/v2/watchlists/{id}/AAPL"
```

Returns the updated watchlist with the symbol removed.

---

## Existing Watchlist State (as of 2026-05-09)

**Watchlist ID:** `11111111-1111-1111-1111-111111111111`  
**Name:** Primary Watchlist  
**Symbols (9):** INTC, AMD, CAT, HOOD, TLRY, GLD, GLW, QCOM, TSM

Notable asset flags from this watchlist:
- `TLRY` — `shortable: false`, `easy_to_borrow: false` (cannabis stock, hard to borrow)
- All others — fully tradable, marginable, shortable, fractionable

---

## Authentication

All REST calls use headers (not query params):

```
APCA-API-KEY-ID: <key>
APCA-API-SECRET-KEY: <secret>
```

Note the header names always use `APCA-API-KEY-ID` / `APCA-API-SECRET-KEY` regardless of what the local env vars are named.

---

## Curl Test Scripts

Verify paper trading credentials and fetch watchlists:

```bash
#!/bin/bash
source .env

# Paper
curl -s \
  -H "APCA-API-KEY-ID: $PAPER_ALPACA_KEY" \
  -H "APCA-API-SECRET-KEY: $PAPER_ALPACA_SECRET" \
  "$PAPER_ALPACA_ENDPOINT/watchlists" | jq '[.[] | {name, id}]'

# Live
curl -s \
  -H "APCA-API-KEY-ID: $LIVE_ALPACA_KEY" \
  -H "APCA-API-SECRET-KEY: $LIVE_ALPACA_SECRET" \
  "$LIVE_ALPACA_ENDPOINT/v2/watchlists" | jq '[.[] | {name, id}]'
```

> Note: `PAPER_ALPACA_ENDPOINT` already includes `/v2` in the path, so watchlist calls append `/watchlists` directly. `LIVE_ALPACA_ENDPOINT` does not include `/v2`, so calls use `/v2/watchlists`.
