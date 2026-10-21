#!/usr/bin/env bash
# Fetches a live decision response and saves it as a versioned fixture.
# Tests stay offline for CI. Use this script in dev to regenerate fixtures.
# Usage: scripts/fetch_fixture.sh <req.json> <resp.json> [--provider openrouter] [--model ID]
set -euo pipefail
REQ="$1"
RESP="$2"
shift 2
cargo run -q -- "$@" --pretty --file "$REQ" > "$RESP"
echo "saved $RESP"
