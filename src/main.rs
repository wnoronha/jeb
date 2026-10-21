//! `jeb` — CLI tool to make quick decisions with TypeSafe AI's System One API.

mod client;
mod types;

use anyhow::{bail, Context, Result};
use clap::{ArgAction, Parser};
use client::{fetch_openrouter_models, DecisionsClient};
use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use types::{Answer, NoulCriteria, Provider, Question, SystemOneRequest, SystemOneResponse};

/// Probability at or above which a noul answer maps to yes.
const NOUL_YES_THRESHOLD: f64 = 0.5;

/// CLI argument parser configuration for `jeb`.
#[derive(Parser, Debug)]
#[command(
    name = "jeb",
    about = "CLI tool to make quick decisions with TypeSafe AI's System One API",
    version
)]
struct Cli {
    /// Positional JSON payload string or "-" to read from stdin
    #[arg(value_name = "JSON_OR_DASH")]
    input: Option<String>,

    /// Read request JSON from a file
    #[arg(short, long, value_name = "FILE")]
    file: Option<PathBuf>,

    /// Decision API provider
    #[arg(long, value_enum, env = "JEB_PROVIDER", default_value_t = Provider::Typesafe)]
    provider: Provider,

    /// Model to use for evaluation.
    /// Env: TYPESAFE_MODEL with --provider typesafe, OPENROUTER_MODEL with --provider openrouter.
    #[arg(short, long)]
    model: Option<String>,

    /// API key for the selected provider.
    /// Env: TYPESAFE_API_KEY with --provider typesafe, OPENROUTER_API_KEY with --provider openrouter.
    #[arg(short = 'k', long)]
    api_key: Option<String>,

    /// API base URL.
    /// Env: TYPESAFE_BASE_URL with --provider typesafe, OPENROUTER_BASE_URL with --provider openrouter.
    #[arg(long)]
    base_url: Option<String>,

    /// Request timeout in seconds
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Pretty print JSON output
    #[arg(short, long, default_value_t = false)]
    pretty: bool,

    /// Output only the top decision / judgment summary
    #[arg(short = 'd', long = "decision-only", default_value_t = false)]
    decision_only: bool,

    // Quick CLI builder flags (alternative to passing raw JSON)
    /// Direct state text or JSON string for quick query mode
    #[arg(long, value_name = "TEXT_OR_JSON")]
    state: Option<String>,

    /// Add a choice question: ID and instructions (e.g. --choice "next_step:What should be done?")
    #[arg(long, value_name = "ID:INSTRUCTIONS")]
    choice: Option<String>,

    /// Option for choice question in key=description format (repeatable, e.g. --option "test=Run tests")
    #[arg(long = "option", action = ArgAction::Append)]
    options: Vec<String>,

    /// Add a noul (yes/no) question: ID and instructions (e.g. --noul "is_safe:Is this action safe to execute?")
    #[arg(long, value_name = "ID:INSTRUCTIONS")]
    noul: Option<String>,

    /// Description for the yes outcome of a noul question
    #[arg(long, value_name = "DESC")]
    noul_true: Option<String>,

    /// Description for the no outcome of a noul question
    #[arg(long, value_name = "DESC")]
    noul_false: Option<String>,

    /// Add a score question: ID and instructions (e.g. --score "urgency:Rate urgency from 0 to 2")
    #[arg(long, value_name = "ID:INSTRUCTIONS")]
    score: Option<String>,

    /// Level descriptions for score question (repeatable or comma-separated)
    #[arg(long = "level", action = ArgAction::Append)]
    levels: Vec<String>,

    /// List available decision models and exit
    #[arg(long, default_value_t = false)]
    list_models: bool,
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();
    let provider = cli.provider;

    if cli.list_models {
        if provider != Provider::Openrouter {
            bail!("Model listing is only supported with --provider openrouter.");
        }
        let base_url = resolve_setting(
            cli.base_url.clone(),
            provider.base_url_env(),
            provider.default_base_url(),
        );
        let api_key = resolve_api_key(cli.api_key.clone(), provider.api_key_env());
        let models = fetch_openrouter_models(&base_url, api_key.as_deref(), cli.timeout)?;
        if cli.pretty {
            println!("{}", serde_json::to_string_pretty(&models)?);
        } else {
            println!("{}", serde_json::to_string(&models)?);
        }
        return Ok(());
    }

    let request = parse_request(&cli, provider)?;

    let api_key =
        resolve_api_key(cli.api_key.clone(), provider.api_key_env()).ok_or_else(|| {
            anyhow::anyhow!(
                "Missing API key. Set {} environment variable or pass --api-key.",
                provider.api_key_env()
            )
        })?;

    let base_url = resolve_setting(
        cli.base_url.clone(),
        provider.base_url_env(),
        provider.default_base_url(),
    );
    let client = DecisionsClient::new(api_key, Some(base_url), cli.timeout, provider)?;
    let response = client.evaluate(&request)?;

    if cli.decision_only {
        print_decision_summary(&response, cli.pretty)?;
    } else if cli.pretty {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", serde_json::to_string(&response)?);
    }

    Ok(())
}

/// Resolves a setting from the explicit flag value, then the provider
/// env fallback, then the provider default. Blank values count as unset.
fn resolve_setting(explicit: Option<String>, fallback_env: &str, default: &str) -> String {
    if let Some(value) = explicit {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if let Ok(value) = std::env::var(fallback_env) {
        if !value.trim().is_empty() {
            return value;
        }
    }
    default.to_string()
}

/// Resolves an API key from the explicit flag value or the provider env fallback.
fn resolve_api_key(explicit: Option<String>, fallback_env: &str) -> Option<String> {
    if let Some(value) = explicit {
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    std::env::var(fallback_env)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// Parses CLI inputs into a validated `SystemOneRequest`.
fn parse_request(cli: &Cli, provider: Provider) -> Result<SystemOneRequest> {
    let model = resolve_setting(
        cli.model.clone(),
        provider.model_env(),
        provider.default_model(),
    );
    // 1. If quick CLI flags are provided, construct request from flags
    if let Some(state_raw) = &cli.state {
        let state_val = serde_json::from_str::<serde_json::Value>(state_raw)
            .unwrap_or_else(|_| serde_json::Value::String(state_raw.clone()));

        let mut questions = BTreeMap::new();

        if let Some(choice_spec) = &cli.choice {
            let (id, instructions) = split_id_and_instructions(choice_spec, "choice")?;
            let mut criteria = BTreeMap::new();
            for opt in &cli.options {
                if let Some((k, v)) = opt.split_once('=') {
                    let key = k.trim().to_string();
                    if key.is_empty() {
                        bail!("Option key must not be empty in '--option \"{}\"'", opt);
                    }
                    if criteria.contains_key(&key) {
                        bail!("Duplicate option key '{}'", key);
                    }
                    criteria.insert(key, serde_json::Value::String(v.trim().to_string()));
                } else {
                    let key = opt.trim().to_string();
                    if key.is_empty() {
                        bail!("Option key must not be empty");
                    }
                    if criteria.contains_key(&key) {
                        bail!("Duplicate option key '{}'", key);
                    }
                    criteria.insert(key, serde_json::Value::Null);
                }
            }
            if criteria.is_empty() {
                bail!("Choice question requires at least one --option key=description");
            }
            questions.insert(
                id,
                Question::Choice {
                    instructions: serde_json::Value::String(instructions),
                    criteria,
                },
            );
        }

        if let Some(noul_spec) = &cli.noul {
            let (id, instructions) = split_id_and_instructions(noul_spec, "noul")?;
            let criteria = match (&cli.noul_true, &cli.noul_false) {
                (None, None) => None,
                (yes, no) => Some(NoulCriteria {
                    r#true: yes
                        .as_ref()
                        .map(|text| serde_json::Value::String(text.trim().to_string())),
                    r#false: no
                        .as_ref()
                        .map(|text| serde_json::Value::String(text.trim().to_string())),
                }),
            };
            questions.insert(
                id,
                Question::Noul {
                    instructions: serde_json::Value::String(instructions),
                    criteria,
                },
            );
        }

        if let Some(score_spec) = &cli.score {
            let (id, instructions) = split_id_and_instructions(score_spec, "score")?;
            let mut criteria = Vec::new();
            for lvl in &cli.levels {
                for part in lvl.split(',') {
                    let trimmed = part.trim();
                    if !trimmed.is_empty() {
                        criteria.push(serde_json::Value::String(trimmed.to_string()));
                    }
                }
            }
            if criteria.len() < 2 {
                bail!("Score question requires at least 2 levels via --level");
            }
            questions.insert(
                id,
                Question::Score {
                    instructions: serde_json::Value::String(instructions),
                    criteria,
                },
            );
        }

        if questions.is_empty() {
            bail!("Quick mode with --state requires at least one question flag (--choice, --noul, or --score)");
        }

        return Ok(SystemOneRequest {
            model,
            state: state_val,
            questions,
        });
    }

    // 2. If file is passed
    if let Some(path) = &cli.file {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read JSON file from {}", path.display()))?;
        return parse_json_payload(&content, &model);
    }

    // 3. If input arg is passed
    if let Some(input) = &cli.input {
        if input == "-" {
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .context("Failed to read from stdin")?;
            return parse_json_payload(&buffer, &model);
        } else {
            return parse_json_payload(input, &model);
        }
    }

    // 4. If stdin is piped
    if !io::stdin().is_terminal() {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        if !buffer.trim().is_empty() {
            return parse_json_payload(&buffer, &model);
        }
    }

    bail!("No input provided. Provide JSON as an argument, pipe through stdin, use --file, or specify --state and question flags.");
}

/// Parses raw JSON string and sets fallback model if omitted.
fn parse_json_payload(raw: &str, default_model: &str) -> Result<SystemOneRequest> {
    let mut val: serde_json::Value =
        serde_json::from_str(raw).context("Invalid JSON payload passed to jeb")?;

    let obj = val.as_object_mut().ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid payload: top-level JSON must be an object with state and questions"
        )
    })?;
    if !obj.contains_key("model") {
        obj.insert(
            "model".to_string(),
            serde_json::Value::String(default_model.to_string()),
        );
    }

    let req: SystemOneRequest = serde_json::from_value(serde_json::Value::Object(obj.clone()))
        .context("Failed to deserialize payload into SystemOneRequest format")?;
    Ok(req)
}

/// Splits flag input of format `id:instructions` into constituent parts.
fn split_id_and_instructions(spec: &str, qtype: &str) -> Result<(String, String)> {
    if let Some((id, inst)) = spec.split_once(':') {
        let id = id.trim().to_string();
        let inst = inst.trim().to_string();
        if id.is_empty() {
            bail!(
                "Invalid format for --{}. Question id must not be empty in '{}'",
                qtype,
                spec
            );
        }
        if inst.is_empty() {
            bail!(
                "Invalid format for --{}. Instructions must not be empty in '{}'",
                qtype,
                spec
            );
        }
        Ok((id, inst))
    } else {
        bail!(
            "Invalid format for --{}. Expected 'question_id:Instructions text', got '{}'",
            qtype,
            spec
        );
    }
}

/// Formats and outputs the top decision summary.
///
/// Maps a noul probability at or above 0.5 to yes.
/// Maps a lower probability to no.
fn print_decision_summary(response: &SystemOneResponse, pretty: bool) -> Result<()> {
    let out = build_decision_value(response);

    if pretty {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("{}", serde_json::to_string(&out)?);
    }

    Ok(())
}

/// Builds the decision-only JSON value for a response.
fn build_decision_value(response: &SystemOneResponse) -> serde_json::Value {
    let mut decisions = BTreeMap::new();

    for (k, ans) in &response.answers {
        match ans {
            Answer::Choice {
                choice,
                confidence,
                probabilities,
            } => {
                decisions.insert(
                    k,
                    serde_json::json!({
                        "type": "choice",
                        "decision": choice,
                        "confidence": confidence,
                        "top_probability": probabilities.get(choice).unwrap_or(&0.0)
                    }),
                );
            }
            Answer::Noul { noul } => {
                let decision = if *noul >= NOUL_YES_THRESHOLD {
                    "yes"
                } else {
                    "no"
                };
                decisions.insert(
                    k,
                    serde_json::json!({
                        "type": "noul",
                        "decision": decision,
                        "probability": noul
                    }),
                );
            }
            Answer::Score {
                score,
                legend,
                probabilities,
                confidence,
            } => {
                decisions.insert(
                    k,
                    serde_json::json!({
                        "type": "score",
                        "score": score,
                        "legend": legend,
                        "probabilities": probabilities,
                        "confidence": confidence
                    }),
                );
            }
        }
    }

    serde_json::json!({
        "model": response.model,
        "decisions": decisions,
        "usage": response.usage
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_payload_full() {
        let json = r#"{
            "state": "Help! My payouts have been failing for 3 days.",
            "questions": {
                "is_urgent": {
                    "type": "noul",
                    "instructions": "Does this convey urgency?"
                },
                "department": {
                    "type": "choice",
                    "instructions": "Which team should handle this?",
                    "criteria": {
                        "billing": "Payments, invoicing, refunds",
                        "technical": "Bugs, outages, integrations"
                    }
                },
                "frustration": {
                    "type": "score",
                    "instructions": "How frustrated is the customer?",
                    "criteria": ["Calm", "Frustrated", "Very angry"]
                }
            }
        }"#;

        let req = parse_json_payload(json, "jev-latest").unwrap();
        assert_eq!(req.model, "jev-latest");
        assert_eq!(req.questions.len(), 3);
        assert!(matches!(
            req.questions.get("is_urgent").unwrap(),
            Question::Noul { .. }
        ));
        assert!(matches!(
            req.questions.get("department").unwrap(),
            Question::Choice { .. }
        ));
        assert!(matches!(
            req.questions.get("frustration").unwrap(),
            Question::Score { .. }
        ));
    }

    #[test]
    fn test_parse_quick_flags() {
        let cli = Cli {
            input: None,
            file: None,
            model: Some("jev-latest".to_string()),
            api_key: Some("dummy-key".to_string()),
            base_url: Some("https://api.typesafe.ai".to_string()),
            timeout: 30,
            pretty: false,
            decision_only: false,
            state: Some("Customer says: Please reset my password".to_string()),
            choice: Some("intent:What is the user requesting?".to_string()),
            options: vec![
                "auth=Password and authentication".to_string(),
                "billing=Billing questions".to_string(),
            ],
            noul: Some("is_clear:Is the request clear?".to_string()),
            noul_true: None,
            noul_false: None,
            provider: Provider::Typesafe,
            list_models: false,
            score: None,
            levels: vec![],
        };

        let req = parse_request(&cli, Provider::Typesafe).unwrap();
        assert_eq!(req.questions.len(), 2);
        assert!(req.questions.contains_key("intent"));
        assert!(req.questions.contains_key("is_clear"));
    }

    #[test]
    fn test_response_deserialization() {
        let resp_json = r#"{
            "model": "jev-1.13.0",
            "answers": {
                "department": {
                    "type": "choice",
                    "choice": "billing",
                    "probabilities": { "billing": 0.88, "technical": 0.12 },
                    "confidence": 0.81
                },
                "is_urgent": {
                    "type": "noul",
                    "noul": 0.95
                }
            },
            "usage": { "input_tokens": 318, "output_tokens": 34 }
        }"#;

        let resp: SystemOneResponse = serde_json::from_str(resp_json).unwrap();
        assert_eq!(resp.model, "jev-1.13.0");
        assert_eq!(resp.answers.len(), 2);
    }

    #[test]
    fn test_split_id_rejects_empty_id() {
        assert!(split_id_and_instructions(":Do this", "choice").is_err());
    }

    #[test]
    fn test_split_id_rejects_empty_instructions() {
        assert!(split_id_and_instructions("task:", "choice").is_err());
    }

    #[test]
    fn test_split_id_rejects_missing_separator() {
        assert!(split_id_and_instructions("task", "choice").is_err());
    }

    #[test]
    fn test_resolve_setting_prefers_explicit() {
        assert_eq!(
            resolve_setting(
                Some("explicit".to_string()),
                "JEB_TEST_UNSET_7XQ2",
                "fallback-default"
            ),
            "explicit"
        );
    }

    #[test]
    fn test_resolve_setting_uses_default_when_unset() {
        assert_eq!(
            resolve_setting(None, "JEB_TEST_UNSET_7XQ2", "fallback-default"),
            "fallback-default"
        );
        assert_eq!(
            resolve_setting(
                Some("  ".to_string()),
                "JEB_TEST_UNSET_7XQ2",
                "fallback-default"
            ),
            "fallback-default"
        );
    }

    #[test]
    fn test_resolve_api_key_none_when_unset() {
        assert_eq!(resolve_api_key(None, "JEB_TEST_UNSET_7XQ2"), None);
        assert_eq!(
            resolve_api_key(Some("k".to_string()), "JEB_TEST_UNSET_7XQ2"),
            Some("k".to_string())
        );
    }

    #[test]
    fn test_quick_choice_rejects_empty_option_key() {
        let cli = Cli {
            input: None,
            file: None,
            model: Some("jev-latest".to_string()),
            api_key: None,
            base_url: Some("https://api.typesafe.ai".to_string()),
            timeout: 30,
            pretty: false,
            decision_only: false,
            state: Some("state".to_string()),
            choice: Some("task:Do this".to_string()),
            options: vec!["=desc".to_string()],
            noul: None,
            noul_true: None,
            noul_false: None,
            provider: Provider::Typesafe,
            list_models: false,
            score: None,
            levels: vec![],
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_choice_rejects_duplicate_option_key() {
        let cli = Cli {
            input: None,
            file: None,
            model: Some("jev-latest".to_string()),
            api_key: None,
            base_url: Some("https://api.typesafe.ai".to_string()),
            timeout: 30,
            pretty: false,
            decision_only: false,
            state: Some("state".to_string()),
            choice: Some("task:Do this".to_string()),
            options: vec!["a=First".to_string(), "a=Second".to_string()],
            noul: None,
            noul_true: None,
            noul_false: None,
            provider: Provider::Typesafe,
            list_models: false,
            score: None,
            levels: vec![],
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_parse_json_payload_rejects_non_object() {
        assert!(parse_json_payload("[1, 2, 3]", "jev-latest").is_err());
    }

    #[test]
    fn test_noul_criteria_flags() {
        let cli = Cli {
            input: None,
            file: None,
            model: Some("jev-latest".to_string()),
            api_key: None,
            base_url: Some("https://api.typesafe.ai".to_string()),
            timeout: 30,
            pretty: false,
            decision_only: false,
            state: Some("state".to_string()),
            choice: None,
            options: vec![],
            noul: Some("safe:Is it safe?".to_string()),
            noul_true: Some("Safe to proceed".to_string()),
            noul_false: Some("Unsafe".to_string()),
            provider: Provider::Typesafe,
            list_models: false,
            score: None,
            levels: vec![],
        };
        let req = parse_request(&cli, Provider::Typesafe).unwrap();
        match req.questions.get("safe").unwrap() {
            Question::Noul { criteria, .. } => {
                assert!(criteria.is_some());
            }
            _ => panic!("Expected noul question"),
        }
    }

    #[test]
    fn test_build_decision_value_includes_score_legend() {
        let resp_json = r#"{
            "model": "jev-1.13.0",
            "answers": {
                "risk": {
                    "type": "score",
                    "score": 2.0,
                    "legend": {"0": "Low", "2": "High"},
                    "probabilities": {"0": 0.1, "2": 0.9},
                    "confidence": 0.9
                },
                "ok": {"type": "noul", "noul": 0.8}
            },
            "usage": {"input_tokens": 10, "output_tokens": 5}
        }"#;
        let resp: SystemOneResponse = serde_json::from_str(resp_json).unwrap();
        let value = build_decision_value(&resp);
        let risk = &value["decisions"]["risk"];
        assert_eq!(risk["score"], 2.0);
        assert_eq!(risk["legend"]["2"], "High");
        assert_eq!(risk["probabilities"]["2"], 0.9);
        assert_eq!(value["decisions"]["ok"]["decision"], "yes");
    }

    #[test]
    fn test_usage_accepts_large_values() {
        let resp_json = r#"{
            "model": "jev-1.13.0",
            "answers": {},
            "usage": {"input_tokens": 5000000000, "output_tokens": 7}
        }"#;
        let resp: SystemOneResponse = serde_json::from_str(resp_json).unwrap();
        assert_eq!(resp.usage.input_tokens, 5_000_000_000);
    }

    fn assert_live_fixture(
        req_json: &str,
        resp_json: &str,
        choice_id: &str,
        noul_id: &str,
        score_id: &str,
        expected_model: &str,
    ) {
        let req: SystemOneRequest = serde_json::from_str(req_json).unwrap();
        assert!(matches!(
            req.questions.get(choice_id).unwrap(),
            Question::Choice { .. }
        ));
        assert!(matches!(
            req.questions.get(noul_id).unwrap(),
            Question::Noul { .. }
        ));
        assert!(matches!(
            req.questions.get(score_id).unwrap(),
            Question::Score { .. }
        ));

        let resp: SystemOneResponse = serde_json::from_str(resp_json).unwrap();
        assert_eq!(resp.model, expected_model);
        assert!(matches!(
            resp.answers.get(choice_id).unwrap(),
            Answer::Choice { .. }
        ));
        assert!(matches!(
            resp.answers.get(noul_id).unwrap(),
            Answer::Noul { .. }
        ));
        assert!(matches!(
            resp.answers.get(score_id).unwrap(),
            Answer::Score { .. }
        ));
        let summary = build_decision_value(&resp);
        assert_eq!(summary["decisions"][choice_id]["type"], "choice");
        assert_eq!(summary["decisions"][noul_id]["type"], "noul");
        assert_eq!(summary["decisions"][score_id]["type"], "score");
    }

    #[test]
    fn test_live_fixture_devops() {
        assert_live_fixture(
            include_str!("../fixtures/req_1_devops.json"),
            include_str!("../fixtures/resp_1_devops.json"),
            "triage_action",
            "critical_sev",
            "risk",
            "jev-1.13.0",
        );
    }

    #[test]
    fn test_live_fixture_billing() {
        assert_live_fixture(
            include_str!("../fixtures/req_2_billing.json"),
            include_str!("../fixtures/resp_2_billing.json"),
            "approval_route",
            "policy_violation",
            "review_priority",
            "jev-1.13.0",
        );
    }

    #[test]
    fn test_live_fixture_security() {
        assert_live_fixture(
            include_str!("../fixtures/req_3_security.json"),
            include_str!("../fixtures/resp_3_security.json"),
            "exploitability",
            "needs_hotfix",
            "severity",
            "jev-1.13.0",
        );
    }

    #[test]
    fn test_openrouter_fixture_devops() {
        assert_live_fixture(
            include_str!("../fixtures/openrouter/oreq_1_devops.json"),
            include_str!("../fixtures/openrouter/oresp_1_devops.json"),
            "triage_action",
            "critical_sev",
            "risk",
            "typesafe/jev-1.13-20260917",
        );
    }

    #[test]
    fn test_openrouter_fixture_billing() {
        assert_live_fixture(
            include_str!("../fixtures/openrouter/oreq_2_billing.json"),
            include_str!("../fixtures/openrouter/oresp_2_billing.json"),
            "approval_route",
            "policy_violation",
            "review_priority",
            "typesafe/jev-1.13-20260917",
        );
    }

    #[test]
    fn test_openrouter_fixture_security() {
        assert_live_fixture(
            include_str!("../fixtures/openrouter/oreq_3_security.json"),
            include_str!("../fixtures/openrouter/oresp_3_security.json"),
            "exploitability",
            "needs_hotfix",
            "severity",
            "typesafe/jev-1.13-20260917",
        );
    }
}
