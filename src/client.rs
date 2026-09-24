//! HTTP client for communicating with the TypeSafe AI System One API.

use crate::types::{SystemOneRequest, SystemOneResponse};
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use std::time::Duration;

/// Client for dispatching System One evaluation requests.
pub struct TypeSafeClient {
    api_key: String,
    base_url: String,
    client: Client,
}

impl TypeSafeClient {
    /// Creates a new `TypeSafeClient` with API key, optional base URL, and timeout.
    pub fn new(api_key: String, base_url: Option<String>, timeout_secs: u64) -> Result<Self> {
        if api_key.trim().is_empty() {
            bail!("TypeSafe API key is required. Set TYPESAFE_API_KEY environment variable or pass --api-key.");
        }

        let base_url = base_url
            .unwrap_or_else(|| "https://api.typesafe.ai".to_string())
            .trim_end_matches('/')
            .to_string();

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            api_key,
            base_url,
            client,
        })
    }

    /// Evaluates a `SystemOneRequest` synchronously against `/v1/systemone`.
    pub fn evaluate(&self, request: &SystemOneRequest) -> Result<SystemOneResponse> {
        let endpoint = format!("{}/v1/systemone", self.base_url);

        let response = self
            .client
            .post(&endpoint)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .with_context(|| {
                format!("Failed to send request to TypeSafe endpoint: {}", endpoint)
            })?;

        let status = response.status();
        let body_text = response
            .text()
            .context("Failed to read response body from TypeSafe API")?;

        if !status.is_success() {
            bail!(
                "TypeSafe API error (status {}): {}",
                status,
                body_text.trim()
            );
        }

        let parsed: SystemOneResponse = serde_json::from_str(&body_text)
            .with_context(|| format!("Failed to parse TypeSafe response JSON: {}", body_text))?;

        Ok(parsed)
    }
}
