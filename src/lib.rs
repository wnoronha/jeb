//! `jeb` shared library: decision providers, validation, and output formatting.

pub mod client;
pub mod types;

use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use types::{Answer, Provider, Question, SystemOneRequest, SystemOneResponse};

/// Probability at or above which a noul answer maps to yes.
const NOUL_YES_THRESHOLD: f64 = 0.5;

/// Validates structural rules shared by quick-mode and JSON inputs.
pub fn validate_request(req: &SystemOneRequest) -> Result<()> {
    if req.questions.is_empty() {
        bail!("Payload must contain at least one question");
    }
    for (id, question) in &req.questions {
        let instructions = match question {
            Question::Choice { instructions, .. }
            | Question::Noul { instructions, .. }
            | Question::Score { instructions, .. } => instructions,
        };
        match instructions {
            serde_json::Value::Null => {
                bail!("Question '{}' instructions must not be null", id);
            }
            serde_json::Value::String(text) if text.trim().is_empty() => {
                bail!("Question '{}' instructions must not be blank", id);
            }
            _ => {}
        }
        match question {
            Question::Choice { criteria, .. } => {
                if criteria.is_empty() {
                    bail!(
                        "Choice question '{}' requires at least one criteria entry",
                        id
                    );
                }
                for key in criteria.keys() {
                    if key.trim().is_empty() {
                        bail!("Choice question '{}' has a blank option key", id);
                    }
                }
            }
            Question::Score { criteria, .. } => {
                if criteria.len() < 2 {
                    bail!(
                        "Score question '{}' requires at least 2 criteria levels",
                        id
                    );
                }
                for level in criteria {
                    match level {
                        serde_json::Value::Null => {
                            bail!("Score question '{}' has a null level", id);
                        }
                        serde_json::Value::String(text) if text.trim().is_empty() => {
                            bail!("Score question '{}' has a blank level", id);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Parses raw JSON string and applies the resolved model.
/// An explicit flag or env model overrides the payload field.
/// Otherwise the payload field applies when present and non-empty.
pub fn parse_json_payload(
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
            Some(serde_json::Value::String(text)) => {
                if text.trim().is_empty() {
                    bail!("Invalid payload: \"model\" must not be blank");
                }
                if model_explicit {
                    obj.insert(
                        "model".to_string(),
                        serde_json::Value::String(default_model.to_string()),
                    );
                }
            }
            Some(_) => {
                bail!("Invalid payload: \"model\" must be a non-empty string");
            }
        }
    }

    let req: SystemOneRequest = serde_json::from_value(val)
        .context("Failed to deserialize payload into SystemOneRequest format")?;
    validate_request(&req)?;
    Ok(req)
}

/// Formats and outputs the top decision summary.
///
/// Maps a noul probability at or above 0.5 to yes.
/// Maps a lower probability to no.
pub fn print_decision_summary(response: &SystemOneResponse, pretty: bool) -> Result<()> {
    let out = build_decision_value(response);

    if pretty {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("{}", serde_json::to_string(&out)?);
    }

    Ok(())
}

/// Resolves a score to its legend label, preferring the rounded value
/// and falling back to the entry nearest the raw score.
pub fn score_decision_label(legend: &BTreeMap<String, String>, score: f64) -> String {
    let target = score.round() as i64;
    if let Some(label) = legend.get(&target.to_string()) {
        return label.clone();
    }
    legend
        .iter()
        .filter_map(|(key, label)| key.parse::<i64>().ok().map(|n| (n as f64, label)))
        .min_by(|(a, _), (b, _)| (a - score).abs().total_cmp(&(b - score).abs()))
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| score.to_string())
}

/// Builds the decision-only JSON value for a response.
pub fn build_decision_value(response: &SystemOneResponse) -> serde_json::Value {
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

/// Resolves a setting from the explicit flag value, then the provider
/// env fallback, then the provider default. Blank values count as unset.
pub fn resolve_setting(explicit: Option<String>, fallback_env: &str, default: &str) -> String {
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
pub fn resolve_api_key(explicit: Option<String>, fallback_env: &str) -> Option<String> {
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
pub fn resolve_model(explicit: Option<String>, provider: Provider) -> (String, bool) {
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
    fn test_validate_request_rejects_blank_instructions() {
        let mut questions = BTreeMap::new();
        questions.insert(
            "q".to_string(),
            Question::Noul {
                instructions: serde_json::json!("   "),
                criteria: None,
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
    fn test_validate_request_rejects_blank_choice_key() {
        let mut criteria = BTreeMap::new();
        criteria.insert("  ".to_string(), serde_json::json!("desc"));
        let mut questions = BTreeMap::new();
        questions.insert(
            "c".to_string(),
            Question::Choice {
                instructions: serde_json::json!("Pick"),
                criteria,
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
    fn test_validate_request_rejects_blank_score_level() {
        let mut questions = BTreeMap::new();
        questions.insert(
            "s".to_string(),
            Question::Score {
                instructions: serde_json::json!("Rate"),
                criteria: vec![serde_json::json!("Low"), serde_json::json!("  ")],
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
        let json = r#"{"model": null, "state": "x", "questions": {"q": {"type": "noul", "instructions": "Proceed?"}}}"#;
        assert!(parse_json_payload(json, "default-model", false).is_err());
    }

    #[test]
    fn test_parse_json_payload_rejects_blank_model() {
        let json = r#"{"model": "  ", "state": "x", "questions": {"q": {"type": "noul", "instructions": "Proceed?"}}}"#;
        assert!(parse_json_payload(json, "default-model", false).is_err());
    }

    #[test]
    fn test_parse_json_payload_rejects_non_string_model() {
        let json = r#"{"model": 123, "state": "x", "questions": {"q": {"type": "noul", "instructions": "Proceed?"}}}"#;
        assert!(parse_json_payload(json, "flag-model", true).is_err());
        assert!(parse_json_payload(json, "default-model", false).is_err());
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
    fn test_score_decision_label_breaks_sparse_ties() {
        let legend: BTreeMap<String, String> = [
            ("0".to_string(), "Low".to_string()),
            ("2".to_string(), "High".to_string()),
        ]
        .into_iter()
        .collect();
        assert_eq!(score_decision_label(&legend, 0.6), "Low");
        assert_eq!(score_decision_label(&legend, 1.6), "High");
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
        assert_eq!(risk["decision"], "High");
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
}
