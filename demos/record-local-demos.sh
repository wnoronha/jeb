#!/usr/bin/env bash
# Record polished jeb local demo (see record_jeb_local_demo.py).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEMO_DIR="$(cd "$(dirname "$0")" && pwd)"
JEB="${JEB_BIN:-$ROOT/target/release/jeb}"
EMBED_URL="${LITERT_EMBED_URL:-http://127.0.0.1:9380}"
MODEL="${LITERT_MODEL:-embeddinggemma-2-740m.litertlm,cpu}"
CAST="${DEMO_CAST:-$DEMO_DIR/jeb-local.cast}"
TITLE="${DEMO_TITLE:-jeb + EmbeddingGemma 2 local backend (LiteRT-LM)}"

export COLUMNS=105
export LINES=28
export JEB_BIN="$JEB"
export LITERT_EMBED_URL="$EMBED_URL"
export LITERT_MODEL="$MODEL"

if [[ ! -x "$JEB" ]]; then
  echo "Build jeb first: cargo build --release" >&2
  exit 1
fi

curl -sf -X POST "${EMBED_URL%/}/v1/embeddings" \
  -H "Content-Type: application/json" \
  -d "{\"model\":\"$MODEL\",\"input\":\"ping\"}" >/dev/null || {
  echo "LiteRT not reachable at $EMBED_URL - start: .venv/bin/litert-lm serve --port 9380" >&2
  exit 1
}

echo "Recording $CAST ..."
asciinema rec --overwrite --idle-time-limit 1.5 --window-size 105x28 -t "$TITLE" \
  -c "python3 \"$DEMO_DIR/record_jeb_local_demo.py\"" \
  "$CAST"

echo "Play:  asciinema play $CAST"
echo "Upload: asciinema upload $CAST"
