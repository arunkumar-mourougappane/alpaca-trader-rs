#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Parse arguments
usage() {
    echo "Usage: $0 [--paper | --live]"
    echo ""
    echo "  --paper   Run against the paper trading environment (simulated funds)"
    echo "  --live    Run against the live trading environment (real money — default)"
    exit 1
}

PAPER_FLAG=""
for arg in "$@"; do
    case "$arg" in
        --paper) PAPER_FLAG="--paper" ;;
        --live)  ;;  # live is the default; accepted for backwards compatibility
        --help|-h) usage ;;
        *) echo "Unknown argument: $arg"; usage ;;
    esac
done

# Load credentials from .env if present (developer convenience)
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

ENV_LABEL="live"
[ -n "$PAPER_FLAG" ] && ENV_LABEL="paper"
echo "Starting alpaca-trader-rs [$ENV_LABEL]…"
cargo run --release --bin alpaca-trader -- $PAPER_FLAG
