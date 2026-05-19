# Roadmap

This document tracks the planned feature releases for **alpaca-trader-rs**.
Each release has a corresponding [GitHub Milestone](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/milestones)
that links individual issues to a target version.

---

## Released

### [v0.5.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.5.0) — 2026-05-18 ✅

UX quality, portability, and safety:
`--dry-run` flag · Persistent TOML preferences · Runtime theme switching (`T`) ·
Windows build + CI coverage · Braille line charts · Instant resize redraw ·
449 tests

---

## Upcoming

### [v0.6.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/milestone/2) — UX Polish & Table Improvements

> Target: 2026-06-15

Quick, high-visibility improvements to the existing panels and interaction model.

| Issue | Feature |
|---|---|
| [#96](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/96) | P&L summary footer row in the Positions panel |
| [#93](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/93) | `gg` / `G` jump-to-top / jump-to-bottom in all tables |
| [#85](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/85) | Order fill notifications — flash status bar when a trade executes |
| [#84](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/84) | Confirmation modal before removing a symbol from the watchlist |
| [#83](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/83) | Copy symbol to clipboard with `c` keybinding |
| [#81](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/81) | Refresh visual feedback — spinner and last-updated timestamp in header |
| [#79](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/79) | Status bar message queue — prevent rapid events from overwriting each other |
| [#78](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/78) | `filled_avg_price` field in Order type + Filled Price column in Orders table |

---

### [v0.7.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/milestone/3) — Data Depth & Navigation

> Target: 2026-07-15

Richer data views, deeper navigation, and improved mouse/keyboard interaction.

| Issue | Feature |
|---|---|
| [#97](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/97) | Buying power and account metrics breakdown in the Account panel |
| [#95](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/95) | Intraday chart crosshair — price and time tooltip when navigating chart data |
| [#92](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/92) | Mouse click support for tabs, table rows, and modal buttons |
| [#91](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/91) | Keyboard shortcut help overlay (`?` key) |
| [#87](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/87) | Position detail modal — `Enter` on a position row opens dedicated view |
| [#86](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/86) | Orders filter by symbol — show orders for the selected ticker |
| [#82](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/82) | Column sorting in Positions and Orders tables |
| [#80](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/80) | Global symbol search — open detail modal for any ticker not in watchlist |
| [#77](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/77) | Portfolio equity curve date-range toggle (Intraday / 1W / 1M) |

---

### [v0.8.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/milestone/4) — Trading Capabilities

> Target: 2026-08-15

Expanded order types, risk management tools, and improved reliability.

| Issue | Feature |
|---|---|
| [#94](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/94) | Extended order types in Order Entry (stop, stop-limit, trailing stop, extended hours) |
| [#102](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/102) | Bracket order support (take-profit + stop-loss legs) in Order Entry modal |
| [#101](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/101) | Price alert triggers with terminal bell notification |
| [#88](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/88) | Implement remaining `AlpacaClient` methods (`get_asset`, `replace_watchlist`, etc.) |
| [#76](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/76) | WebSocket auto-reconnect with exponential back-off and UI retry indicator |

---

### [v0.9.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/milestone/5) — Power-User & Integration

> Target: 2026-09-15

Data portability, external integrations, and power-user workflow features.

| Issue | Feature |
|---|---|
| [#105](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/105) | CSV snapshot export of positions and P&L on demand or each refresh tick |
| [#103](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/103) | Structured log file output via `--log-file` CLI flag or config option |
| [#100](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/100) | Watchlist bulk import and export (comma-separated or one-per-line text) |
| [#98](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/98) | News feed for the selected symbol in the Symbol Detail modal |
| [#89](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/89) | Runtime paper ↔ live account toggle without app restart |

---

### [v1.0.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/milestone/6) — General Availability

> Target: 2026-11-01

Production-ready release: full feature parity, distribution polish, and enterprise networking.

| Issue | Feature |
|---|---|
| [#90](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/90) | HTTP/SOCKS5 proxy support for REST and WebSocket connections |
| [#104](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/104) | Shell completion scripts via `completions` subcommand (bash, zsh, fish, PowerShell) |
| [#99](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/issues/99) | Market hours indicator in the header (PRE-MARKET / OPEN / AFTER-HOURS / CLOSED) |

---

## Guiding Principles

- **Each release ships as a tagged, signed binary** available on the [Releases page](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases).
- **No breaking changes** to the library public API without a major version bump.
- **Test coverage must not regress** — new features must ship with tests.
- **All milestones are aspirational** — issues may move between releases as scope becomes clearer.
- **Have a feature request?** Open an issue and it will be triaged into a milestone.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The milestone for each issue indicates its planned
release — picking up an issue targeting the next milestone is the best way to contribute.
