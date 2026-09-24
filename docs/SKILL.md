---
name: jeb
description: CLI tool to make quick decisions with TypeSafe AI's System One API (pick options, answer yes/no questions, score risk).
---

# Jeb Skill

`jeb` lets scripts and agents get quick decisions from TypeSafe AI's System One model without the latency or overhead of a full LLM conversation.

---

## When to Use

Use `jeb` when an agent needs to:
- **Choose between discrete action paths** (`choice` question) with calibrated probabilities.
- **Perform quick sanity/safety/policy checks** (`noul` yes/no question) before running destructive or irreversible tools.
- **Score urgency, risk, or priority** (`score` question) on an ordered numerical or categorical scale.
- **Offload reflexive decision-making** without consuming large LLM context and reasoning latency.

---

## Invocation Patterns

### 1. Decision Summary Pipe (Recommended for Agents)
Piping JSON into `jeb -d` returns only the top decision and confidence:

```bash
echo '{
  "state": "User requested database migration rollback after migration 004 failure.",
  "questions": {
    "action": {
      "type": "choice",
      "instructions": "Determine the safest immediate action.",
      "criteria": {
        "rollback": "Execute automated rollback script to migration 003",
        "investigate": "Inspect PostgreSQL error logs before modifying DB state",
        "escalate": "Alert on-call engineer and hold execution"
      }
    },
    "is_critical": {
      "type": "noul",
      "instructions": "Is this incident Sev-1 / critical data integrity risk?"
    }
  }
}' | jeb -d
```

**Output:**
```json
{
  "model": "jev-1.13.0",
  "decisions": {
    "action": {
      "type": "choice",
      "decision": "investigate",
      "confidence": 0.88,
      "top_probability": 0.91
    },
    "is_critical": {
      "type": "noul",
      "decision": "yes",
      "probability": 0.85
    }
  },
  "usage": { "input_tokens": 140, "output_tokens": 24 }
}
```

---

### 2. Fast CLI Flags Mode
For quick shell evaluations without formatting raw JSON:

```bash
jeb \
  --state "Production API latency spike to 4500ms after deploy" \
  --choice "action:Select immediate response" \
  --option "rollback=Trigger canary rollback" \
  --option "scale=Scale out replicas by 50%" \
  --noul "alert_oncall:Should on-call engineer be paged?" \
  -d
```

---

## Question Types Reference

| Question Type | JSON Structure | Output Fields |
| :--- | :--- | :--- |
| **`choice`** | `{"type": "choice", "instructions": "...", "criteria": {"key": "desc"}}` | `choice`, `probabilities`, `confidence` |
| **`noul`** | `{"type": "noul", "instructions": "...", "criteria": {"true": "...", "false": "..."}}` | `noul` (float `0.0` - `1.0`), resolved to `"yes"`/`"no"` with `-d` |
| **`score`** | `{"type": "score", "instructions": "...", "criteria": ["L0", "L1", "L2"]}` | `score` (float), `probabilities`, `confidence` |

---

## Agent Integration Guidelines

1. **State Brevity**: Pass concise, factual state text or structured JSON. Avoid dumping entire conversation histories.
2. **Deterministic Keys**: Use semantic snake_case identifiers for question IDs (e.g., `next_action`, `is_safe`, `severity`).
3. **Threshold Checking**: When evaluating `noul` probabilities, use calibrated thresholds:
   - `probability >= 0.70`: High confidence affirmative.
   - `0.40 < probability < 0.70`: Ambiguous / borderline (consider escalation or human review).
   - `probability <= 0.40`: High confidence negative.
4. **Environment Variables**:
   - Ensure `TYPESAFE_API_KEY` is set in the agent runtime environment.
   - Optional overrides: `TYPESAFE_MODEL` (default: `jev-latest`), `TYPESAFE_BASE_URL` (default: `https://api.typesafe.ai`).
