# UI Mockups & Interaction Design

ASCII mockups and full keyboard/mouse interaction specification for `alpaca-trader-rs`.

---

## Global Shell

Every screen shares this outer chrome:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ [PAPER] alpaca-trader-rs           Market: OPEN   09:45:23 ET   2026-05-09  │
├──────────────────────────────────────────────────────────────────────────────┤
│  1:Account  2:Watchlist  3:Positions  4:Orders                               │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  (active panel — see sections below)                                        │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ ?:Help  q:Quit  Tab:Switch Panel  r:Refresh  o:Order                        │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Header (row 1)**
- Left: `[PAPER]` cyan / `[LIVE]` red badge — set from `ALPACA_ENV`
- Center: app name
- Right: market state (`OPEN` / `CLOSED` / `PRE-MARKET` / `AFTER-HOURS`) + clock + date, updated every second via `Event::Tick`

**Tab bar (row 2)**
- Active tab: bold + underlined; inactive: dimmed
- Mouse-clickable

**Status bar (last row)**
- Context-sensitive: shows shortcuts relevant to the currently active panel

---

## Panel 1 — Account

```
┌─ Account ────────────────────────────────────────────────────────────────────┐
│                                                                              │
│   Portfolio Value    $125,432.18       Day P&L    +$843.22  (+0.68%)        │
│   Buying Power        $48,210.00       Open P&L   +$1,204.50                │
│   Cash                $48,210.00       Account #  PA1234567                 │
│   Long Market Value   $77,222.18       Status     ACTIVE                    │
│                                                                              │
│  ── Today's Equity Curve ───────────────────────────────────────────────── │
│                                                                              │
│   ▁▂▃▄▄▅▄▅▆▆▅▄▃▄▅▆▇▇▆▅▄▃▂▃▄▅▆▆▇▇█▇▆▅▄▅▆▇▇▆▅▄▅▆                           │
│   09:30                12:00                              16:00             │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

- Fields sourced from `GET /v2/account`
- Day P&L / Open P&L: green if positive, red if negative
- Sparkline: intraday equity history, updated on each `Event::AccountUpdated`
- No cursor/selection — display only

---

## Panel 2 — Watchlist

```
┌─ Watchlist: Primary Watchlist ──────────────────────────────────────────────┐
│  Symbol   Name                          Price      Change      Volume       │
│ ──────────────────────────────────────────────────────────────────────────  │
│  INTC    Intel Corporation              $24.18    -0.42%      45.2M         │
│▶ AMD     Advanced Micro Devices        $142.85    +1.24%      28.7M         │
│  CAT     Caterpillar Inc.              $318.42    +0.87%       1.2M         │
│  HOOD    Robinhood Markets              $28.63    +3.41%       8.9M         │
│  TLRY    Tilray Brands                   $1.84    -1.60%      12.1M         │
│  GLD     SPDR Gold Trust               $305.22    +0.32%       4.5M         │
│  GLW     Corning Incorporated           $47.18    -0.18%       3.2M         │
│  QCOM    QUALCOMM                      $168.40    +0.72%       6.8M         │
│  TSM     Taiwan Semiconductor          $182.15    +1.15%       7.3M         │
│                                                                              │
│ a:Add  d:Remove  Enter:Detail  /:Search                                     │
└──────────────────────────────────────────────────────────────────────────────┘
```

- `▶` cursor row highlighted; follows `j`/`k` and mouse click
- Price and Change: green if positive, red if negative
- Prices update live from WebSocket `Event::MarketQuote`
- `/` opens an inline search bar above the table header that filters rows by symbol as you type

---

## Panel 3 — Positions

```
┌─ Positions ──────────────────────────────────────────────────────────────────┐
│  Symbol   Qty    Avg Cost    Cur Price   Mkt Value    Unrealized P&L    %   │
│ ──────────────────────────────────────────────────────────────────────────  │
│▶ AMD       50    $138.20     $142.85     $7,142.50    +$232.50       +3.36% │
│  NVDA      10    $875.00     $922.40     $9,224.00    +$474.00       +5.42% │
│  INTC     200     $26.10      $24.18     $4,836.00    -$384.00       -7.36% │
│                                                                              │
│  ─────────────────────────────────────────────────────────────────────────  │
│  Total Long: $21,202.50    Total Unrealized: +$322.50  (+1.54%)            │
│                                                                              │
│ o:Close  s:Short  Enter:Detail                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

- Cur Price column updates from `Event::MarketQuote` on each tick
- Unrealized P&L and % columns: green / red
- Footer totals recalculate on every price update
- `o` opens Order Entry pre-filled with current symbol + SELL
- `s` opens Order Entry pre-filled with current symbol + SELL SHORT

---

## Panel 4 — Orders

```
┌─ Orders ─────────────────────────────────────────────────────────────────────┐
│  [ Open (3) ]  Filled (12)  Cancelled (2)                                   │
│ ──────────────────────────────────────────────────────────────────────────  │
│  ID        Symbol  Side   Qty   Type     Limit     Status     Submitted     │
│ ──────────────────────────────────────────────────────────────────────────  │
│▶ a3f2…     AMD     BUY     10   LIMIT   $141.00    PENDING    09:32:15      │
│  b7c1…     NVDA    BUY      5   MARKET  —          PENDING    09:28:44      │
│  f2d9…     INTC    SELL   100   LIMIT    $25.50    PENDING    09:15:02      │
│                                                                              │
│ o:New Order  c:Cancel Selected  Enter:Detail  1-3:Filter Tab                │
└──────────────────────────────────────────────────────────────────────────────┘
```

- Sub-tabs (Open / Filled / Cancelled): active tab in brackets `[ ]`, clickable with mouse
- BUY: green; SELL: red
- Limit column shows `—` for MARKET orders
- `c` cancels the highlighted order with a confirmation prompt
- Order state updates arrive from `Event::TradeUpdate` via the account WebSocket stream

---

## Modal — Order Entry

Triggered by `o` from any panel. Pre-fills Symbol if a row is selected.

```
╔═ New Order ══════════════════════════════╗
║                                          ║
║  Symbol  [ AMD            ]              ║
║  Side    ( ● BUY )  ( ○ SELL )          ║
║  Type    [ LIMIT  ▾       ]              ║
║  Qty     [ 10             ]              ║
║  Price   [ 141.00         ]  (limit only)║
║                                          ║
║  ─────────────────────────────────────   ║
║  Est. Total    $1,410.00                 ║
║  Buying Power  $48,210.00  ✓ sufficient  ║
║                                          ║
║       [ Submit Order ]  [ Cancel ]       ║
║                                          ║
║  Tab:Next Field  Enter:Submit  Esc:Close ║
╚══════════════════════════════════════════╝
```

**Behavior:**
- Price field is hidden / greyed when Type = MARKET
- Est. Total recalculates as Qty and Price change
- Buying Power indicator: green `✓ sufficient` / red `✗ insufficient`
- Submit button is disabled (dimmed) when buying power is insufficient or required fields are empty
- `Tab` / `Shift-Tab` moves focus between fields; focused field has a highlighted border

---

## Modal — Symbol Detail

Triggered by `Enter` on a Watchlist or Positions row.

```
╔═ AMD — Advanced Micro Devices ═══════════╗
║                                          ║
║  Price   $142.85    Change  +1.24%       ║
║  Open    $141.10    High    $143.20      ║
║  Low     $140.85    Volume  28.7M        ║
║                                          ║
║  ── Intraday ──────────────────────────  ║
║  ▁▂▃▄▅▄▅▆▇▆▅▄▅▆▇█▇▆▅▄▃▄▅▆▇▆▅▄▅▆▇▆▅▄▅  ║
║  09:30                             16:00 ║
║                                          ║
║  Exchange    NASDAQ   Class    us_equity ║
║  Tradable    ✓        Shortable ✓        ║
║  Fractionable ✓       ETB      ✓        ║
║                                          ║
║  o:Buy  s:Sell  w:Toggle Watchlist  Esc  ║
╚══════════════════════════════════════════╝
```

- Price and Change update live from WebSocket while modal is open
- `w` adds/removes symbol from the watchlist (toggles)
- Asset flags (`Tradable`, `Shortable`, `ETB`, `Fractionable`) sourced from watchlist asset data

---

## Modal — Help Overlay

Triggered by `?` from any context.

```
╔═ Keyboard Shortcuts ═════════════════════╗
║                                          ║
║  NAVIGATION                              ║
║  1/2/3/4 or Tab    Switch panels         ║
║  j / k  or ↑/↓    Move cursor           ║
║  g / G             Top / Bottom          ║
║  Enter             Open detail           ║
║  Esc               Close / Cancel        ║
║                                          ║
║  ACTIONS                                 ║
║  o    New order (pre-fills symbol)       ║
║  c    Cancel selected order              ║
║  a    Add symbol to watchlist            ║
║  d    Remove symbol from watchlist       ║
║  r    Force refresh                      ║
║  /    Search / filter                    ║
║                                          ║
║  GLOBAL                                  ║
║  q / Ctrl-C   Quit                       ║
║  ?            This help screen           ║
║                                          ║
║             Press any key to close       ║
╚══════════════════════════════════════════╝
```

---

## Keyboard Interaction Model

### Global (always active)

| Key | Action |
|-----|--------|
| `1` / `2` / `3` / `4` | Switch to Account / Watchlist / Positions / Orders |
| `Tab` / `Shift-Tab` | Cycle tabs forward / backward |
| `q` / `Ctrl-C` | Quit |
| `r` | Force REST re-poll |
| `?` | Toggle help overlay |
| `Esc` | Close any open modal |

### List Navigation (Watchlist, Positions, Orders)

| Key | Action |
|-----|--------|
| `j` / `↓` | Move cursor down one row |
| `k` / `↑` | Move cursor up one row |
| `g` | Jump to first row |
| `G` | Jump to last row |
| `Enter` | Open Symbol Detail modal for selected row |

### Watchlist Panel

| Key | Action |
|-----|--------|
| `a` | Open Add Symbol text input |
| `d` | Remove selected symbol (confirmation prompt) |
| `/` | Focus inline search bar; filters rows as you type |

### Positions Panel

| Key | Action |
|-----|--------|
| `o` | Open Order Entry pre-filled: selected symbol + SELL |
| `s` | Open Order Entry pre-filled: selected symbol + SELL SHORT |

### Orders Panel

| Key | Action |
|-----|--------|
| `o` | Open Order Entry (blank) |
| `c` | Cancel selected order (confirmation prompt) |
| `1` / `2` / `3` | Switch sub-tabs: Open / Filled / Cancelled |

### Order Entry Modal

| Key | Action |
|-----|--------|
| `Tab` / `Shift-Tab` | Move focus forward / backward between fields |
| `↑` / `↓` | Cycle values in dropdown fields (Side, Type) |
| `Enter` | Advance to next field; submit when Submit button is focused |
| `Esc` | Close modal without submitting |

---

## Mouse Interaction Model

| Element | Left Click | Scroll |
|---------|-----------|--------|
| Tab bar | Switch to that panel | — |
| List row | Select (move cursor) | Scroll list up/down |
| Sub-tabs (Orders) | Switch sub-tab | — |
| Modal: text input | Focus field | — |
| Modal: radio button | Select option | — |
| Modal: dropdown | Open dropdown | — |
| Modal: dropdown option | Select and close | — |
| Modal: Submit / Cancel | Activate button | — |

Mouse support requires `crossterm` with the `event-stream` feature and `crossterm::execute!(stdout, EnableMouseCapture)` at startup. Hit positions for all interactive elements are calculated from the rendered `Rect` areas and stored in `App` state each frame.

---

## ratatui Widget Mapping

| UI Element | ratatui Widget |
|---|---|
| Header / status bar | `Paragraph` with `Line` and styled `Span`s |
| Tab bar | `Tabs` widget |
| Data tables | `Table` + `TableState` (carries selected row index) |
| Sparklines (equity, intraday) | `Sparkline` |
| Modal background overlay | `Clear` rendered over a centered `Rect` |
| Modal container | `Block` with double border `BorderType::Double` |
| Text input fields | `Paragraph` in edit mode; cursor rendered as `▌` |
| Radio buttons (BUY/SELL) | `Paragraph` with `●`/`○` styled spans |
| Dropdown (Type) | `List` inside a small popup `Block` |
| Confirmation prompt | `Paragraph` in a small `Clear` + `Block` popup |
| Help overlay | `Table` (two-column: key / description) inside `Clear` + `Block` |
| Inline search bar | `Paragraph` in a 1-row `Block` above the table |
