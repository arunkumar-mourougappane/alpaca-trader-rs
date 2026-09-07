# Release Notes — v0.8.1

**Release date:** 2026-09-07
**MSRV:** Rust 1.88+ (unchanged)
**Previous release:** [v0.8.0](https://github.com/arunkumar-mourougappane/alpaca-trader-rs/releases/tag/v0.8.0)

---

## Overview

v0.8.1 is a **maintenance release**. `src/` and `tests/` are byte-identical to v0.8.0 — there are no new features, no behaviour changes, and no bug fixes. The release ships two things:

1. **A refreshed dependency tree** — nine direct dependencies and 186 packages overall move forward, most notably `tokio` 1.52.3 → 1.53.1 and `tokio-tungstenite` 0.29 → 0.30
2. **A CI configuration fix** — Dependabot no longer proposes bumps that repoint the MSRV job at nonexistent Rust toolchains

If you are already on v0.8.0 and build from a published binary, there is nothing user-visible in this release. It matters mainly if you depend on `alpaca-trader-rs` as a library and want the newer transitive versions in your own lockfile.

---

## Dependency Updates

Direct dependencies:

| Crate | v0.8.0 | v0.8.1 |
|---|---|---|
| `anyhow` | 1.0.103 | 1.0.104 |
| `clap` | 4.6.1 | 4.6.6 |
| `futures` | 0.3.32 | 0.3.34 |
| `serde` | 1.0.228 | 1.0.229 |
| `serde_json` | 1.0.150 | 1.0.151 |
| `tokio` | 1.52.3 | 1.53.1 |
| `tokio-tungstenite` | 0.29.0 | 0.30.0 |
| `tokio-util` | 0.7.18 | 0.7.19 |
| `toml` | 1.1.2+spec-1.1.0 | 1.1.5+spec-1.1.0 |

Across the whole lockfile, 186 packages change version, 8 are added and 16 are dropped, leaving 450 locked packages (down from 453). The largest transitive move is `aws-lc-sys` 0.40.0 → 0.45.0, which also drops the `wit-bindgen` / `wasm-encoder` build crates it no longer needs.

**On `tokio-tungstenite` 0.29 → 0.30:** this is a major version bump of a dependency, but it is used only inside `src/stream/account.rs` and `src/stream/market.rs` and does not appear in any public signature. Library consumers are unaffected — no type from `tokio_tungstenite` crosses the crate boundary.

---

## Internal

- **Dependabot MSRV ignore rule fixed** (`.github/dependabot.yml`) — the rule added in #49 used `versions: ["*"]`, which is not honoured as a range for the `github-actions` ecosystem. Dependabot therefore kept opening PRs that repointed the MSRV job at `dtolnay/rust-toolchain`'s placeholder branches for unreleased Rust versions, failing CI with `could not download nonexistent rust version` (most recently 1.120 in #191). Dropping `versions` makes the rule ignore every update for that action, keeping the MSRV job pinned to 1.88. (#194)

---

## Tests

**1254 tests total** — unchanged from v0.8.0, all passing against the refreshed lockfile:

| Scope | Count |
|---|---|
| Library (`src/lib.rs`: `types`, `config`, `stream`, `prefs`, `logging`) | 134 |
| App (`src/main.rs`: `app`, `update`, `input/*`, `ui/*`, `handlers/*`) | 1090 |
| HTTP integration (`tests/client_tests.rs`) | 29 |
| Doc-tests | 1 |

---

## Upgrade Notes

No action required. `config.toml`, credentials, and stored alerts are read unchanged; there are no new or renamed configuration keys, and no API surface has changed since v0.8.0.

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
