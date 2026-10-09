//! Local decision evaluation via LiteRT-LM OpenAI-compatible `/v1/embeddings`.

use crate::types::{Answer, NoulCriteria, Question, SystemOneRequest, SystemOneResponse, Usage};
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

/// Client that maps System One requests to embedding similarity decisions.
pub struct LocalEmbeddingClient {
    embed_base_url: String,
    model: String,
    temperature: f64,
    client: Client,
}

impl LocalEmbeddingClient {
    pub fn new(
        embed_base_url: String,
        model: String,
        temperature: f64,
        timeout_secs: u64,
    ) -> Result<Self> {
        let embed_base_url = embed_base_url.trim_end_matches('/').to_string();
        if !temperature.is_finite() || temperature <= 0.0 {
            bail!("LiteRT similarity temperature must be a positive finite number");
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("Failed to build HTTP client for LiteRT embeddings")?;

        Ok(Self {
            embed_base_url,
            model,
            temperature,
            client,
        })
    }

    /// Evaluates all questions using EmbeddingGemma-style classification prefixes.
    pub fn evaluate(&self, request: &SystemOneRequest) -> Result<SystemOneResponse> {
        let mut answers = BTreeMap::new();
        let mut embed_calls = 0u32;

        for (id, question) in &request.questions {
            match question {
                Question::Choice {
                    instructions,
                    criteria,
                } => {
                    let (answer, calls) =
                        self.evaluate_choice(&request.state, instructions, criteria)?;
                    embed_calls += calls;
                    answers.insert(id.clone(), answer);
                }
                Question::Noul {
                    instructions,
                    criteria,
                } => {
                    let (answer, calls) =
                        self.evaluate_noul(&request.state, instructions, criteria.as_ref())?;
                    embed_calls += calls;
                    answers.insert(id.clone(), answer);
                }
                Question::Score {
                    instructions,
                    criteria,
                } => {
                    let (answer, calls) =
                        self.evaluate_score(&request.state, instructions, criteria)?;
                    embed_calls += calls;
                    answers.insert(id.clone(), answer);
                }
            }
        }

        Ok(SystemOneResponse {
            model: format!("litert-local:{}", self.model),
            answers,
            usage: Usage {
                input_tokens: embed_calls,
                output_tokens: 0,
            },
        })
    }

    fn evaluate_choice(
        &self,
        state: &Value,
        instructions: &Value,
        criteria: &BTreeMap<String, Value>,
    ) -> Result<(Answer, u32)> {
        let query = build_query_text(state, instructions);
        let mut keys = Vec::with_capacity(criteria.len());
        let mut candidate_texts = Vec::with_capacity(criteria.len());

        for (key, desc) in criteria {
            keys.push(key.clone());
            candidate_texts.push(classification_text(&format_option_label(key, desc)));
        }

        let mut texts = vec![classification_text(&query)];
        texts.extend(candidate_texts);
        let vectors = self.embed_batch(&texts)?;
        let query_vec = &vectors[0];
        let mut scores = Vec::with_capacity(keys.len());
        for cand in &vectors[1..] {
            scores.push(cosine_similarity(query_vec, cand));
        }
        let probs = softmax(&scores, self.temperature);
        let mut probabilities = BTreeMap::new();
        for (key, p) in keys.iter().zip(probs.iter()) {
            probabilities.insert(key.clone(), *p);
        }

        let choice = keys
            .iter()
            .zip(probs.iter())
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| keys[0].clone());

        let confidence = choice_confidence(&probs);

        Ok((
            Answer::Choice {
                choice,
                probabilities,
                confidence,
            },
            1,
        ))
    }

    fn evaluate_noul(
        &self,
        state: &Value,
        instructions: &Value,
        criteria: Option<&NoulCriteria>,
    ) -> Result<(Answer, u32)> {
        let query = build_query_text(state, instructions);
        let yes_desc = criteria
            .and_then(|c| c.r#true.as_ref())
            .map(value_as_text)
            .unwrap_or_else(|| "The situation warrants a yes answer.".to_string());
        let no_desc = criteria
            .and_then(|c| c.r#false.as_ref())
            .map(value_as_text)
            .unwrap_or_else(|| "The situation warrants a no answer.".to_string());

        let texts = vec![
            classification_text(&query),
            classification_text(&yes_desc),
            classification_text(&no_desc),
        ];
        let vectors = self.embed_batch(&texts)?;
        let sim_yes = cosine_similarity(&vectors[0], &vectors[1]);
        let sim_no = cosine_similarity(&vectors[0], &vectors[2]);
        let probs = softmax(&[sim_yes, sim_no], self.temperature);
        let noul = probs[0];

        Ok((Answer::Noul { noul }, 1))
    }

    fn evaluate_score(
        &self,
        state: &Value,
        instructions: &Value,
        criteria: &[Value],
    ) -> Result<(Answer, u32)> {
        if criteria.len() < 2 {
            bail!("Score question requires at least 2 criteria levels");
        }

        let query = build_query_text(state, instructions);
        let mut texts = vec![classification_text(&query)];
        for (i, level) in criteria.iter().enumerate() {
            let label = value_as_text(level);
            texts.push(classification_text(&format!(
                "Rating level {}: {}",
                i, label
            )));
        }

        let vectors = self.embed_batch(&texts)?;
        let query_vec = &vectors[0];
        let mut scores = Vec::with_capacity(criteria.len());
        for cand in &vectors[1..] {
            scores.push(cosine_similarity(query_vec, cand));
        }
        let probs = softmax(&scores, self.temperature);

        let mut probabilities = BTreeMap::new();
        let mut legend = BTreeMap::new();
        let mut weighted = 0.0f64;
        for (i, (level, p)) in criteria.iter().zip(probs.iter()).enumerate() {
            let key = i.to_string();
            probabilities.insert(key.clone(), *p);
            legend.insert(key, value_as_text(level));
            weighted += (i as f64) * p;
        }

        let confidence = choice_confidence(&probs);

        Ok((
            Answer::Score {
                score: weighted,
                legend,
                probabilities,
                confidence,
            },
            1,
        ))
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f64>>> {
        let endpoint = format!("{}/v1/embeddings", self.embed_base_url);
        let body = serde_json::json!({
            "model": self.model,
            "input": texts,
        });

        let response = self
            .client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .with_context(|| format!("Failed to POST embeddings to {}", endpoint))?;

        let status = response.status();
        let body_text = response
            .text()
            .context("Failed to read LiteRT embeddings response body")?;

        if !status.is_success() {
            let hint = litert_model_not_found_hint(status, &body_text, &self.model);
            if let Some(hint) = hint {
                bail!("{}", hint);
            }
            bail!(
                "LiteRT embeddings error (status {}): {}",
                status,
                body_text.trim()
            );
        }

        let parsed: EmbeddingsResponse = serde_json::from_str(&body_text).with_context(|| {
            format!(
                "Failed to parse LiteRT embeddings JSON (is `litert-lm serve` running?): {}",
                body_text
            )
        })?;

        let mut by_index: Vec<(usize, Vec<f64>)> = parsed
            .data
            .into_iter()
            .map(|item| (item.index, item.embedding))
            .collect();
        by_index.sort_by_key(|(idx, _)| *idx);

        if by_index.len() != texts.len() {
            bail!(
                "LiteRT returned {} embeddings but {} inputs were requested",
                by_index.len(),
                texts.len()
            );
        }

        Ok(by_index.into_iter().map(|(_, v)| v).collect())
    }
}

#[derive(Debug, Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingItem {
    index: usize,
    embedding: Vec<f64>,
}

fn litert_model_not_found_hint(
    status: reqwest::StatusCode,
    body_text: &str,
    model: &str,
) -> Option<String> {
    if status != reqwest::StatusCode::NOT_FOUND {
        return None;
    }
    let base_model = model.split(',').next().unwrap_or(model).trim();
    if !body_text.contains("not found") && !body_text.contains(base_model) {
        return None;
    }
    Some(format!(
        "LiteRT model {base_model:?} is not registered with `litert-lm serve`. \
Run `litert-lm list` and pass the ID column to `--litert-model` or `LITERT_MODEL` \
(for example `LITERT_MODEL='your-model.litertlm,cpu'`). \
Import the text bundle with `litert-lm import litert-community/embeddinggemma-2-text-270m-litert-lm` \
if you have not imported any embedding model yet."
    ))
}

/// LiteRT EmbeddingGemma 2 classification prefix (see LiteRT embedding_models guide).
pub fn classification_text(content: &str) -> String {
    format!("task: classification | text: {}", content.trim())
}

fn build_query_text(state: &Value, instructions: &Value) -> String {
    format!(
        "Context:\n{}\n\nQuestion:\n{}",
        value_as_text(state),
        value_as_text(instructions)
    )
}

fn format_option_label(key: &str, desc: &Value) -> String {
    let description = value_as_text(desc);
    if description.is_empty() {
        key.to_string()
    } else {
        format!("{} — {}", key, description)
    }
}

fn value_as_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn l2_normalize(v: &[f64]) -> Vec<f64> {
    let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm <= f64::EPSILON {
        return v.to_vec();
    }
    v.iter().map(|x| x / norm).collect()
}

pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let a = l2_normalize(a);
    let b = l2_normalize(b);
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

pub fn softmax(scores: &[f64], temperature: f64) -> Vec<f64> {
    if scores.is_empty() {
        return vec![];
    }
    let scaled: Vec<f64> = scores.iter().map(|s| s / temperature).collect();
    let max = scaled.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exp: Vec<f64> = scaled.iter().map(|s| (s - max).exp()).collect();
    let sum: f64 = exp.iter().sum();
    if sum <= f64::EPSILON {
        let uniform = 1.0 / scores.len() as f64;
        return vec![uniform; scores.len()];
    }
    exp.iter().map(|e| e / sum).collect()
}

fn choice_confidence(probs: &[f64]) -> f64 {
    if probs.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f64> = probs.to_vec();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    if sorted.len() == 1 {
        return sorted[0];
    }
    (sorted[0] - sorted[1]).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_prefix_trims_content() {
        assert_eq!(
            classification_text("  hello  "),
            "task: classification | text: hello"
        );
    }

    #[test]
    fn softmax_sums_to_one() {
        let p = softmax(&[0.2, 0.5, 0.1], 0.1);
        let sum: f64 = p.iter().sum();
        assert!((sum - 1.0).abs() < 1e-9);
        assert!(p[1] > p[0] && p[1] > p[2]);
    }

    #[test]
    fn cosine_identical_vectors() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn choice_confidence_gap() {
        assert!((choice_confidence(&[0.7, 0.2, 0.1]) - 0.5).abs() < 1e-9);
    }
}
