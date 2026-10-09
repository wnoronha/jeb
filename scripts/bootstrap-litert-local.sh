#!/usr/bin/env bash
# Optional helper: install LiteRT-LM, import EmbeddingGemma 2 text bundle, serve or verify.
# Not part of the jeb binary — see docs/LITERT_LOCAL.md.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LITERT_VERSION="${LITERT_VERSION:-0.18.0}"
MODEL_REPO="${LITERT_MODEL_REPO:-litert-community/embeddinggemma-2-text-270m-litert-lm}"
HOST="${LITERT_HOST:-127.0.0.1}"
PORT="${LITERT_PORT:-9379}"
EMBED_URL="http://${HOST}:${PORT}"
PROBE_MODEL="${LITERT_MODEL:-embedding-gemma-v2-text.litertlm,cpu}"

usage() {
  cat <<EOF
Usage: $(basename "$0") <command>

Commands:
  setup     Install litert-lm (${LITERT_VERSION}) and import ${MODEL_REPO}
  install   Install litert-lm into active venv, repo .venv, or user site
  import    Import the EmbeddingGemma 2 text bundle (run once)
  list      Show registered models (copy ID for LITERT_MODEL / --litert-model)
  serve     Start \`litert-lm serve\` on ${HOST}:${PORT} (foreground)
  verify    POST a test embedding to ${EMBED_URL}

Environment:
  LITERT_VERSION, LITERT_MODEL_REPO, LITERT_HOST, LITERT_PORT, LITERT_MODEL
  VIRTUAL_ENV — if set, installs into that environment

Example:
  $(basename "$0") setup
  $(basename "$0") serve    # in another terminal
  export JEB_BACKEND=local
  jeb --state "..." --noul "ok:Is this OK?" -d
EOF
}

python_for_pip() {
  if [[ -n "${VIRTUAL_ENV:-}" ]]; then
    echo "python3"
  elif [[ -x "${ROOT}/.venv/bin/python" ]]; then
    echo "${ROOT}/.venv/bin/python"
  elif command -v uv >/dev/null 2>&1; then
    if [[ ! -x "${ROOT}/.venv/bin/python" ]]; then
      (cd "$ROOT" && uv venv .venv)
    fi
    echo "${ROOT}/.venv/bin/python"
  else
    echo "python3"
  fi
}

run_pip_install() {
  if ! command -v python3 >/dev/null 2>&1 && [[ ! -x "${ROOT}/.venv/bin/python" ]]; then
    echo "error: python3 not found (needed for litert-lm)" >&2
    exit 1
  fi
  local py
  py=$(python_for_pip)
  if command -v uv >/dev/null 2>&1 && [[ "$py" == "${ROOT}/.venv/bin/python" ]]; then
    (cd "$ROOT" && uv pip install "litert-lm==${LITERT_VERSION}")
  else
    "$py" -m pip install --upgrade "litert-lm==${LITERT_VERSION}"
  fi
  echo "Installed litert-lm==${LITERT_VERSION}"
}

litert_bin() {
  if [[ -n "${VIRTUAL_ENV:-}" ]] && [[ -x "${VIRTUAL_ENV}/bin/litert-lm" ]]; then
    echo "${VIRTUAL_ENV}/bin/litert-lm"
  elif [[ -x "${ROOT}/.venv/bin/litert-lm" ]]; then
    echo "${ROOT}/.venv/bin/litert-lm"
  elif command -v litert-lm >/dev/null 2>&1; then
    echo "litert-lm"
  else
    echo "error: litert-lm not on PATH; run: $(basename "$0") install" >&2
    exit 1
  fi
}

cmd_install() {
  run_pip_install
}

cmd_import() {
  local lm
  lm=$(litert_bin)
  "$lm" import "$MODEL_REPO"
  echo "Imported ${MODEL_REPO}. Run '$lm list' and set LITERT_MODEL if needed."
}

cmd_list() {
  "$(litert_bin)" list
}

cmd_serve() {
  local lm
  lm=$(litert_bin)
  exec "$lm" serve --host "$HOST" --port "$PORT"
}

cmd_verify() {
  local payload
  payload=$(printf '{"model":"%s","input":"task: classification | text: hello"}' "$PROBE_MODEL")
  if ! curl -sf -X POST "${EMBED_URL}/v1/embeddings" \
    -H "Content-Type: application/json" \
    -d "$payload" >/dev/null; then
    echo "error: embeddings request failed. Is '$(basename "$0") serve' running?" >&2
    echo "  URL: ${EMBED_URL}/v1/embeddings  model: ${PROBE_MODEL}" >&2
    exit 1
  fi
  echo "OK — LiteRT embeddings responded at ${EMBED_URL}"
}

cmd_setup() {
  cmd_install
  cmd_import
  cmd_list
  echo ""
  echo "Next: run '$(basename "$0") serve' in a separate terminal, then use jeb with JEB_BACKEND=local"
}

main() {
  local cmd="${1:-}"
  case "$cmd" in
    setup) cmd_setup ;;
    install) cmd_install ;;
    import) cmd_import ;;
    list) cmd_list ;;
    serve) cmd_serve ;;
    verify) cmd_verify ;;
    -h | --help | help | "") usage ;;
    *)
      echo "unknown command: $cmd" >&2
      usage >&2
      exit 1
      ;;
  esac
}

main "$@"
