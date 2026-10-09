# CLI Reference (`jeb`)

`jeb` is a single binary with no subcommands. LiteRT-LM setup is documented in [LITERT_LOCAL.md](LITERT_LOCAL.md) and optional script `scripts/bootstrap-litert-local.sh`.

`jeb` provides four primary invocation patterns:

---

## 1. Flags & Options

| Flag | Env Var | Default | Description |
| :--- | :--- | :--- | :--- |
| `-k`, `--api-key` | `TYPESAFE_API_KEY` | *(Required for cloud)* | TypeSafe API key |
| `--backend` | `JEB_BACKEND` | `cloud` | `cloud` or `local` (LiteRT-LM embeddings) |
| `--litert-embed-url` | `LITERT_EMBED_URL` | `http://127.0.0.1:9379` | LiteRT `serve` base URL (local only) |
| `--litert-model` | `LITERT_MODEL` | `embedding-gemma-v2-text.litertlm,cpu` | Model id for `/v1/embeddings` |
| `--litert-similarity-temperature` | `LITERT_SIMILARITY_TEMPERATURE` | `0.07` | Softmax temperature over cosine scores |
| `-m`, `--model` | `TYPESAFE_MODEL` | `jev-latest` | Evaluation model |
| `--base-url` | `TYPESAFE_BASE_URL` | `https://api.typesafe.ai` | API endpoint base URL |
| `--timeout` | — | `30` | Request timeout (seconds) |
| `-p`, `--pretty` | — | `false` | Pretty-print JSON output |
| `-d`, `--decision-only` | — | `false` | Output top decisions only |
| `-f`, `--file` | — | — | Read input JSON from file |
| `--state` | — | — | State text/JSON for quick mode |
| `--choice` | — | — | `ID:Instructions` for choice |
| `--option` | — | — | `key=Description` (repeatable) |
| `--noul` | — | — | `ID:Instructions` for yes/no |
| `--score` | — | — | `ID:Instructions` for score |
| `--level` | — | — | Level labels (repeatable/comma) |

---

## 2. Invocation Examples

### Stdin Pipeline
```bash
cat payload.json | jeb -p
```

### Positional Argument
```bash
jeb '{"state": "CPU spike 95%", "questions": {"scale": {"type": "noul", "instructions": "Scale?"}}}'
```

### Quick Mode
```bash
jeb \
  --state "High error rate after deployment" \
  --choice "action:Select resolution" \
  --option "rollback=Roll back immediately" \
  --option "wait=Wait 5 minutes" \
  --decision-only
```
