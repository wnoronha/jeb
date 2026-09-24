# Use Cases Guide

`jeb` makes quick decisions across different domains and workflows. Every use case can be run either by passing **JSON** (via stdin, file, or argument) or by using **CLI flags**.

---

## 1. AI Agents (Autonomous Tool & Action Routing)

Agents evaluate next tool actions, detect risk, and determine if human intervention is required before taking irreversible actions.

#### Option A: Raw JSON (Stdin Pipe)
```bash
echo '{
  "state": "User asks to cancel their annual subscription and request a pro-rated refund after 45 days.",
  "questions": {
    "action": {
      "type": "choice",
      "instructions": "Determine the required agent action according to standard refund policy (30-day window).",
      "criteria": {
        "auto_refund": "Issue full or pro-rated refund automatically",
        "deny_and_explain": "Politely deny refund citing 30-day policy and offer subscription pause",
        "escalate_support": "Route to human billing specialist"
      }
    },
    "needs_human_review": {
      "type": "noul",
      "instructions": "Does this request require human approval before responding?"
    }
  }
}' | jeb -d
```

#### Option B: CLI Flags
```bash
jeb \
  --state "User asks to cancel their annual subscription and request a pro-rated refund after 45 days." \
  --choice "action:Determine the required agent action according to standard refund policy (30-day window)." \
  --option "auto_refund=Issue full or pro-rated refund automatically" \
  --option "deny_and_explain=Politely deny refund citing 30-day policy and offer subscription pause" \
  --option "escalate_support=Route to human billing specialist" \
  --noul "needs_human_review:Does this request require human approval before responding?" \
  -d
```

---

## 2. Software Engineers & DevOps (Incident Triage & Deployment)

Embed into deployment automation scripts or on-call runbooks to triage active incidents, determine whether to roll back, and classify severity.

#### Option A: Raw JSON
```bash
echo '{
  "state": "Production API error rate spiked to 8.4% after v3.12 deployment; canary instances show DB connection timeouts.",
  "questions": {
    "triage_action": {
      "type": "choice",
      "instructions": "What is the immediate mitigation action?",
      "criteria": {
        "rollback": "Trigger automated deployment rollback to v3.11",
        "restart": "Restart API container pods across cluster",
        "scale_db": "Increase PostgreSQL connection pool size"
      }
    },
    "critical_sev": {
      "type": "noul",
      "instructions": "Is this incident Sev-1 / outage level?"
    },
    "risk": {
      "type": "score",
      "instructions": "Rate the operational risk of waiting 10 minutes",
      "criteria": ["Low", "Moderate", "High", "Extreme"]
    }
  }
}' | jeb -d
```

#### Option B: CLI Flags
```bash
jeb \
  --state "Production API error rate spiked to 8.4% after v3.12 deployment; canary instances show DB connection timeouts." \
  --choice "triage_action:What is the immediate mitigation action?" \
  --option "rollback=Trigger automated deployment rollback to v3.11" \
  --option "restart=Restart API container pods across cluster" \
  --option "scale_db=Increase PostgreSQL connection pool size" \
  --noul "critical_sev:Is this incident Sev-1 / outage level?" \
  --score "risk:Rate the operational risk of waiting 10 minutes" \
  --level "Low,Moderate,High,Extreme" \
  -d
```

---

## 3. Data Engineers (ETL Pipeline Anomaly Handling)

Integrate decision logic in Apache Airflow, Dagster, or Prefect pipelines when upstream data drift or schema anomalies occur.

#### Option A: Raw JSON
```bash
echo '{
  "state": "Nightly customer metrics batch: 12% of rows have null country codes (historical baseline is 0.5%).",
  "questions": {
    "pipeline_action": {
      "type": "choice",
      "instructions": "Select pipeline behavior for upstream schema/data deviation.",
      "criteria": {
        "fail_job": "Abort ETL task and alert data engineering on-call",
        "quarantine": "Write anomalous rows to quarantine table and proceed with valid records",
        "impute_default": "Impute country code as UNKNOWN and continue batch"
      }
    },
    "data_integrity_risk": {
      "type": "score",
      "instructions": "Rate potential downstream reporting risk.",
      "criteria": ["Negligible", "Moderate", "Severe"]
    }
  }
}' | jeb --pretty
```

#### Option B: CLI Flags
```bash
jeb \
  --state "Nightly customer metrics batch: 12% of rows have null country codes (historical baseline is 0.5%)." \
  --choice "pipeline_action:Select pipeline behavior for upstream schema/data deviation." \
  --option "fail_job=Abort ETL task and alert data engineering on-call" \
  --option "quarantine=Write anomalous rows to quarantine table and proceed with valid records" \
  --option "impute_default=Impute country code as UNKNOWN and continue batch" \
  --score "data_integrity_risk:Rate potential downstream reporting risk." \
  --level "Negligible,Moderate,Severe" \
  --pretty
```

---

## 4. Finance & Accounting (Expense & Invoice Compliance)

Automate accounts payable classification to detect policy deviations, approval thresholds, and procurement audit routing.

#### Option A: Raw JSON
```bash
echo '{
  "state": "Vendor invoice from ACME Cloud for $14,250. Previous 3-month average was $8,100 without prior change order on file.",
  "questions": {
    "approval_route": {
      "type": "choice",
      "instructions": "Determine compliance routing for this AP invoice.",
      "criteria": {
        "auto_approve": "Approve invoice under standard operating limits",
        "request_po": "Hold payment and request retroactive purchase order from engineering lead",
        "audit_review": "Flag invoice for internal procurement audit"
      }
    },
    "policy_violation": {
      "type": "noul",
      "instructions": "Does this invoice violate variance threshold policy (>50% variance without PO)?"
    }
  }
}' | jeb -d
```

#### Option B: CLI Flags
```bash
jeb \
  --state "Vendor invoice from ACME Cloud for $14,250. Previous 3-month average was $8,100 without prior change order on file." \
  --choice "approval_route:Determine compliance routing for this AP invoice." \
  --option "auto_approve=Approve invoice under standard operating limits" \
  --option "request_po=Hold payment and request retroactive purchase order from engineering lead" \
  --option "audit_review=Flag invoice for internal procurement audit" \
  --noul "policy_violation:Does this invoice violate variance threshold policy (>50% variance without PO)?" \
  -d
```
