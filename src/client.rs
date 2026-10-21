//! HTTP client for decision evaluation APIs (TypeSafe System One, OpenRouter Decisions).

use crate::types::{
    parse_model_catalog, ApiErrorResponse, ModelInfo, OpenRouterModelsResponse, Provider,
    SystemOneRequest, SystemOneResponse,
};
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use std::time::Duration;

/// Referer header sent with OpenRouter requests.
const OPENROUTER_REFERER: &str = "https://github.com/typesafe-ai/jeb";

/// Client for dispatching decision evaluation requests.
pub struct DecisionsClient {
    api_key: String,
    base_url: String,
    provider: Provider,
    client: Client,
}

impl DecisionsClient {
    /// Creates a new `DecisionsClient` with API key, optional base URL, and timeout.
    pub fn new(
        api_key: String,
        base_url: Option<String>,
        timeout_secs: u64,
        provider: Provider,
    ) -> Result<Self> {
        if api_key.trim().is_empty() {
            bail!(
                "API key is required. Set {} environment variable or pass --api-key.",
                provider.api_key_env()
            );
        }
        if timeout_secs == 0 {
            bail!("Timeout must be greater than zero seconds.");
        }

        let base_url = base_url
            .unwrap_or_else(|| provider.default_base_url().to_string())
            .trim_end_matches('/')
            .to_string();

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            api_key,
            base_url,
            provider,
            client,
        })
    }

    /// Full evaluation endpoint URL for the configured provider.
    pub fn endpoint(&self) -> String {
        format!("{}{}", self.base_url, self.provider.decisions_path())
    }

    /// Evaluates a `SystemOneRequest` synchronously against the provider endpoint.
    pub fn evaluate(&self, request: &SystemOneRequest) -> Result<SystemOneResponse> {
        let endpoint = self.endpoint();
        let user_agent = format!("jeb/{}", env!("CARGO_PKG_VERSION"));

        let mut call = self
            .client
            .post(&endpoint)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .header("User-Agent", user_agent);
        if self.provider == Provider::Openrouter {
            call = call
                .header("HTTP-Referer", OPENROUTER_REFERER)
                .header("X-Title", "jeb");
        }

        let response = call.json(request).send().with_context(|| {
            format!("Failed to send request to decision endpoint: {}", endpoint)
        })?;

        let status = response.status();
        let body_text = response
            .text()
            .context("Failed to read response body from decision API")?;

        if !status.is_success() {
            bail!(
                "Decision API error (status {}): {}",
                status,
                parse_api_error_message(&body_text)
            );
        }

        let parsed: SystemOneResponse = serde_json::from_str(&body_text)
            .with_context(|| format!("Failed to parse decision response JSON: {}", body_text))?;

        Ok(parsed)
    }
}

/// Fetches the OpenRouter decision model catalog.
pub fn fetch_openrouter_models(
    base_url: &str,
    api_key: Option<&str>,
    timeout_secs: u64,
) -> Result<Vec<ModelInfo>> {
    if timeout_secs == 0 {
        bail!("Timeout must be greater than zero seconds.");
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .context("Failed to build HTTP client")?;
    let url = format!(
        "{}/api/v1/models?output_modalities=decisions",
        base_url.trim_end_matches('/')
    );
    let mut call = client
        .get(&url)
        .header("Content-Type", "application/json")
        .header("User-Agent", format!("jeb/{}", env!("CARGO_PKG_VERSION")));
    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        call = call.bearer_auth(key);
    }
    let response = call
        .send()
        .with_context(|| format!("Failed to fetch model catalog: {}", url))?;
    let status = response.status();
    let body = response
        .text()
        .context("Failed to read model catalog response")?;
    if !status.is_success() {
        bail!(
            "Model catalog error (status {}): {}",
            status,
            parse_api_error_message(&body)
        );
    }
    let parsed: OpenRouterModelsResponse =
        serde_json::from_str(&body).context("Failed to parse model catalog JSON")?;
    Ok(parse_model_catalog(&parsed))
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
        let result = DecisionsClient::new(
            "dummy-key".to_string(),
            Some("https://api.typesafe.ai".to_string()),
            0,
            Provider::Typesafe,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_endpoint_per_provider() {
        let typesafe =
            DecisionsClient::new("dummy-key".to_string(), None, 30, Provider::Typesafe).unwrap();
        assert_eq!(typesafe.endpoint(), "https://api.typesafe.ai/v1/systemone");
        let openrouter =
            DecisionsClient::new("dummy-key".to_string(), None, 30, Provider::Openrouter).unwrap();
        assert_eq!(
            openrouter.endpoint(),
            "https://openrouter.ai/api/alpha/decisions"
        );
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
