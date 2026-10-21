# CLI Reference (`jeb`)

`jeb` provides four primary invocation patterns:

---

## 1. Flags & Options

| Flag | Env Var | Default | Description |
| :--- | :--- | :--- | :--- |
| `-k`, `--api-key` | `TYPESAFE_API_KEY` / `OPENROUTER_API_KEY` | *(Required)* | Provider API key |
| `-m`, `--model` | `TYPESAFE_MODEL` / `OPENROUTER_MODEL` | `jev-latest` / `typesafe/jev-1.13-20260917` | Evaluation model |
| `--base-url` | `TYPESAFE_BASE_URL` / `OPENROUTER_BASE_URL` | per-provider default | API endpoint base URL |
| `--timeout` | — | `30` | Request timeout (seconds) |
| `-p`, `--pretty` | — | `false` | Pretty-print JSON output |
| `-d`, `--decision-only` | — | `false` | Output top decisions only |
| `--provider` | `JEB_PROVIDER` | `typesafe` | Decision API provider (`typesafe`, `openrouter`) |
| `--list-models` | — | `false` | List OpenRouter decision models and exit |
| `-f`, `--file` | — | — | Read input JSON from file |
| `--state` | — | — | State text/JSON for quick mode |
| `--choice` | — | — | `ID:Instructions` for choice |
| `--option` | — | — | `key=Description` (repeatable) |
| `--noul` | — | — | `ID:Instructions` for yes/no |
| `--noul-true` | — | — | Description for yes outcome |
| `--noul-false` | — | — | Description for no outcome |
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
