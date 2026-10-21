//! `jeb` — CLI tool to make quick decisions with TypeSafe AI's System One API.

use anyhow::{bail, Context, Result};
use clap::{ArgAction, Parser};
use jeb::client::{fetch_openrouter_models, DecisionsClient};
use jeb::types::{NoulCriteria, Provider, Question, SystemOneRequest};
use jeb::{
    parse_json_payload, print_decision_summary, resolve_api_key, resolve_model, resolve_setting,
    validate_request,
};
use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

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

/// Parses CLI inputs into a validated `SystemOneRequest`.
fn parse_request(cli: &Cli, provider: Provider) -> Result<SystemOneRequest> {
    let (model, model_explicit) = resolve_model(cli.model.clone(), provider);

    if cli.file.is_some() && cli.input.is_some() {
        bail!("--file cannot be combined with positional JSON input");
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cli() -> Cli {
        Cli {
            input: None,
            file: None,
            model: None,
            api_key: None,
            base_url: None,
            timeout: 30,
            pretty: false,
            decision_only: false,
            state: None,
            choice: None,
            options: vec![],
            noul: None,
            noul_true: None,
            noul_false: None,
            provider: Provider::Typesafe,
            list_models: false,
            score: None,
            levels: vec![],
        }
    }

    #[test]
    fn test_parse_quick_flags() {
        let cli = Cli {
            model: Some("jev-latest".to_string()),
            api_key: Some("dummy-key".to_string()),
            base_url: Some("https://api.typesafe.ai".to_string()),
            state: Some("Customer says: Please reset my password".to_string()),
            choice: Some("intent:What is the user requesting?".to_string()),
            options: vec![
                "auth=Password and authentication".to_string(),
                "billing=Billing questions".to_string(),
            ],
            noul: Some("is_clear:Is the request clear?".to_string()),
            ..test_cli()
        };

        let req = parse_request(&cli, Provider::Typesafe).unwrap();
        assert_eq!(req.questions.len(), 2);
        assert!(req.questions.contains_key("intent"));
        assert!(req.questions.contains_key("is_clear"));
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
    fn test_quick_rejects_duplicate_question_id() {
        let cli = Cli {
            state: Some("state".to_string()),
            choice: Some("task:Do this".to_string()),
            options: vec!["a=First".to_string()],
            noul: Some("task:Is it done?".to_string()),
            ..test_cli()
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_rejects_file_with_input() {
        let cli = Cli {
            input: Some("{\"state\": \"x\"}".to_string()),
            file: Some(std::path::PathBuf::from("req.json")),
            ..test_cli()
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_rejects_question_flag_without_state() {
        let cli = Cli {
            choice: Some("task:Do this".to_string()),
            options: vec!["a=First".to_string()],
            ..test_cli()
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_rejects_dangling_option() {
        let cli = Cli {
            state: Some("state".to_string()),
            options: vec!["a=First".to_string()],
            noul: Some("safe:Is it safe?".to_string()),
            ..test_cli()
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_rejects_dangling_level() {
        let cli = Cli {
            state: Some("state".to_string()),
            choice: Some("task:Do this".to_string()),
            options: vec!["a=First".to_string()],
            levels: vec!["High".to_string()],
            ..test_cli()
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_rejects_dangling_noul_labels() {
        let cli = Cli {
            state: Some("state".to_string()),
            noul_true: Some("Yes".to_string()),
            score: Some("risk:Rate it".to_string()),
            levels: vec!["Low".to_string(), "High".to_string()],
            ..test_cli()
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_rejects_state_with_file() {
        let cli = Cli {
            file: Some(std::path::PathBuf::from("req.json")),
            state: Some("state".to_string()),
            choice: Some("task:Do this".to_string()),
            options: vec!["a=First".to_string()],
            ..test_cli()
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_choice_rejects_empty_option_key() {
        let cli = Cli {
            model: Some("jev-latest".to_string()),
            base_url: Some("https://api.typesafe.ai".to_string()),
            state: Some("state".to_string()),
            choice: Some("task:Do this".to_string()),
            options: vec!["=desc".to_string()],
            ..test_cli()
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_quick_choice_rejects_duplicate_option_key() {
        let cli = Cli {
            model: Some("jev-latest".to_string()),
            base_url: Some("https://api.typesafe.ai".to_string()),
            state: Some("state".to_string()),
            choice: Some("task:Do this".to_string()),
            options: vec!["a=First".to_string(), "a=Second".to_string()],
            ..test_cli()
        };
        assert!(parse_request(&cli, Provider::Typesafe).is_err());
    }

    #[test]
    fn test_noul_criteria_flags() {
        let cli = Cli {
            model: Some("jev-latest".to_string()),
            base_url: Some("https://api.typesafe.ai".to_string()),
            state: Some("state".to_string()),
            noul: Some("safe:Is it safe?".to_string()),
            noul_true: Some("Safe to proceed".to_string()),
            noul_false: Some("Unsafe".to_string()),
            ..test_cli()
        };
        let req = parse_request(&cli, Provider::Typesafe).unwrap();
        match req.questions.get("safe").unwrap() {
            Question::Noul { criteria, .. } => {
                assert!(criteria.is_some());
            }
            _ => panic!("Expected noul question"),
        }
    }
}
