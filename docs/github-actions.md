# GitHub Actions for Rust Projects

Reference for every category of GitHub Actions workflow applicable to a Rust project. Includes YAML examples, action sources, and guidance on when to use each.

---

## Table of Contents

1. [Core CI](#1-core-ci)
2. [Security and Audit](#2-security-and-audit)
3. [Code Coverage](#3-code-coverage)
4. [Release Automation](#4-release-automation)
5. [Documentation](#5-documentation)
6. [Matrix Builds](#6-matrix-builds)
7. [Caching](#7-caching)
8. [Specialised Tools](#8-specialised-tools)
9. [Complete Workflow Examples](#9-complete-workflow-examples)

---

## 1. Core CI

### `actions/checkout`

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0   # full history — needed for changelog generation
```

### `dtolnay/rust-toolchain`

The recommended toolchain action. Lighter and faster than `actions-rs/toolchain`.

```yaml
- uses: dtolnay/rust-toolchain@stable

- uses: dtolnay/rust-toolchain@nightly

- uses: dtolnay/rust-toolchain@1.85.0   # pin to specific version

- uses: dtolnay/rust-toolchain@stable
  with:
    components: rustfmt, clippy
    targets: aarch64-unknown-linux-gnu
```

### `Swatinem/rust-cache`

Caches `~/.cargo` and `target/`. Always put immediately after toolchain setup.

```yaml
- uses: Swatinem/rust-cache@v2

# With options
- uses: Swatinem/rust-cache@v2
  with:
    prefix-key: "v2"          # bump to force cache bust
    shared-key: "build"       # share cache across jobs
    workspaces: |
      .
      crates/core
    env-vars: |
      RUSTFLAGS
      CC
```

### Test, Build, Lint, Format

```yaml
- run: cargo test --all-features --workspace

- run: cargo build --release

- run: cargo clippy --all-targets --all-features -- -D warnings

- run: cargo fmt --all -- --check
```

---

## 2. Security and Audit

### `cargo audit`

Checks dependencies against the [RustSec Advisory Database](https://rustsec.org/).

```yaml
- name: Security audit
  run: |
    cargo install cargo-audit --locked
    cargo audit
```

Or with the wrapper action:

```yaml
- uses: actions-rust-lang/audit@v1
  with:
    deny-warnings: true
```

**When:** Every CI run. Fast and catches known CVEs before they reach production.

### `cargo deny`

License compliance, banned crates, duplicate versions, and source verification.

```yaml
- uses: EmbarkStudios/cargo-deny-action@v1
  with:
    command: check all
    log-level: warn
```

Requires a `deny.toml` at the repo root:

```toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause"]

[bans]
deny = [{ name = "openssl" }]   # example: force rustls

[sources]
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
```

**When:** Projects with license compliance requirements or controlled dependency lists.

### Dependabot

Automatic dependency update PRs. Add `.github/dependabot.yml`:

```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
    open-pull-requests-limit: 5
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
```

**When:** Always. Low-effort, high-value for keeping dependencies current.

---

## 3. Code Coverage

### `cargo-llvm-cov` (preferred)

LLVM source-based instrumentation. More accurate than tarpaulin and cross-platform.

```yaml
- uses: taiki-e/install-action@cargo-llvm-cov

- name: Generate coverage
  run: cargo llvm-cov --all-features --workspace --lcov --output-path coverage.lcov

- uses: codecov/codecov-action@v3
  with:
    files: ./coverage.lcov
    fail_ci_if_error: true
```

### `cargo-tarpaulin`

Linux-only but simpler setup. Suitable for existing projects already using it.

```yaml
- name: Coverage with tarpaulin
  run: |
    cargo install cargo-tarpaulin --locked
    cargo tarpaulin --out Lcov --output-dir ./coverage

- uses: codecov/codecov-action@v3
  with:
    files: ./coverage/lcov.info
```

**When:** Use `cargo-llvm-cov` for new projects. `tarpaulin` for Linux-only projects with existing setup.

---

## 4. Release Automation

### `taiki-e/upload-rust-binary-action`

Builds and uploads pre-compiled binaries to a GitHub Release across multiple targets.

```yaml
name: Release
on:
  push:
    tags: ["v*"]

jobs:
  upload-assets:
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-pc-windows-msvc
            os: windows-latest

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: taiki-e/upload-rust-binary-action@v1
        with:
          bin: alpaca-trader
          target: ${{ matrix.target }}
          archive: $name-$target.$ext
          token: ${{ secrets.GITHUB_TOKEN }}
```

### `cargo-release`

Version bumping, git tagging, and crates.io publishing.

```yaml
name: Publish
on:
  workflow_dispatch:
    inputs:
      level:
        description: "patch | minor | major"
        required: true
        default: patch

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
      - uses: dtolnay/rust-toolchain@stable
      - name: Publish
        run: |
          cargo install cargo-release --locked
          cargo release ${{ github.event.inputs.level }} --execute
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
```

### Cross-compilation with `cross`

```yaml
- name: Install cross
  run: cargo install cross --locked

- name: Build for Linux ARM64
  run: cross build --target aarch64-unknown-linux-gnu --release

- name: Build for Windows (from Linux)
  run: cross build --target x86_64-pc-windows-gnu --release
```

### Cross-compilation with `cargo-zigbuild`

Modern alternative to `cross`. Uses Zig as the linker, no Docker required.

```yaml
- uses: taiki-e/install-action@cargo-zigbuild

- run: cargo zigbuild --target x86_64-unknown-linux-musl --release
- run: cargo zigbuild --target aarch64-unknown-linux-gnu --release
```

---

## 5. Documentation

### `cargo doc` → GitHub Pages

```yaml
name: Docs

on:
  push:
    branches: [main]

permissions:
  contents: read
  pages: write
  id-token: write

jobs:
  deploy:
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Build docs
        run: cargo doc --no-deps --all-features
        env:
          RUSTDOCFLAGS: "-D warnings"
      - uses: actions/upload-pages-artifact@v2
        with:
          path: target/doc
      - uses: actions/deploy-pages@v2
        id: deployment
```

Enable GitHub Pages in repo Settings → Pages → Source: GitHub Actions.

### docs.rs (for published crates)

No workflow needed — docs.rs builds documentation automatically on every crates.io publish. Configure in `Cargo.toml`:

```toml
[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

---

## 6. Matrix Builds

### Multi-OS × multi-toolchain

```yaml
jobs:
  test:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable, beta, nightly]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@${{ matrix.rust }}
      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: ${{ matrix.os }}-${{ matrix.rust }}
      - run: cargo test --all-features
```

### MSRV check

```yaml
  msrv:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.85.0   # your declared MSRV
      - uses: Swatinem/rust-cache@v2
      - run: cargo test
```

### Feature flag combinations with `cargo-hack`

Tests every combination of feature flags, catching compile errors that only appear with certain combinations.

```yaml
- uses: taiki-e/install-action@cargo-hack

- run: cargo hack test --feature-powerset

# Cap combinatorial explosion
- run: cargo hack test --feature-powerset --depth 2

# Lint all combinations
- run: cargo hack clippy --feature-powerset -- -D warnings
```

**When:** Libraries with multiple feature flags. Not needed for applications.

---

## 7. Caching

`Swatinem/rust-cache` handles the common case. For fine-grained control:

```yaml
# Manual cache (if you need more control than rust-cache provides)
- uses: actions/cache@v4
  with:
    path: |
      ~/.cargo/registry/index/
      ~/.cargo/registry/cache/
      ~/.cargo/git/db/
      target/
    key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    restore-keys: |
      ${{ runner.os }}-cargo-
```

**Cache invalidation:** Change `prefix-key` in `Swatinem/rust-cache` (or the `key` prefix above) whenever you need to force a clean build across all branches.

---

## 8. Specialised Tools

### `cargo-semver-checks`

Detects breaking API changes before release by comparing against the published version.

```yaml
- uses: obi1kenobi/cargo-semver-checks-action@v2
  with:
    package: alpaca-trader-rs
```

**When:** Before every release of a library crate.

### `cargo-msrv`

Finds or verifies the minimum supported Rust version.

```yaml
- name: Verify MSRV
  run: |
    cargo install cargo-msrv --locked
    cargo msrv verify   # checks against rust-version in Cargo.toml
```

**When:** Before releases to confirm the declared MSRV is accurate.

### `cargo-mutants`

Mutation testing — checks that tests actually catch bugs.

```yaml
- name: Mutation testing
  run: |
    cargo install cargo-mutants --locked
    cargo mutants --timeout 60
```

**When:** Periodically on important modules, not every CI run (slow).

### `cargo-udeps`

Finds unused dependencies.

```yaml
- name: Check for unused deps
  run: |
    cargo install cargo-udeps --locked
    cargo +nightly udeps --all-targets
```

**When:** Periodically or before releases to keep `Cargo.toml` clean.

---

## 9. Complete Workflow Examples

### Minimal CI (fast feedback)

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  ci:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test --all-features
```

### Full CI (thorough, multi-platform)

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  fmt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --all-targets --all-features -- -D warnings

  test:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all-features

  msrv:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.85.0
      - uses: Swatinem/rust-cache@v2
      - run: cargo test

  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo install cargo-audit --locked
      - run: cargo audit

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: taiki-e/install-action@cargo-llvm-cov
      - uses: Swatinem/rust-cache@v2
      - run: cargo llvm-cov --all-features --lcov --output-path coverage.lcov
      - uses: codecov/codecov-action@v3
        with:
          files: ./coverage.lcov

  docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo doc --no-deps --all-features
        env:
          RUSTDOCFLAGS: "-D warnings"
```

### Release workflow (binary distribution)

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ["v*"]

jobs:
  upload-assets:
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-pc-windows-msvc
            os: windows-latest
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: taiki-e/upload-rust-binary-action@v1
        with:
          bin: alpaca-trader
          target: ${{ matrix.target }}
          archive: $name-$target.$ext
          token: ${{ secrets.GITHUB_TOKEN }}
```

---

## Quick Reference: Action → Use Case

| Action / Tool | Use case | Frequency |
|---|---|---|
| `dtolnay/rust-toolchain` | Toolchain setup | Every workflow |
| `Swatinem/rust-cache` | Build caching | Every workflow |
| `cargo fmt -- --check` | Format gate | Every PR |
| `cargo clippy -D warnings` | Lint gate | Every PR |
| `cargo test` | Test suite | Every PR |
| `cargo audit` | Security scan | Every PR |
| `EmbarkStudios/cargo-deny-action` | License + dependency policy | Every PR |
| `cargo-llvm-cov` + Codecov | Coverage tracking | Every PR |
| `taiki-e/upload-rust-binary-action` | Binary releases | On tag push |
| `cargo-release` | Version + publish | Manual / on tag |
| `actions/deploy-pages` | API docs | On main push |
| `cargo-hack --feature-powerset` | Feature flag matrix | Libraries, weekly |
| `cargo-semver-checks` | API compat check | Before releases |
| `cargo-msrv verify` | MSRV check | Before releases |
| `cargo-udeps` | Unused dep cleanup | Periodically |
| `cargo-mutants` | Mutation testing | Periodically |
| Dependabot | Dep updates | Weekly (automatic) |
