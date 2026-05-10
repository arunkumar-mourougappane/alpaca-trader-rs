#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Parse arguments
usage() {
    echo "Usage: $0 [--paper | --live]"
    echo ""
    echo "  --paper   Run against the paper trading environment (default)"
    echo "  --live    Run against the live trading environment (real money)"
    exit 1
}

ENV_OVERRIDE=""
for arg in "$@"; do
    case "$arg" in
        --paper) ENV_OVERRIDE="paper" ;;
        --live)  ENV_OVERRIDE="live" ;;
        --help|-h) usage ;;
        *) echo "Unknown argument: $arg"; usage ;;
    esac
done

# Load credentials
if [ -f .env ]; then
    set -a
    source .env
    set +a
else
    echo "Error: .env file not found."
    echo "Copy .env.example to .env and fill in your Alpaca API credentials."
    exit 1
fi

# CLI flag overrides .env value; fall back to paper if neither is set
if [ -n "$ENV_OVERRIDE" ]; then
    ALPACA_ENV="$ENV_OVERRIDE"
else
    ALPACA_ENV="${ALPACA_ENV:-paper}"
fi
export ALPACA_ENV

echo "Starting alpaca-trader-rs [$ALPACA_ENV]…"
cargo run --release --bin alpaca-trader
