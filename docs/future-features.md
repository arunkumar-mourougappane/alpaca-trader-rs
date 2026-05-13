# Future Features

Planned and proposed features not yet scheduled for a specific release.
Items here are candidates for future milestones. Developer/UX quality improvements
have been filed as GitHub issues and are not listed here.

---

## High Value / Medium Effort

### Paper → Live Trade Confirmation Modal
Before executing any order on the live account, display a "You are trading REAL money — confirm?" interstitial modal. This is a critical safety feature to prevent accidental live trades.

- **Trigger**: any order submission when `env == Live`
- **UI**: modal with order summary, `y` to confirm, `Esc` / `n` to cancel
- **Files**: `src/handlers/commands.rs`, `src/ui/modals.rs`, `src/input/modal.rs`

---

### Trade History Panel
Add a new panel (tab `5`) showing all closed orders with per-trade P&L, filterable by date range and symbol.

- **API**: `GET /v2/orders?status=closed&limit=500`
- **Columns**: Date, Symbol, Side, Qty, Avg Fill, P&L
- **Keybindings**: `/` to filter by symbol, `d` to filter by date range
- **Files**: new `src/ui/history.rs`, `src/handlers/rest.rs`, `src/input/history.rs`

---

### Price Alerts / Triggers
Allow the user to set a price threshold for any symbol. When the live quote crosses the threshold the TUI flashes a notification and rings the terminal bell.

- **Storage**: in-memory `HashMap<Symbol, AlertThreshold>` (persisted to config file when #61 lands)
- **Trigger**: evaluated in the market stream handler on each quote tick
- **UI**: `A` key in Watchlist panel → "Set Alert" modal; active alerts shown with `🔔` marker in the watchlist row
- **Files**: `src/app.rs`, `src/stream/`, `src/ui/watchlist.rs`, new `src/input/alerts.rs`

---

### Portfolio P&L Chart
Full-screen equity curve panel using `/v2/portfolio/history` data with a zoomable `Chart + GraphType::Line + Marker::Braille` widget (the no-fill chart approach from issue #58).

- **Timeframes**: 1D, 1W, 1M, 3M, 1Y selectable with `←`/`→`
- **Overlay**: show key events (large fills) as markers on the curve
- **Files**: new `src/ui/equity_chart.rs`, `src/handlers/rest.rs`

---

## Quick Wins / Low Effort

### Export to CSV
Export the current panel's data (positions, orders, trade history) to `~/alpaca-export-<date>.csv` with a single keypress (`X`).

- **Format**: UTF-8 CSV with header row matching the visible columns
- **Feedback**: brief status bar flash "Exported to ~/alpaca-export-2026-05-11.csv"
- **Files**: new `src/export.rs`, `src/input/mod.rs`

---

### Symbol News Feed
Add a scrollable news section at the bottom of the Symbol Detail modal, populated from Alpaca's `/v2/news?symbols=<SYMBOL>&limit=5` endpoint.

- **Display**: headline + source + published timestamp, `j`/`k` to scroll
- **Files**: `src/ui/modals.rs`, `src/handlers/rest.rs`, `src/types.rs`

---

### Multiple Watchlists
Support Alpaca's multiple named watchlists. Use `[`/`]` to cycle between watchlists; the panel header shows the active watchlist name.

- **API**: `GET /v2/watchlists` returns all lists; `GET /v2/watchlists/{id}` for symbols
- **Paper mode**: falls back to local-file watchlist (see issue #59)
- **Files**: `src/ui/watchlist.rs`, `src/app.rs`, `src/handlers/rest.rs`

---

### Local-File Watchlist Fallback for Paper Mode
When running in paper mode, the Alpaca watchlist API is unavailable. Store and load a watchlist from a local JSON file (`~/.config/alpaca-trader/watchlist.json`) so the panel is usable regardless of environment.

- **Related**: issue #59 (paper mode watchlist silent failure)
- **Files**: `src/handlers/rest.rs`, `src/ui/watchlist.rs`, new `src/local_watchlist.rs`

---

## Advanced / Longer Term

### Options Chain Viewer
Display a basic options chain (calls + puts by strike and expiry) for the selected symbol using Alpaca's options endpoints.

- **Columns**: Strike, Expiry, Bid, Ask, IV, Delta, Volume
- **Navigation**: `←`/`→` to switch expiry dates; `c`/`p` to toggle calls/puts
- **Files**: new `src/ui/options.rs`, `src/handlers/rest.rs`, `src/types.rs`

---

### Historical Backtester
Load OHLCV bars via `/v2/stocks/{symbol}/bars` and simulate a configurable moving-average crossover strategy, displaying cumulative P&L and trade log inline.

- **Parameters**: fast MA period, slow MA period, starting capital — entered via a config modal
- **Output**: equity curve (Chart widget), trade log table, summary stats (total return, Sharpe, max drawdown)
- **Files**: new `src/backtest/` module, new `src/ui/backtest.rs`

---

### Multi-Account Support
Allow switching between multiple Alpaca accounts (e.g., personal brokerage + IRA) at launch via a selector prompt or with a cycle key.

- **Config**: multiple `[account.*]` sections in `config.toml` (#61), each with its own env/key/secret
- **UI**: header badge shows active account nickname; `Ctrl-A` opens account switcher
- **Files**: `src/credentials.rs`, `src/config.rs`, `src/ui/dashboard.rs`

---

### Stock Screener
Filter the full universe of tradeable symbols by criteria (price range, volume, % change today, market cap) using Alpaca's bulk snapshot endpoints.

- **UI**: new panel (tab `6`) with filter fields at top and a scrollable results table
- **API**: `GET /v2/stocks/snapshots?symbols=...` or a third-party screener API
- **Files**: new `src/ui/screener.rs`, `src/handlers/rest.rs`

---

## Related Issues

| Issue | Title |
|-------|-------|
| [#51](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/51) | Header: PRE-MARKET / AFTER-HOURS state detection |
| [#52](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/52) | Account panel: Day P&L, Open P&L, Account #, equity sparkline |
| [#53](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/53) | Watchlist: columns (Volume not Ask/Bid), Change%, color-coding |
| [#54](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/54) | Symbol Detail modal: OHLCV, sparkline, w:Toggle Watchlist |
| [#55](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/55) | Positions: `s` key for SELL SHORT + status bar hint |
| [#56](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/56) | Order Entry: ↑/↓ cycling for dropdown fields |
| [#57](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/57) | About modal with app/author metadata |
| [#58](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/58) | Replace filled Sparkline with no-fill Chart + Braille line |
| [#59](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/59) | Bug: watchlist silently fails in paper trading mode |
| [#60](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/60) | `--dry-run` flag to simulate order submissions |
| [#61](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/61) | Persist user preferences to `~/.config/alpaca-trader/config.toml` |
| [#62](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/62) | Theme switching (default / dark / high-contrast) via `T` key |
