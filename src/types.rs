//! Data types and schemas for TypeSafe AI System One requests and responses.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Decision API provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Provider {
    /// TypeSafe AI System One API.
    Typesafe,
    /// OpenRouter Decisions API.
    Openrouter,
}

impl Provider {
    /// Base URL used when no explicit URL or env override is set.
    pub fn default_base_url(self) -> &'static str {
        match self {
            Provider::Typesafe => "https://api.typesafe.ai",
            Provider::Openrouter => "https://openrouter.ai",
        }
    }

    /// Model used when no explicit model or env override is set.
    pub fn default_model(self) -> &'static str {
        match self {
            Provider::Typesafe => "jev-latest",
            Provider::Openrouter => "typesafe/jev-1.13",
        }
    }

    /// Env var holding the API key for this provider.
    pub fn api_key_env(self) -> &'static str {
        match self {
            Provider::Typesafe => "TYPESAFE_API_KEY",
            Provider::Openrouter => "OPENROUTER_API_KEY",
        }
    }

    /// Env var holding the model override for this provider.
    pub fn model_env(self) -> &'static str {
        match self {
            Provider::Typesafe => "TYPESAFE_MODEL",
            Provider::Openrouter => "OPENROUTER_MODEL",
        }
    }

    /// Env var holding the base URL override for this provider.
    pub fn base_url_env(self) -> &'static str {
        match self {
            Provider::Typesafe => "TYPESAFE_BASE_URL",
            Provider::Openrouter => "OPENROUTER_BASE_URL",
        }
    }

    /// Path appended to the base URL for decision evaluations.
    pub fn decisions_path(self) -> &'static str {
        match self {
            Provider::Typesafe => "/v1/systemone",
            Provider::Openrouter => "/api/alpha/decisions",
        }
    }
}

/// A single entry of the OpenRouter decision model catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier passed as `model` in decision requests.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Vendor prefix of the identifier.
    pub vendor: String,
    /// True when the model reports zero pricing.
    pub is_free: bool,
}

/// Raw OpenRouter `/models` response envelope.
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModelsResponse {
    #[serde(default)]
    pub data: Vec<OpenRouterModelEntry>,
}

/// Raw OpenRouter model entry.
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModelEntry {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub pricing: Option<OpenRouterPricing>,
}

/// Raw OpenRouter pricing block. Values arrive as decimal strings.
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterPricing {
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub completion: Option<String>,
}

fn parse_price(raw: Option<&str>) -> Option<f64> {
    raw?.trim().parse::<f64>().ok()
}

/// Converts a raw OpenRouter catalog into display entries.
/// Drops internal entries and sorts free models first.
pub fn parse_model_catalog(response: &OpenRouterModelsResponse) -> Vec<ModelInfo> {
    let mut out: Vec<ModelInfo> = response
        .data
        .iter()
        .filter(|m| !m.id.is_empty() && !m.id.starts_with('~'))
        .map(|m| {
            let vendor =
                m.id.split_once('/')
                    .map(|(v, _)| v.to_string())
                    .unwrap_or_else(|| "other".to_string());
            let label = m
                .name
                .as_deref()
                .unwrap_or(&m.id)
                .split_once(':')
                .map(|(_, rest)| rest.trim())
                .unwrap_or_else(|| m.name.as_deref().unwrap_or(&m.id))
                .trim()
                .to_string();
            let prompt = parse_price(m.pricing.as_ref().and_then(|p| p.prompt.as_deref()));
            let completion = parse_price(m.pricing.as_ref().and_then(|p| p.completion.as_deref()));
            let is_free = m.id.contains(":free")
                || (prompt == Some(0.0) && completion.is_none_or(|c| c == 0.0));
            ModelInfo {
                id: m.id.clone(),
                label: if label.is_empty() {
                    m.id.clone()
                } else {
                    label
                },
                vendor,
                is_free,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.is_free
            .cmp(&a.is_free)
            .then(a.vendor.cmp(&b.vendor))
            .then(a.label.cmp(&b.label))
    });
    out
}

/// Request payload sent to the TypeSafe AI System One evaluation endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemOneRequest {
    /// Model identifier to use (e.g., "jev-latest").
    #[serde(default = "default_model")]
    pub model: String,
    /// Contextual state (text description, JSON object, or structured facts).
    pub state: Value,
    /// Map of question IDs to question definitions.
    pub questions: BTreeMap<String, Question>,
}

fn default_model() -> String {
    "jev-latest".to_string()
}

/// A question evaluated by System One.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Question {
    /// Yes/No probability question.
    #[serde(rename = "noul")]
    Noul {
        /// Instructions or question prompt.
        instructions: Value,
        /// Optional criteria mapping true/false labels to descriptions.
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    /// Multi-option categorical choice question.
    #[serde(rename = "choice")]
    Choice {
        /// Instructions or selection prompt.
        instructions: Value,
        /// Map of option keys to description text.
        criteria: BTreeMap<String, Value>,
    },
    /// Discrete rating or intensity score question.
    #[serde(rename = "score")]
    Score {
        /// Instructions or evaluation prompt.
        instructions: Value,
        /// Ordered levels describing score progression (e.g., ["Low", "Med", "High"]).
        criteria: Vec<Value>,
    },
}

/// Optional criteria descriptions for `noul` (yes/no) questions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NoulCriteria {
    /// Description for true/yes outcome.
    #[serde(rename = "true", skip_serializing_if = "Option::is_none")]
    pub r#true: Option<Value>,
    /// Description for false/no outcome.
    #[serde(rename = "false", skip_serializing_if = "Option::is_none")]
    pub r#false: Option<Value>,
}

/// Answer returned by System One for a single question.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    /// Result for a `noul` question containing probability between 0.0 and 1.0.
    #[serde(rename = "noul")]
    Noul { noul: f64 },
    /// Result for a `choice` question containing winning choice, probabilities, and confidence.
    #[serde(rename = "choice")]
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    /// Result for a `score` question containing numerical score, legend, and confidence.
    #[serde(rename = "score")]
    Score {
        score: f64,
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
}

/// Token usage metadata for the evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Number of prompt tokens processed.
    pub input_tokens: u64,
    /// Number of output tokens generated.
    pub output_tokens: u64,
}

/// System One response payload containing evaluated answers and usage stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemOneResponse {
    /// Model used to perform evaluation.
    pub model: String,
    /// Map of question IDs to evaluated answers.
    pub answers: BTreeMap<String, Answer>,
    /// Token usage metrics.
    pub usage: Usage,
}

/// Error structure returned by the TypeSafe AI API on failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    #[serde(default)]
    pub error: Option<Value>,
    #[serde(default)]
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_endpoints() {
        assert_eq!(
            format!(
                "{}{}",
                Provider::Typesafe.default_base_url(),
                Provider::Typesafe.decisions_path()
            ),
            "https://api.typesafe.ai/v1/systemone"
        );
        assert_eq!(
            format!(
                "{}{}",
                Provider::Openrouter.default_base_url(),
                Provider::Openrouter.decisions_path()
            ),
            "https://openrouter.ai/api/alpha/decisions"
        );
    }

    #[test]
    fn test_parse_model_catalog() {
        let raw = r#"{
            "data": [
                {"id": "~internal", "name": "Internal"},
                {"id": "typesafe/jev-1.13", "name": "Typesafe: Jev 1.13", "pricing": {"prompt": "0.0001", "completion": "0.0002"}},
                {"id": "openai/gpt-6-luna-decisions", "name": "GPT-6 Luna Decisions", "pricing": {"prompt": "0", "completion": "0"}},
                {"id": "free/model:free", "name": "Free Model", "pricing": {"prompt": "0.1", "completion": "0.1"}}
            ]
        }"#;
        let response: OpenRouterModelsResponse = serde_json::from_str(raw).unwrap();
        let models = parse_model_catalog(&response);
        assert_eq!(models.len(), 3);
        assert!(models
            .iter()
            .any(|m| m.id == "typesafe/jev-1.13" && !m.is_free));
        assert!(models
            .iter()
            .any(|m| m.id == "openai/gpt-6-luna-decisions" && m.is_free));
        assert!(models
            .iter()
            .any(|m| m.id == "free/model:free" && m.is_free));
        assert_eq!(models[0].vendor, "free");
    }
}
