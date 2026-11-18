//! `jeb` CLI shape and request construction.
//!
//! Everything about how a `SystemOneRequest` comes to exist lives here:
//! quick-mode flags, file input, positional args, and piped stdin.

use anyhow::{bail, Context, Result};
use clap::{ArgAction, Parser};
use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

use crate::resolve_model;
use crate::types::{NoulCriteria, Provider, Question, SystemOneRequest};

/// CLI argument parser configuration for `jeb`.
#[derive(Parser, Debug)]
#[command(
    name = "jeb",
    about = "CLI tool to make quick decisions with TypeSafe AI's System One API",
    version
)]
pub struct Cli {
    /// Positional JSON payload string or "-" to read from stdin
    #[arg(value_name = "JSON_OR_DASH")]
    pub input: Option<String>,

    /// Read request JSON from a file
    #[arg(short, long, value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Decision API provider
    #[arg(long, value_enum, env = "JEB_PROVIDER", default_value_t = Provider::Typesafe)]
    pub provider: Provider,

    /// Model to use for evaluation.
    /// Env: TYPESAFE_MODEL with --provider typesafe, OPENROUTER_MODEL with --provider openrouter.
    #[arg(short, long)]
    pub model: Option<String>,

    /// API key for the selected provider. Prefer the provider env var:
    /// a flag value stays visible in shell history and process lists.
    /// Env: TYPESAFE_API_KEY with --provider typesafe, OPENROUTER_API_KEY with --provider openrouter.
    #[arg(short = 'k', long)]
    pub api_key: Option<String>,

    /// API base URL.
    /// Env: TYPESAFE_BASE_URL with --provider typesafe, OPENROUTER_BASE_URL with --provider openrouter.
    #[arg(long)]
    pub base_url: Option<String>,

    /// Request timeout in seconds
    #[arg(long, default_value = "30")]
    pub timeout: u64,

    /// Pretty print JSON output
    #[arg(short, long, default_value_t = false)]
    pub pretty: bool,

    /// Output only the top decision / judgment summary
    #[arg(short = 'd', long = "decision-only", default_value_t = false)]
    pub decision_only: bool,

    // Quick CLI builder flags (alternative to passing raw JSON)
    /// Direct state text or JSON string for quick query mode
    #[arg(long, value_name = "TEXT_OR_JSON")]
    pub state: Option<String>,

    /// Add a choice question: ID and instructions (e.g. --choice "next_step:What should be done?")
    #[arg(long, value_name = "ID:INSTRUCTIONS")]
    pub choice: Option<String>,

    /// Option for choice question in key=description format (repeatable, e.g. --option "test=Run tests")
    #[arg(long = "option", action = ArgAction::Append)]
    pub options: Vec<String>,

    /// Add a noul (yes/no) question: ID and instructions (e.g. --noul "is_safe:Is this action safe to execute?")
    /// Repeatable for multiple questions. --noul-true and --noul-false need exactly one --noul.
    #[arg(long, value_name = "ID:INSTRUCTIONS", action = ArgAction::Append)]
    pub noul: Vec<String>,

    /// Description for the yes outcome of a noul question
    #[arg(long, value_name = "DESC")]
    pub noul_true: Option<String>,

    /// Description for the no outcome of a noul question
    #[arg(long, value_name = "DESC")]
    pub noul_false: Option<String>,

    /// Add a score question: ID and instructions (e.g. --score "urgency:Rate urgency from 0 to 2")
    /// Repeatable for multiple questions. --level needs exactly one --score.
    #[arg(long, value_name = "ID:INSTRUCTIONS", action = ArgAction::Append)]
    pub score: Vec<String>,

    /// Level descriptions for score question (repeatable or comma-separated)
    #[arg(long = "level", action = ArgAction::Append)]
    pub levels: Vec<String>,

    /// List available decision models and exit
    #[arg(long, default_value_t = false)]
    pub list_models: bool,

    /// Print the built request JSON and exit without calling the API
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// Exit 1 when any noul decision is no. Output still prints.
    #[arg(long, default_value_t = false)]
    pub fail: bool,

    /// Suppress normal output. Errors still print.
    #[arg(short, long, default_value_t = false)]
    pub quiet: bool,
}

impl Cli {
    /// Rejects evaluation flags that `--list-models` silently drops.
    pub fn reject_list_mode_conflicts(&self) -> Result<()> {
        if !self.list_models {
            return Ok(());
        }
        let mut dropped = Vec::new();
        if self.input.is_some() {
            dropped.push("positional JSON input");
        }
        if self.file.is_some() {
            dropped.push("--file");
        }
        if self.state.is_some() {
            dropped.push("--state");
        }
        if self.choice.is_some() {
            dropped.push("--choice");
        }
        if !self.options.is_empty() {
            dropped.push("--option");
        }
        if !self.noul.is_empty() {
            dropped.push("--noul");
        }
        if self.noul_true.is_some() {
            dropped.push("--noul-true");
        }
        if self.noul_false.is_some() {
            dropped.push("--noul-false");
        }
        if !self.score.is_empty() {
            dropped.push("--score");
        }
        if !self.levels.is_empty() {
            dropped.push("--level");
        }
        if self.model.is_some() {
            dropped.push("--model");
        }
        if self.decision_only {
            dropped.push("--decision-only");
        }
        if dropped.is_empty() {
            Ok(())
        } else {
            bail!(
                "--list-models cannot be combined with evaluation flags: {}",
                dropped.join(", ")
            );
        }
    }

    /// Builds a validated `SystemOneRequest` from CLI inputs.
    pub fn build_request(&self) -> Result<SystemOneRequest> {
        if self.timeout == 0 {
            bail!("Timeout must be greater than zero seconds.");
        }

        let (model, model_explicit) = resolve_model(self.model.clone(), self.provider);

        if self.file.is_some() && self.input.is_some() {
            bail!("--file cannot be combined with positional JSON input");
        }

        let has_question_flag =
            self.choice.is_some() || !self.noul.is_empty() || !self.score.is_empty();
        if self.state.is_none() {
            if has_question_flag {
                bail!("Question flags (--choice, --noul, --score) require --state");
            }
            if !self.options.is_empty() {
                bail!("--option requires --choice");
            }
            if !self.levels.is_empty() {
                bail!("--level requires --score");
            }
            if self.noul_true.is_some() || self.noul_false.is_some() {
                bail!("--noul-true and --noul-false require --noul");
            }
        } else {
            if self.choice.is_none() && !self.options.is_empty() {
                bail!("--option requires --choice");
            }
            if self.score.is_empty() && !self.levels.is_empty() {
                bail!("--level requires --score");
            }
            if self.noul.is_empty() && (self.noul_true.is_some() || self.noul_false.is_some()) {
                bail!("--noul-true and --noul-false require --noul");
            }
            if (self.noul_true.is_some() || self.noul_false.is_some()) && self.noul.len() != 1 {
                bail!("--noul-true and --noul-false require exactly one --noul");
            }
            if !self.levels.is_empty() && self.score.len() != 1 {
                bail!("--level requires exactly one --score");
            }
            if self.file.is_some() || self.input.is_some() {
                bail!("--state cannot be combined with --file or JSON input");
            }
        }
        // 1. If quick CLI flags are provided, construct request from flags
        if let Some(state_raw) = &self.state {
            let state_val = serde_json::from_str::<serde_json::Value>(state_raw)
                .unwrap_or_else(|_| serde_json::Value::String(state_raw.clone()));

            let mut questions = BTreeMap::new();

            if let Some(choice_spec) = &self.choice {
                let (id, instructions) = split_id_and_instructions(choice_spec, "choice")?;
                let mut criteria = BTreeMap::new();
                for opt in &self.options {
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
                        criteria.insert(key.clone(), serde_json::Value::String(key));
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

            for noul_spec in &self.noul {
                let (id, instructions) = split_id_and_instructions(noul_spec, "noul")?;
                let criteria = match (&self.noul_true, &self.noul_false) {
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

            for score_spec in &self.score {
                let (id, instructions) = split_id_and_instructions(score_spec, "score")?;
                let mut criteria = Vec::new();
                for lvl in &self.levels {
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
                bail!(
                    "Quick mode with --state requires at least one question flag (--choice, --noul, or --score)"
                );
            }

            let req = SystemOneRequest {
                model,
                state: state_val,
                questions,
            };
            req.validate()?;
            return Ok(req);
        }

        // 2. If file is passed
        if let Some(path) = &self.file {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read JSON file from {}", path.display()))?;
            return SystemOneRequest::from_json(&content, &model, model_explicit);
        }

        // 3. If input arg is passed
        if let Some(input) = &self.input {
            if input == "-" {
                let mut buffer = String::new();
                io::stdin()
                    .read_to_string(&mut buffer)
                    .context("Failed to read from stdin")?;
                return SystemOneRequest::from_json(&buffer, &model, model_explicit);
            } else {
                return SystemOneRequest::from_json(input, &model, model_explicit);
            }
        }

        // 4. If stdin is piped
        if !io::stdin().is_terminal() {
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .context("Failed to read from stdin")?;
            if !buffer.trim().is_empty() {
                return SystemOneRequest::from_json(&buffer, &model, model_explicit);
            }
        }

        bail!(
            "No input provided. Provide JSON as an argument, pipe through stdin, use --file, or specify --state and question flags."
        );
    }
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
            noul: vec![],
            noul_true: None,
            noul_false: None,
            provider: Provider::Typesafe,
            list_models: false,
            dry_run: false,
            fail: false,
            quiet: false,
            score: vec![],
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
            noul: vec!["is_clear:Is the request clear?".to_string()],
            ..test_cli()
        };

        let req = cli.build_request().unwrap();
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
            noul: vec!["task:Is it done?".to_string()],
            ..test_cli()
        };
        assert!(cli.build_request().is_err());
    }

    #[test]
    fn test_quick_rejects_file_with_input() {
        let cli = Cli {
            input: Some("{\"state\": \"x\"}".to_string()),
            file: Some(std::path::PathBuf::from("req.json")),
            ..test_cli()
        };
        assert!(cli.build_request().is_err());
    }

    #[test]
    fn test_quick_rejects_question_flag_without_state() {
        let cli = Cli {
            choice: Some("task:Do this".to_string()),
            options: vec!["a=First".to_string()],
            ..test_cli()
        };
        assert!(cli.build_request().is_err());
    }

    #[test]
    fn test_quick_rejects_dangling_option() {
        let cli = Cli {
            state: Some("state".to_string()),
            options: vec!["a=First".to_string()],
            noul: vec!["safe:Is it safe?".to_string()],
            ..test_cli()
        };
        assert!(cli.build_request().is_err());
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
        assert!(cli.build_request().is_err());
    }

    #[test]
    fn test_quick_rejects_dangling_noul_labels() {
        let cli = Cli {
            state: Some("state".to_string()),
            noul_true: Some("Yes".to_string()),
            score: vec!["risk:Rate it".to_string()],
            levels: vec!["Low".to_string(), "High".to_string()],
            ..test_cli()
        };
        assert!(cli.build_request().is_err());
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
        assert!(cli.build_request().is_err());
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
        assert!(cli.build_request().is_err());
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
        assert!(cli.build_request().is_err());
    }

    #[test]
    fn test_zero_timeout_rejected_before_network() {
        let cli = Cli {
            timeout: 0,
            state: Some("state".to_string()),
            noul: vec!["safe:Is it safe?".to_string()],
            ..test_cli()
        };
        let err = cli.build_request().unwrap_err().to_string();
        assert!(err.contains("greater than zero"));
    }

    #[test]
    fn test_list_mode_rejects_evaluation_flags() {
        let cli = Cli {
            list_models: true,
            state: Some("state".to_string()),
            noul: vec!["safe:Is it safe?".to_string()],
            ..test_cli()
        };
        let err = cli.reject_list_mode_conflicts().unwrap_err().to_string();
        assert!(err.contains("--list-models cannot be combined"));

        let clean = Cli {
            list_models: true,
            ..test_cli()
        };
        assert!(clean.reject_list_mode_conflicts().is_ok());

        let evaluating = test_cli();
        assert!(evaluating.reject_list_mode_conflicts().is_ok());
    }

    #[test]
    fn test_repeatable_noul_and_score() {
        let cli = Cli {
            state: Some("state".to_string()),
            noul: vec!["a:Is it safe?".to_string(), "b:Is it cheap?".to_string()],
            score: vec!["x:Rate x".to_string()],
            levels: vec!["Low".to_string(), "High".to_string()],
            ..test_cli()
        };
        let req = cli.build_request().unwrap();
        assert_eq!(req.questions.len(), 3);
    }

    #[test]
    fn test_labels_require_single_question() {
        let cli = Cli {
            state: Some("state".to_string()),
            noul: vec!["a:Is it safe?".to_string(), "b:Is it cheap?".to_string()],
            noul_true: Some("Yes".to_string()),
            ..test_cli()
        };
        assert!(cli.build_request().is_err());

        let cli = Cli {
            state: Some("state".to_string()),
            score: vec!["x:Rate x".to_string(), "y:Rate y".to_string()],
            levels: vec!["Low".to_string(), "High".to_string()],
            ..test_cli()
        };
        assert!(cli.build_request().is_err());
    }

    #[test]
    fn test_bare_option_uses_key_as_description() {
        let cli = Cli {
            state: Some("state".to_string()),
            choice: Some("task:Do this".to_string()),
            options: vec!["restart".to_string()],
            ..test_cli()
        };
        let req = cli.build_request().unwrap();
        match req.questions.get("task").unwrap() {
            Question::Choice { criteria, .. } => {
                assert_eq!(
                    criteria.get("restart").unwrap(),
                    &serde_json::Value::String("restart".to_string())
                );
            }
            _ => panic!("Expected choice question"),
        }
    }

    #[test]
    fn test_noul_criteria_flags() {
        let cli = Cli {
            model: Some("jev-latest".to_string()),
            base_url: Some("https://api.typesafe.ai".to_string()),
            state: Some("state".to_string()),
            noul: vec!["safe:Is it safe?".to_string()],
            noul_true: Some("Safe to proceed".to_string()),
            noul_false: Some("Unsafe".to_string()),
            ..test_cli()
        };
        let req = cli.build_request().unwrap();
        match req.questions.get("safe").unwrap() {
            Question::Noul { criteria, .. } => {
                assert!(criteria.is_some());
            }
            _ => panic!("Expected noul question"),
        }
    }
}
