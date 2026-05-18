# Contributing to alpaca-trader-rs

First off, thank you for considering contributing to `alpaca-trader-rs`! It's people like you that make open source such a great community.

Please take a moment to review this document in order to make the contribution process easy and effective for everyone involved.

## Code of Conduct

This project and everyone participating in it is governed by the `alpaca-trader-rs` [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## Getting Started

1. **Fork the repository** on GitHub.
2. **Clone your fork** locally: `git clone https://github.com/YOUR_USERNAME/alpaca-trader-rs.git`
3. **Set up credentials** following [docs/credentials-setup.md](docs/credentials-setup.md).
4. **Create a branch** for your feature or bug fix: `git checkout -b my-new-feature`

## Development Workflow

### Rust Ecosystem Standards

We rely on the standard Rust tooling to maintain code quality:

*   **Formatting:** Run `cargo fmt` to format your code before committing. CI will fail if the code is not formatted.
*   **Linting:** Run `cargo clippy -- -D warnings` to catch common mistakes and improve your Rust code. Please address all warnings.
*   **Building:** `cargo build` should compile without errors.

### Testing

Tests are a crucial part of this project. **All new features and bug fixes must include tests.**

*   Run the test suite locally: `cargo test`
*   For an overview of our testing strategy, including mock patterns for the API client, please read [docs/testing.md](docs/testing.md).
*   If you are fixing a bug, please write a test that reproduces the bug before fixing it, ensuring it fails first and then passes with your fix.

### Architecture

This application is built using a unidirectional data flow (TEA - The Elm Architecture) adapted for Rust.

*   **State:** The `App` struct is the single source of truth.
*   **Events:** The `Event` enum represents all possible state changes.
*   **Update:** The `update` function handles state transitions based on incoming events.
*   **View:** The UI is rendered purely as a function of the current state.

Before contributing significant architectural changes or new UI panels, please familiarize yourself with [docs/architecture.md](docs/architecture.md) and [docs/ui-mockups.md](docs/ui-mockups.md).

## Submitting a Pull Request

1. **Commit your changes:** Write clear, concise commit messages.
2. **Push to your fork:** `git push origin my-new-feature`
3. **Open a Pull Request (PR):**
    *   Describe the changes you've made in detail.
    *   Link any relevant issues (e.g., "Fixes #123").
    *   Ensure CI checks pass (formatting, clippy, tests, and security audits).
    *   If your PR changes the UI or public API, ensure the relevant documentation in `docs/` or `README.md` is updated.

## Licensing

By contributing to `alpaca-trader-rs`, you agree that your contributions will be dual-licensed under the **MIT License** and the **Apache License, Version 2.0**, matching the rest of the project. See [docs/licensing.md](docs/licensing.md) for more details.
