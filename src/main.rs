//! `jeb` — CLI tool to make quick decisions with TypeSafe AI's System One API.

mod client;
mod types;

use anyhow::{bail, Context, Result};
use clap::{ArgAction, Parser};
use client::TypeSafeClient;
use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use types::{Answer, Question, SystemOneRequest, SystemOneResponse};

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

    /// Model to use for evaluation
    #[arg(short, long, env = "TYPESAFE_MODEL", default_value = "jev-latest")]
    model: String,

    /// TypeSafe API key
    #[arg(short = 'k', long, env = "TYPESAFE_API_KEY")]
    api_key: Option<String>,

    /// TypeSafe API Base URL
    #[arg(
        long,
        env = "TYPESAFE_BASE_URL",
        default_value = "https://api.typesafe.ai"
    )]
    base_url: String,

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

    /// Add a score question: ID and instructions (e.g. --score "urgency:Rate urgency from 0 to 2")
    #[arg(long, value_name = "ID:INSTRUCTIONS")]
    score: Option<String>,

    /// Level descriptions for score question (repeatable or comma-separated)
    #[arg(long = "level", action = ArgAction::Append)]
    levels: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let request = parse_request(&cli)?;

    let api_key = cli
        .api_key
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Missing TypeSafe API key. Set TYPESAFE_API_KEY environment variable or pass --api-key."
            )
        })?;

    let client = TypeSafeClient::new(api_key, Some(cli.base_url), cli.timeout)?;
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

/// Parses CLI inputs into a validated `SystemOneRequest`.
fn parse_request(cli: &Cli) -> Result<SystemOneRequest> {
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
                    criteria.insert(
                        k.trim().to_string(),
                        serde_json::Value::String(v.trim().to_string()),
                    );
                } else {
                    criteria.insert(opt.trim().to_string(), serde_json::Value::Null);
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
            questions.insert(
                id,
                Question::Noul {
                    instructions: serde_json::Value::String(instructions),
                    criteria: None,
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
            model: cli.model.clone(),
            state: state_val,
            questions,
        });
    }

    // 2. If file is passed
    if let Some(path) = &cli.file {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read JSON file from {}", path.display()))?;
        return parse_json_payload(&content, &cli.model);
    }

    // 3. If input arg is passed
    if let Some(input) = &cli.input {
        if input == "-" {
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .context("Failed to read from stdin")?;
            return parse_json_payload(&buffer, &cli.model);
        } else {
            return parse_json_payload(input, &cli.model);
        }
    }

    // 4. If stdin is piped
    if !io::stdin().is_terminal() {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        if !buffer.trim().is_empty() {
            return parse_json_payload(&buffer, &cli.model);
        }
    }

    bail!("No input provided. Provide JSON as an argument, pipe through stdin, use --file, or specify --state and question flags.");
}

/// Parses raw JSON string and sets fallback model if omitted.
fn parse_json_payload(raw: &str, default_model: &str) -> Result<SystemOneRequest> {
    let mut val: serde_json::Value =
        serde_json::from_str(raw).context("Invalid JSON payload passed to jeb")?;

    if val.get("model").is_none() {
        if let Some(obj) = val.as_object_mut() {
            obj.insert(
                "model".to_string(),
                serde_json::Value::String(default_model.to_string()),
            );
        }
    }

    let req: SystemOneRequest = serde_json::from_value(val)
        .context("Failed to deserialize payload into SystemOneRequest format")?;
    Ok(req)
}

/// Splits flag input of format `id:instructions` into constituent parts.
fn split_id_and_instructions(spec: &str, qtype: &str) -> Result<(String, String)> {
    if let Some((id, inst)) = spec.split_once(':') {
        Ok((id.trim().to_string(), inst.trim().to_string()))
    } else {
        bail!(
            "Invalid format for --{}. Expected 'question_id:Instructions text', got '{}'",
            qtype,
            spec
        );
    }
}

/// Formats and outputs the top decision summary.
fn print_decision_summary(response: &SystemOneResponse, pretty: bool) -> Result<()> {
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
                let decision = if *noul >= 0.5 { "yes" } else { "no" };
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
                score, confidence, ..
            } => {
                decisions.insert(
                    k,
                    serde_json::json!({
                        "type": "score",
                        "score": score,
                        "confidence": confidence
                    }),
                );
            }
        }
    }

    let out = serde_json::json!({
        "model": response.model,
        "decisions": decisions,
        "usage": response.usage
    });

    if pretty {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("{}", serde_json::to_string(&out)?);
    }

    Ok(())
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
            model: "jev-latest".to_string(),
            api_key: Some("dummy-key".to_string()),
            base_url: "https://api.typesafe.ai".to_string(),
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
            score: None,
            levels: vec![],
        };

        let req = parse_request(&cli).unwrap();
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
}
