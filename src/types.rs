//! Data types and schemas for TypeSafe AI System One requests and responses.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

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
    pub input_tokens: u32,
    /// Number of output tokens generated.
    pub output_tokens: u32,
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
#[allow(dead_code)]
pub struct ApiErrorResponse {
    #[serde(default)]
    pub error: Option<Value>,
    #[serde(default)]
    pub message: Option<String>,
}
