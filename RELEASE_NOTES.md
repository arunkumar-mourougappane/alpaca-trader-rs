# Release Notes — v0.7.1

**Release date:** 2026-05-22
**MSRV:** Rust 1.88+
**Previous release:** [v0.7.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.7.0)

---

## Overview

v0.7.1 is a patch release fixing a responsiveness regression during market hours.
No new features or breaking changes.

---

## Bug Fixes

### TUI Sluggishness During Market Hours (Closes #143)

**Problem:** The TUI became sluggish and unresponsive to keyboard input during market hours.

**Root cause:** The main loop called `terminal.draw()` on every event — including every
`MarketQuote` arriving from the WebSocket stream. With many symbols subscribed, quote events
arrive at high frequency, triggering continuous full re-renders that starved keyboard input.

**Fix:** Added a non-blocking drain loop after processing the first event. All queued events
are consumed in a single pass before the next `terminal.draw()`, decoupling render rate from
quote arrival rate.

```rust
// Before: one render per event (N quotes = N renders per frame)
// After: drain all queued events, then render once
while let Ok(event) = rx.try_recv() {
    update(&mut app, event);
    if app.should_quit { break; }
}
```

N quote events between frames now costs **1 render** instead of N renders. Keyboard input
remains responsive regardless of quote volume or number of subscribed symbols.

---

## Tests

**800 tests** (unchanged from v0.7.0) — this patch contains no new logic paths requiring
additional test coverage.

---

## No Breaking Changes

v0.7.1 is fully backwards-compatible with v0.7.0. All CLI flags, credential resolution,
environment variables, key bindings, and library API are unchanged.

---

## Getting Started

```bash
git clone https://github.com/arunkumar-mourougappane/alpaca-trader-rs
cd alpaca-trader-rs

./run.sh --paper   # paper trading (recommended for first run)
./run.sh           # live trading
```

See [README.md](README.md) for full setup and
[docs/credentials-setup.md](docs/credentials-setup.md) for API key setup.
