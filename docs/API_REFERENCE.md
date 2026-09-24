# API & Payload Reference

`jeb` interacts with the TypeSafe AI System One `/v1/systemone` endpoint.

---

## 1. Request Schema

```json
{
  "model": "jev-latest",
  "state": "<string | object | array>",
  "questions": {
    "<question_id>": {
      "type": "choice | noul | score",
      "instructions": "<string | object>",
      "criteria": "<criteria_payload>"
    }
  }
}
```

### Question Types

| Type | Criteria Format | Example |
| :--- | :--- | :--- |
| **`choice`** | Key-value map (`{"id": "description"}`) | `{"opt1": "Action A", "opt2": "Action B"}` |
| **`noul`** | Optional object (`{"true": "...", "false": "..."}`) | `null` or `{"true": "Proceed", "false": "Halt"}` |
| **`score`** | Array of string levels | `["Low", "Medium", "High", "Critical"]` |

---

## 2. Response Schema

```json
{
  "model": "jev-1.13.0",
  "answers": {
    "<question_id>": {
      "type": "choice",
      "choice": "opt1",
      "probabilities": { "opt1": 0.85, "opt2": 0.15 },
      "confidence": 0.70
    },
    "<noul_id>": {
      "type": "noul",
      "noul": 0.94
    },
    "<score_id>": {
      "type": "score",
      "score": 2.7,
      "legend": { "0": "Low", "1": "Medium", "2": "High", "3": "Critical" },
      "probabilities": { "0": 0.05, "1": 0.10, "2": 0.65, "3": 0.20 },
      "confidence": 0.65
    }
  },
  "usage": {
    "input_tokens": 140,
    "output_tokens": 28
  }
}
```

---

## 3. Decision Summary Format (`-d` / `--decision-only`)

```json
{
  "model": "jev-1.13.0",
  "decisions": {
    "next_action": {
      "type": "choice",
      "decision": "opt1",
      "confidence": 0.70,
      "top_probability": 0.85
    },
    "is_safe": {
      "type": "noul",
      "decision": "yes",
      "probability": 0.94
    }
  },
  "usage": {
    "input_tokens": 140,
    "output_tokens": 28
  }
}
```
