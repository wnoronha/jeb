//! HTTP client for communicating with the TypeSafe AI System One API.

use crate::types::{ApiErrorResponse, SystemOneRequest, SystemOneResponse};
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
        if timeout_secs == 0 {
            bail!("Timeout must be greater than zero seconds.");
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
        let user_agent = format!("jeb/{}", env!("CARGO_PKG_VERSION"));

        let response = self
            .client
            .post(&endpoint)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .header("User-Agent", user_agent)
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
                parse_api_error_message(&body_text)
            );
        }

        let parsed: SystemOneResponse = serde_json::from_str(&body_text)
            .with_context(|| format!("Failed to parse TypeSafe response JSON: {}", body_text))?;

        Ok(parsed)
    }
}

/// Extracts a concise message from an API error body.
fn parse_api_error_message(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "empty error body".to_string();
    }
    if let Ok(parsed) = serde_json::from_str::<ApiErrorResponse>(trimmed) {
        if let Some(message) = parsed.message {
            let message = message.trim();
            if !message.is_empty() {
                return message.to_string();
            }
        }
        if let Some(error) = parsed.error {
            match error {
                serde_json::Value::String(text) => {
                    let text = text.trim();
                    if !text.is_empty() {
                        return text.to_string();
                    }
                }
                other => {
                    return other.to_string();
                }
            }
        }
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_rejects_zero_timeout() {
        let result = TypeSafeClient::new(
            "dummy-key".to_string(),
            Some("https://api.typesafe.ai".to_string()),
            0,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_api_error_prefers_message() {
        let body = r#"{"message": "Invalid key", "error": "auth_failed"}"#;
        assert_eq!(parse_api_error_message(body), "Invalid key");
    }

    #[test]
    fn test_parse_api_error_falls_back_to_error_field() {
        let body = r#"{"error": "rate_limited"}"#;
        assert_eq!(parse_api_error_message(body), "rate_limited");
    }

    #[test]
    fn test_parse_api_error_handles_empty_body() {
        assert_eq!(parse_api_error_message("   "), "empty error body");
    }

    #[test]
    fn test_parse_api_error_keeps_plain_text() {
        assert_eq!(parse_api_error_message("  boom  "), "boom");
    }
}
