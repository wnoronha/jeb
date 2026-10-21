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

---

## 5. Robotics & Autonomous Systems (ArduPilot Flight Telemetry)

Evaluate real-time MAVLink (`SYS_STATUS`, `GLOBAL_POSITION_INT`, `VIBRATION`, `STATUSTEXT`) or ArduPilot DataFlash (`XKF4`, `GPS`, `RCOU`) telemetry on companion computers for autonomous failsafe and mode transitions.

#### Option A: Raw JSON (Battery Failsafe in Headwind)
```bash
echo '{
  "state": "ArduPilot telemetry: SYS_STATUS mode=GUIDED armed=true vbat=13.82V curr=29.4A rem=18%; GLOBAL_POSITION_INT alt_rel=42.8m dist_home=680m headwind=7.2m/s; STATUSTEXT SEVERITY_CRITICAL: Battery 1 low voltage failsafe triggered (13.82V < 14.00V).",
  "questions": {
    "flight_action": {
      "type": "choice",
      "instructions": "Select the immediate flight mode transition for the companion computer.",
      "criteria": {
        "continue_mission": "Remain in GUIDED mode and proceed to next waypoint",
        "rtl": "Switch to RTL (Return-to-Launch) at current altitude",
        "land_now": "Switch to LAND immediately at current GPS coordinates"
      }
    },
    "abort_mission": {
      "type": "noul",
      "instructions": "Should the autonomous mission be aborted immediately?"
    },
    "airframe_risk": {
      "type": "score",
      "instructions": "Rate the risk of airframe loss if flight continues to the next waypoint.",
      "criteria": ["Low", "Moderate", "High", "Critical"]
    }
  }
}' | jeb -d
```

#### Option B: CLI Flags (EKF3 Lane Switch & Magnetometer Anomaly)
```bash
jeb \
  --state "ArduPilot DataFlash: XKF4 vel_var=0.84 pos_horiz_var=0.92 compass_var=1.18; GPS status=3 num_sats=6 hdop=2.85; MSG: EKF3 IMU0 MAG0 ground mag anomaly, lane switch 0->1 while hovering 15m from steel structure." \
  --choice "ekf_action:Select the safest navigation recovery action after the EKF lane switch." \
  --option "hold_loiter=Switch to LOITER and wait for EKF variance to settle" \
  --option "land_alt_hold=Switch to ALT_HOLD or LAND using barometric altitude" \
  --option "switch_rtl=Trigger RTL using backup EKF lane" \
  --noul "mag_compromised:Is the primary magnetometer heading unreliable for autonomous waypoint navigation?" \
  --score "flyaway_risk:Rate the risk of flyaway or position divergence." \
  --level "Negligible,Moderate,Severe,Extreme" \
  -d
```

#### Option C: CLI Flags (IMU Vibration Clipping & Motor Saturation)
```bash
jeb \
  --state "ArduPilot telemetry: VIBRATION vibe_x=44.2m/s2 vibe_y=48.9m/s2 vibe_z=74.6m/s2 clipping_0=312 clipping_1=245; RCOU C1=1890 C2=1510 C3=1910 C4=1495; STATUSTEXT SEVERITY_WARNING: High vibration and motor 1/3 saturation detected in AUTO survey." \
  --choice "motor_action:Select the immediate flight response to the vibration and motor output imbalance." \
  --option "continue_survey=Continue AUTO survey grid at current speed" \
  --option "reduce_throttle_rtl=Reduce max velocity to 4m/s and command RTL" \
  --option "emergency_land=Execute immediate vertical descent in LAND mode" \
  --noul "clipping_critical:Does accelerometer clipping exceed safe attitude control margins?" \
  --score "hardware_severity:Rate the severity of the propulsion and IMU vibration anomaly." \
  --level "Normal,Elevated,Degraded,Hazardous" \
  -d
```

---

## 6. Model Evaluation & Benchmarking (Multi-Model Decision Matrix)

Compare how different decision models evaluate identical request fixtures (`fixtures/req_*.json`) using cached responses in `fixtures/typesafe/` and `fixtures/openrouter/` (mapped in `tests/fixtures.rs` and normalized with `jeb::build_decision_value`), or expand the matrix with live calls.

#### Option A: Offline Matrix via `jq` (`req_ping` across cached models)
```bash
jq -r '
  [
    .model,
    (if .answers.proceed.noul >= 0.5 then "yes" else "no" end),
    (.answers.proceed.noul | tostring),
    (.usage.input_tokens | tostring)
  ] | @tsv
' fixtures/typesafe/resp_ping.json \
  fixtures/openrouter/oresp_openai.json \
  fixtures/openrouter/oresp_microsoft.json \
  fixtures/openrouter/oresp_cloudflare.json | column -t
```

#### Option B: Offline Scenario Matrix (`req_1` to `req_10` across TypeSafe and OpenRouter Jev variants)
```bash
python3 - <<'PY'
import glob, json, os

rows = [("scenario", "model", "choice", "noul", "score")]
for req_path in sorted(glob.glob("fixtures/req_[0-9]*.json")):
    stem = os.path.basename(req_path).removeprefix("req_")
    for resp_path in (f"fixtures/typesafe/resp_{stem}", f"fixtures/openrouter/oresp_{stem}"):
        with open(resp_path) as f:
            resp = json.load(f)
        choice = noul = score = "-"
        for k, ans in resp["answers"].items():
            if ans["type"] == "choice":
                choice = f"{k}={ans['choice']} ({ans['confidence']:.2f})"
            elif ans["type"] == "noul":
                decision = "yes" if ans["noul"] >= 0.5 else "no"
                noul = f"{k}={decision} ({ans['noul']:.2f})"
            elif ans["type"] == "score":
                label = resp["answers"][k]["legend"].get(str(round(ans["score"])), str(ans["score"]))
                score = f"{k}={label} ({ans['score']:.2f})"
        rows.append((stem.removesuffix(".json"), resp["model"], choice, noul, score))

widths = [max(len(r[i]) for r in rows) for i in range(len(rows[0]))]
for r in rows:
    print("  ".join(val.ljust(widths[i]) for i, val in enumerate(r)))
PY
```

#### Option C: Live Expansion Across Models (`jeb` CLI and `scripts/fetch_fixture.sh`)
```bash
# Run a live decision summary against any OpenRouter decision model
for model in \
  typesafe/jev-1.13-20260917 \
  openai/gpt-6-luna-decisions-20261006 \
  microsoft/microsoft-decision-1-20261009 \
  cloudflare/clef; do
  jeb --provider openrouter --model "$model" --file fixtures/req_ping.json -d
done

# Record a new versioned response fixture for offline tests and evaluation matrices
scripts/fetch_fixture.sh \
  fixtures/req_1_devops.json \
  fixtures/openrouter/oresp_1_devops_openai.json \
  --provider openrouter \
  --model openai/gpt-6-luna-decisions-20261006
```

