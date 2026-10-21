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

/// Resolves the model and reports whether the value came from an
/// explicit flag or provider env override as opposed to the default.
fn resolve_model(explicit: Option<String>, provider: Provider) -> (String, bool) {
    if let Some(value) = explicit {
        if !value.trim().is_empty() {
            return (value, true);
        }
    }
    if let Ok(value) = std::env::var(provider.model_env()) {
        if !value.trim().is_empty() {
            return (value, true);
        }
    }
    (provider.default_model().to_string(), false)
}

/// Parses CLI inputs into a validated `SystemOneRequest`.
fn parse_request(cli: &Cli, provider: Provider) -> Result<SystemOneRequest> {
    let (model, model_explicit) = resolve_model(cli.model.clone(), provider);

    let has_question_flag = cli.choice.is_some() || cli.noul.is_some() || cli.score.is_some();
    if cli.state.is_none() {
        if has_question_flag {
            bail!("Question flags (--choice, --noul, --score) require --state");
        }
        if !cli.options.is_empty() {
            bail!("--option requires --choice");
        }
        if !cli.levels.is_empty() {
            bail!("--level requires --score");
        }
        if cli.noul_true.is_some() || cli.noul_false.is_some() {
            bail!("--noul-true and --noul-false require --noul");
        }
    } else {
        if cli.choice.is_none() && !cli.options.is_empty() {
            bail!("--option requires --choice");
        }
        if cli.score.is_none() && !cli.levels.is_empty() {
            bail!("--level requires --score");
        }
        if cli.noul.is_none() && (cli.noul_true.is_some() || cli.noul_false.is_some()) {
            bail!("--noul-true and --noul-false require --noul");
        }
        if cli.file.is_some() || cli.input.is_some() {
            bail!("--state cannot be combined with --file or JSON input");
        }
    }
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
            if questions.contains_key(&id) {
                bail!("Duplicate question id '{}'", id);
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
            if questions.contains_key(&id) {
                bail!("Duplicate question id '{}'", id);
            }
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
            if questions.contains_key(&id) {
                bail!("Duplicate question id '{}'", id);
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

        let req = SystemOneRequest {
            model,
            state: state_val,
            questions,
        };
        validate_request(&req)?;
        return Ok(req);
    }

    // 2. If file is passed
    if let Some(path) = &cli.file {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read JSON file from {}", path.display()))?;
        return parse_json_payload(&content, &model, model_explicit);
    }

    // 3. If input arg is passed
    if let Some(input) = &cli.input {
        if input == "-" {
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .context("Failed to read from stdin")?;
            return parse_json_payload(&buffer, &model, model_explicit);
        } else {
            return parse_json_payload(input, &model, model_explicit);
        }
    }

    // 4. If stdin is piped
    if !io::stdin().is_terminal() {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        if !buffer.trim().is_empty() {
            return parse_json_payload(&buffer, &model, model_explicit);
        }
    }

    bail!("No input provided. Provide JSON as an argument, pipe through stdin, use --file, or specify --state and question flags.");
}

/// Validates structural rules shared by quick-mode and JSON inputs.
fn validate_request(req: &SystemOneRequest) -> Result<()> {
    if req.questions.is_empty() {
        bail!("Payload must contain at least one question");
    }
    for (id, question) in &req.questions {
        match question {
            Question::Choice { criteria, .. } if criteria.is_empty() => {
                bail!(
                    "Choice question '{}' requires at least one criteria entry",
                    id
                );
            }
            Question::Score { criteria, .. } if criteria.len() < 2 => {
                bail!(
                    "Score question '{}' requires at least 2 criteria levels",
                    id
                );
            }
            _ => {}
        }
    }
    Ok(())
}

/// Parses raw JSON string and applies the resolved model.
/// An explicit flag or env model overrides the payload field.
/// Otherwise the payload field applies when present and non-empty.
fn parse_json_payload(
    raw: &str,
    default_model: &str,
    model_explicit: bool,
) -> Result<SystemOneRequest> {
    let mut val: serde_json::Value =
        serde_json::from_str(raw).context("Invalid JSON payload passed to jeb")?;

    {
        let obj = val.as_object_mut().ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid payload: top-level JSON must be an object with state and questions"
            )
        })?;
        match obj.get("model") {
            None => {
                obj.insert(
                    "model".to_string(),
                    serde_json::Value::String(default_model.to_string()),
                );
            }
            Some(serde_json::Value::Null) => {
                bail!("Invalid payload: \"model\" must not be null");
            }
            Some(serde_json::Value::String(text)) if text.trim().is_empty() => {
                bail!("Invalid payload: \"model\" must not be blank");
            }
            _ if model_explicit => {
                obj.insert(
                    "model".to_string(),
                    serde_json::Value::String(default_model.to_string()),
                );
            }
            _ => {}
        }
    }

    let req: SystemOneRequest = serde_json::from_value(val)
        .context("Failed to deserialize payload into SystemOneRequest format")?;
    validate_request(&req)?;
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

/// Resolves a score to its legend label, preferring the rounded value
/// and falling back to the nearest numeric key.
fn score_decision_label(legend: &BTreeMap<String, String>, score: f64) -> String {
    let target = score.round() as i64;
    if let Some(label) = legend.get(&target.to_string()) {
        return label.clone();
    }
    legend
        .iter()
        .filter_map(|(key, label)| key.parse::<i64>().ok().map(|n| (n, label)))
        .min_by_key(|(n, _)| (n - target).abs())
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| score.to_string())
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
                        "decision": score_decision_label(legend, *score),
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

        let req = parse_json_payload(json, "jev-latest", false).unwrap();
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
    fn test_validate_request_rejects_empty_questions() {
        let req = SystemOneRequest {
            model: "jev-latest".to_string(),
            state: serde_json::json!("x"),
            questions: BTreeMap::new(),
        };
        assert!(validate_request(&req).is_err());
    }

    #[test]
    fn test_validate_request_rejects_empty_choice() {
        let mut questions = BTreeMap::new();
        questions.insert(
            "c".to_string(),
            Question::Choice {
                instructions: serde_json::json!("Pick"),
                criteria: BTreeMap::new(),
            },
        );
        let req = SystemOneRequest {
            model: "jev-latest".to_string(),
            state: serde_json::json!("x"),
            questions,
        };
        assert!(validate_request(&req).is_err());
    }

    #[test]
    fn test_validate_request_rejects_short_score() {
        let mut questions = BTreeMap::new();
        questions.insert(
            "s".to_string(),
            Question::Score {
                instructions: serde_json::json!("Rate"),
                criteria: vec![serde_json::json!("Low")],
            },
        );
        let req = SystemOneRequest {
            model: "jev-latest".to_string(),
            state: serde_json::json!("x"),
            questions,
        };
        assert!(validate_request(&req).is_err());
    }

    #[test]
    fn test_parse_json_payload_rejects_empty_questions() {
        let json = r#"{"model": "m", "state": "x", "questions": {}}"#;
        assert!(parse_json_payload(json, "m", false).is_err());
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
    fn test_quick_rejects_duplicate_question_id() {
        let cli = Cli {
            input: None,
            file: None,
            model: None,
            api_key: None,
            base_url: None,
            timeout: 30,
            pretty: false,
            decision_only: false,
            state: Some("state".to_string()),
            choice: Some("task:Do this".to_string()),
            options: vec!["a=First".to_string()],
            noul: Some("task:Is it done?".to_string()),
            noul_true: None,
            noul_false: None,
            score: None,
            levels: vec![],
            provider: Provider::Typesafe,
            list_models: false,
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_rejects_question_flag_without_state() {
        let cli = Cli {
            input: None,
            file: None,
            model: None,
            api_key: None,
            base_url: None,
            timeout: 30,
            pretty: false,
            decision_only: false,
            state: None,
            choice: Some("task:Do this".to_string()),
            options: vec!["a=First".to_string()],
            noul: None,
            noul_true: None,
            noul_false: None,
            score: None,
            levels: vec![],
            provider: Provider::Typesafe,
            list_models: false,
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_rejects_dangling_option() {
        let cli = Cli {
            input: None,
            file: None,
            model: None,
            api_key: None,
            base_url: None,
            timeout: 30,
            pretty: false,
            decision_only: false,
            state: Some("state".to_string()),
            choice: None,
            options: vec!["a=First".to_string()],
            noul: Some("safe:Is it safe?".to_string()),
            noul_true: None,
            noul_false: None,
            score: None,
            levels: vec![],
            provider: Provider::Typesafe,
            list_models: false,
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_rejects_dangling_level() {
        let cli = Cli {
            input: None,
            file: None,
            model: None,
            api_key: None,
            base_url: None,
            timeout: 30,
            pretty: false,
            decision_only: false,
            state: Some("state".to_string()),
            choice: Some("task:Do this".to_string()),
            options: vec!["a=First".to_string()],
            noul: None,
            noul_true: None,
            noul_false: None,
            score: None,
            levels: vec!["High".to_string()],
            provider: Provider::Typesafe,
            list_models: false,
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_rejects_dangling_noul_labels() {
        let cli = Cli {
            input: None,
            file: None,
            model: None,
            api_key: None,
            base_url: None,
            timeout: 30,
            pretty: false,
            decision_only: false,
            state: Some("state".to_string()),
            choice: None,
            options: vec![],
            noul: None,
            noul_true: Some("Yes".to_string()),
            noul_false: None,
            score: Some("risk:Rate it".to_string()),
            levels: vec!["Low".to_string(), "High".to_string()],
            provider: Provider::Typesafe,
            list_models: false,
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_rejects_state_with_file() {
        let cli = Cli {
            input: None,
            file: Some(std::path::PathBuf::from("req.json")),
            model: None,
            api_key: None,
            base_url: None,
            timeout: 30,
            pretty: false,
            decision_only: false,
            state: Some("state".to_string()),
            choice: Some("task:Do this".to_string()),
            options: vec!["a=First".to_string()],
            noul: None,
            noul_true: None,
            noul_false: None,
            score: None,
            levels: vec![],
            provider: Provider::Typesafe,
            list_models: false,
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
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
        assert!(parse_json_payload("[1, 2, 3]", "jev-latest", false).is_err());
    }

    #[test]
    fn test_explicit_model_overrides_payload_model() {
        let json = r#"{"model": "payload-model", "state": "x", "questions": {"q": {"type": "noul", "instructions": "Proceed?"}}}"#;
        let req = parse_json_payload(json, "flag-model", true).unwrap();
        assert_eq!(req.model, "flag-model");
    }

    #[test]
    fn test_payload_model_kept_when_not_explicit() {
        let json = r#"{"model": "payload-model", "state": "x", "questions": {"q": {"type": "noul", "instructions": "Proceed?"}}}"#;
        let req = parse_json_payload(json, "default-model", false).unwrap();
        assert_eq!(req.model, "payload-model");
    }

    #[test]
    fn test_parse_json_payload_rejects_null_model() {
        let json = r#"{"model": null, "state": "x", "questions": {}}"#;
        assert!(parse_json_payload(json, "default-model", false).is_err());
    }

    #[test]
    fn test_parse_json_payload_rejects_blank_model() {
        let json = r#"{"model": "  ", "state": "x", "questions": {}}"#;
        assert!(parse_json_payload(json, "default-model", false).is_err());
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
    fn test_score_decision_label_exact() {
        let legend: BTreeMap<String, String> = [
            ("0".to_string(), "Low".to_string()),
            ("2".to_string(), "High".to_string()),
        ]
        .into_iter()
        .collect();
        assert_eq!(score_decision_label(&legend, 2.0), "High");
    }

    #[test]
    fn test_score_decision_label_rounds_and_falls_back() {
        let legend: BTreeMap<String, String> = [
            ("0".to_string(), "Low".to_string()),
            ("1".to_string(), "Medium".to_string()),
            ("2".to_string(), "High".to_string()),
        ]
        .into_iter()
        .collect();
        assert_eq!(score_decision_label(&legend, 1.6), "High");
        assert_eq!(score_decision_label(&legend, 5.0), "High");
        assert_eq!(score_decision_label(&BTreeMap::new(), 1.0), "1".to_string());
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
            include_str!("../fixtures/typesafe/resp_1_devops.json"),
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
            include_str!("../fixtures/typesafe/resp_2_billing.json"),
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
            include_str!("../fixtures/typesafe/resp_3_security.json"),
            "exploitability",
            "needs_hotfix",
            "severity",
            "jev-1.13.0",
        );
    }

    #[test]
    fn test_openrouter_fixture_devops() {
        assert_live_fixture(
            include_str!("../fixtures/req_1_devops.json"),
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
            include_str!("../fixtures/req_2_billing.json"),
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
            include_str!("../fixtures/req_3_security.json"),
            include_str!("../fixtures/openrouter/oresp_3_security.json"),
            "exploitability",
            "needs_hotfix",
            "severity",
            "typesafe/jev-1.13-20260917",
        );
    }

    fn assert_noul_fixture(req_json: &str, resp_json: &str, id: &str, expected_model: &str) {
        let req: SystemOneRequest = serde_json::from_str(req_json).unwrap();
        assert!(matches!(
            req.questions.get(id).unwrap(),
            Question::Noul { .. }
        ));

        let resp: SystemOneResponse = serde_json::from_str(resp_json).unwrap();
        assert_eq!(resp.model, expected_model);
        match resp.answers.get(id).unwrap() {
            Answer::Noul { noul } => assert!((0.0..=1.0).contains(noul)),
            _ => panic!("Expected noul answer"),
        }
        let summary = build_decision_value(&resp);
        assert_eq!(summary["decisions"][id]["type"], "noul");
    }

    #[test]
    fn test_openrouter_fixture_openai() {
        assert_noul_fixture(
            include_str!("../fixtures/req_ping.json"),
            include_str!("../fixtures/openrouter/oresp_openai.json"),
            "proceed",
            "openai/gpt-6-luna-decisions-20261006",
        );
    }

    #[test]
    fn test_openrouter_fixture_microsoft() {
        assert_noul_fixture(
            include_str!("../fixtures/req_ping.json"),
            include_str!("../fixtures/openrouter/oresp_microsoft.json"),
            "proceed",
            "microsoft/microsoft-decision-1-20261009",
        );
    }

    #[test]
    fn test_openrouter_fixture_cloudflare() {
        assert_noul_fixture(
            include_str!("../fixtures/req_ping.json"),
            include_str!("../fixtures/openrouter/oresp_cloudflare.json"),
            "proceed",
            "cloudflare/clef",
        );
    }

    #[test]
    fn test_typesafe_fixture_ping() {
        assert_noul_fixture(
            include_str!("../fixtures/req_ping.json"),
            include_str!("../fixtures/typesafe/resp_ping.json"),
            "proceed",
            "jev-1.13.0",
        );
    }

    #[test]
    fn test_typesafe_fixture_dataeng() {
        assert_live_fixture(
            include_str!("../fixtures/req_4_dataeng.json"),
            include_str!("../fixtures/typesafe/resp_4_dataeng.json"),
            "data_action",
            "page_oncall",
            "report_risk",
            "jev-1.13.0",
        );
    }

    #[test]
    fn test_typesafe_fixture_refund() {
        assert_live_fixture(
            include_str!("../fixtures/req_5_refund.json"),
            include_str!("../fixtures/typesafe/resp_5_refund.json"),
            "refund_action",
            "needs_human_review",
            "churn_risk",
            "jev-1.13.0",
        );
    }

    #[test]
    fn test_typesafe_fixture_flags() {
        assert_live_fixture(
            include_str!("../fixtures/req_6_flags.json"),
            include_str!("../fixtures/typesafe/resp_6_flags.json"),
            "rollout_action",
            "within_budget",
            "rollout_risk",
            "jev-1.13.0",
        );
    }

    #[test]
    fn test_typesafe_fixture_oncall() {
        assert_live_fixture(
            include_str!("../fixtures/req_7_oncall.json"),
            include_str!("../fixtures/typesafe/resp_7_oncall.json"),
            "page_action",
            "sev1",
            "customer_impact",
            "jev-1.13.0",
        );
    }

    #[test]
    fn test_openrouter_fixture_dataeng() {
        assert_live_fixture(
            include_str!("../fixtures/req_4_dataeng.json"),
            include_str!("../fixtures/openrouter/oresp_4_dataeng.json"),
            "data_action",
            "page_oncall",
            "report_risk",
            "typesafe/jev-1.13-20260917",
        );
    }

    #[test]
    fn test_openrouter_fixture_refund() {
        assert_live_fixture(
            include_str!("../fixtures/req_5_refund.json"),
            include_str!("../fixtures/openrouter/oresp_5_refund.json"),
            "refund_action",
            "needs_human_review",
            "churn_risk",
            "typesafe/jev-1.13-20260917",
        );
    }

    #[test]
    fn test_openrouter_fixture_flags() {
        assert_live_fixture(
            include_str!("../fixtures/req_6_flags.json"),
            include_str!("../fixtures/openrouter/oresp_6_flags.json"),
            "rollout_action",
            "within_budget",
            "rollout_risk",
            "typesafe/jev-1.13-20260917",
        );
    }

    #[test]
    fn test_openrouter_fixture_oncall() {
        assert_live_fixture(
            include_str!("../fixtures/req_7_oncall.json"),
            include_str!("../fixtures/openrouter/oresp_7_oncall.json"),
            "page_action",
            "sev1",
            "customer_impact",
            "typesafe/jev-1.13-20260917",
        );
    }
}
