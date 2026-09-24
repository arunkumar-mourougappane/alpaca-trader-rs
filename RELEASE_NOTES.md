# Release Notes — v0.8.2

**Release date:** 2026-09-24
**MSRV:** Rust 1.88+ (unchanged)
**Previous release:** [v0.8.1](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.8.1)

---

## Overview

v0.8.2 is a **maintenance release**. `src/` and `tests/` are byte-identical to v0.8.1 — there are no new features, no behaviour changes, and no bug fixes in application code. The release ships two things:

1. **A dependency refresh that closes a security advisory** — `rustls` moves to 0.23.45, clearing RUSTSEC-2026-0285, alongside routine bumps to `clap`, `dirs`, `reqwest`, and `toml`
2. **A license file cleanup** — the two license files collapse into a single `LICENSE`

The advisory is the reason to take this release. Everything else is housekeeping.

---

## Security

**RUSTSEC-2026-0285 — `rustls` 0.23.44 → 0.23.45** (medium, CVSS 5.3)

`rustls` 0.23.44 incorrectly accepted TLS 1.3 handshake messages across encryption level boundaries. `cargo audit` flagged it against the v0.8.1 lockfile.

`rustls` is transitive here — it arrives through `reqwest` directly and via `hyper-rustls`, `tokio-rustls`, and `rustls-platform-verifier` — so the fix is a lockfile bump with no `Cargo.toml` change and no code change. `cargo audit` reports no vulnerabilities against this release. (#201)

If you depend on `alpaca-trader-rs` as a library, your own lockfile decides your `rustls` version; run `cargo update -p rustls` to pick up 0.23.45 regardless of which version of this crate you are on.

---

## Dependency Updates

Direct dependencies:

| Crate | v0.8.1 | v0.8.2 | PR |
|---|---|---|---|
| `clap` | 4.6.6 | 4.6.7 | #200 |
| `dirs` | 6.0.0 | 7.0.0 | #197 |
| `reqwest` | 0.13.4 | 0.13.5 | #198 |
| `toml` | 1.1.5+spec-1.1.0 | 1.1.6+spec-1.1.0 | #199 |

Transitively, `rustls` 0.23.44 → 0.23.45 (above) and `base64` 0.23.1 enters the lockfile next to the existing 0.22.1 as a new `reqwest` dependency. The locked package count goes from 449 to 450.

**On `dirs` 6 → 7:** the only breaking change in `dirs` 7.0.0 is `preference_dir` on Windows, which now resolves to `RoamingAppData` instead of `LocalAppData`. This crate does not call `preference_dir` — `src/prefs.rs` uses `config_dir` and `src/logging.rs` uses `home_dir` and `data_local_dir`. Config and log file locations are unchanged on every platform, and no migration is needed.

---

## Licensing

The project carried two license files: `LICENSE-MIT` with the license text and `LICENSE.md`, a short pointer to it. They are now a single `LICENSE` — the conventional name GitHub, crates.io, and cargo packaging look for.

The license itself does not change: the text is the standard unmodified MIT License, and `Cargo.toml` still declares `license = "MIT"`. Links in `README.md`, `docs/licensing.md`, and `docs/architecture.md` were updated to the new path.

---

## Tests

**1254 tests total** — unchanged from v0.8.1, all passing against the refreshed lockfile:

| Scope | Count |
|---|---|
| Library (`src/lib.rs`: `types`, `config`, `stream`, `prefs`, `logging`) | 134 |
| App (`src/main.rs`: `app`, `update`, `input/*`, `ui/*`, `handlers/*`) | 1090 |
| HTTP integration (`tests/client_tests.rs`) | 29 |
| Doc-tests | 1 |

---

## Upgrade Notes

No action required. `config.toml`, credentials, and stored alerts are read unchanged; there are no new or renamed configuration keys, and no API surface has changed since v0.8.1.

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
