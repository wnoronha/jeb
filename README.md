# jeb

[![CI](https://github.com/typesafe-ai/jeb/actions/workflows/ci.yml/badge.svg)](https://github.com/typesafe-ai/jeb/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

CLI tool to query TypeSafe AI's System One API (Jev model). Designed for scripts, shell pipelines, and agent tool calls where you need fast decisions without invoking a full LLM workflow.

---

## Installation

### From source
```bash
git clone https://github.com/typesafe-ai/jeb.git
cd jeb
cargo install --path .
```

Or build a release binary:
```bash
cargo build --release
# binary is at target/release/jeb
```

---

## Configuration

Set your API key via environment variable:
```bash
export TYPESAFE_API_KEY="your-api-key"
```

Optional settings:
```bash
export TYPESAFE_MODEL="jev-latest"               # default: jev-latest
export TYPESAFE_BASE_URL="https://api.typesafe.ai" # default: https://api.typesafe.ai
```

---

## Usage

### 1. Pipe JSON via stdin
```bash
echo '{
  "state": "User requested database migration rollback after error in migration 004",
  "questions": {
    "action": {
      "type": "choice",
      "instructions": "What is the safest next action?",
      "criteria": {
        "rollback": "Execute automated rollback script to migration 003",
        "investigate": "Inspect PostgreSQL error logs before changing DB state",
        "escalate": "Alert on-call engineer and hold execution"
      }
    },
    "is_critical": {
      "type": "noul",
      "instructions": "Is this situation critical?"
    }
  }
}' | jeb
```

### 2. Pass JSON as argument or file
```bash
# JSON string
jeb '{"state": "...", "questions": {...}}'

# File
jeb --file request.json
```

### 3. Quick CLI flags (no JSON required)
```bash
jeb \
  --state "Deploying v2.4 to production cluster" \
  --choice "next_step:Which playbook should be executed?" \
  --option "deploy=Run ansible deploy.yml" \
  --option "os_setup=Run ansible os-setup.yml" \
  --noul "need_backup:Does this require taking a pre-deployment backup?"
```

### 4. Decision summary only (`-d`)
To get just the top decision per question without the full distribution:
```bash
jeb -d '{"state": "CPU usage at 98% with 500 error spikes", "questions": {"scale": {"type": "noul", "instructions": "Should we scale out replicas?"}}}'
```

Output:
```json
{
  "model": "jev-1.13.0",
  "decisions": {
    "scale": {
      "type": "noul",
      "decision": "yes",
      "probability": 0.94
    }
  },
  "usage": { "input_tokens": 120, "output_tokens": 12 }
}
```

---

## Question Types

| Type | Description | Returns |
| :--- | :--- | :--- |
| **`choice`** | Multiple-choice options | Top choice, probability per option, confidence score |
| **`noul`** | Binary yes/no question | Probability between 0.0 and 1.0 |
| **`score`** | Ordered rating levels | Weighted numerical score, legend, probabilities |

---

## Docs

- [API Reference](docs/API_REFERENCE.md) — Payload schema and response format
- [CLI Reference](docs/CLI_REFERENCE.md) — Available flags and options
- [Use Cases](docs/USE_CASES.md) — Examples for devops, data, finance, and agents
- [Skill Definition](docs/SKILL.md) — Agent tool specification

---

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

---

## License

[MIT](LICENSE)
