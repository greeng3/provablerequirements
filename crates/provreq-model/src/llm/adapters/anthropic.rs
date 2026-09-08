//! Anthropic Messages API adapter.
//!
//! Posts to `<endpoint>/v1/messages` with the native
//! `x-api-key` + `anthropic-version` headers. `system` is
//! a native top-level field; user/assistant messages alternate
//! in `messages`.

use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::llm::config::ProviderFamily;
use crate::llm::provider::{
    Adapter, AdapterError, BoxFuture, PromptRequest, PromptResponse, PromptRole, PromptUsage,
};

use super::common::{HttpEndpoint, classify_reqwest_error};

const FAMILY: &str = "anthropic";
const DEFAULT_ENDPOINT: &str = "https://api.anthropic.com";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const TIMEOUT_MS: u64 = 30_000;
const API_VERSION: &str = "2023-06-01";

pub struct AnthropicAdapter {
    endpoint: HttpEndpoint,
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl AnthropicAdapter {
    pub fn new(endpoint: Option<String>, model: String, api_key: Option<String>) -> Self {
        let endpoint = endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_owned());
        let client = reqwest::Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .expect("reqwest client build");
        Self {
            endpoint: HttpEndpoint::new(endpoint),
            model,
            api_key,
            client,
        }
    }

    fn request_url(&self) -> String {
        self.endpoint.join("/v1/messages")
    }
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<MessageEntry<'a>>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize)]
struct MessageEntry<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<MessageContent>,
    #[serde(default)]
    usage: Option<MessagesUsage>,
}

#[derive(Deserialize)]
struct MessageContent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct MessagesUsage {
    input_tokens: u32,
    output_tokens: u32,
}

impl Adapter for AnthropicAdapter {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::Anthropic
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }
    fn api_key_available(&self) -> bool {
        // Anthropic always requires a key; report unavailable
        // when none is configured so the chain skips this slot
        // before issuing a doomed request.
        self.api_key.is_some()
    }

    fn send_prompt<'a>(
        &'a self,
        req: &'a PromptRequest,
    ) -> BoxFuture<'a, Result<PromptResponse, AdapterError>> {
        Box::pin(async move {
            let api_key = self.api_key.clone();
            let messages: Vec<MessageEntry<'_>> = req
                .messages
                .iter()
                .map(|m| MessageEntry {
                    role: match m.role {
                        PromptRole::User => "user",
                        PromptRole::Assistant => "assistant",
                    },
                    content: &m.content,
                })
                .collect();
            let body = MessagesRequest {
                model: &self.model,
                system: req.system.as_deref(),
                messages,
                max_tokens: req.max_tokens,
                temperature: req.temperature.min(1.0),
            };

            let mut headers = HeaderMap::new();
            headers.insert(
                HeaderName::from_static("anthropic-version"),
                HeaderValue::from_static(API_VERSION),
            );
            if let Some(key) = &api_key {
                let mut hv = HeaderValue::from_str(key).map_err(|_| AdapterError::Auth {
                    family: FAMILY,
                    detail: "api key contains invalid header characters".into(),
                })?;
                hv.set_sensitive(true);
                headers.insert(HeaderName::from_static("x-api-key"), hv);
            }

            let timeout_ms = req.timeout_ms.unwrap_or(TIMEOUT_MS);
            let response = self
                .client
                .post(self.request_url())
                .timeout(Duration::from_millis(timeout_ms))
                .headers(headers)
                .json(&body)
                .send()
                .await
                .map_err(|e| classify_reqwest_error(FAMILY, timeout_ms, e))?;

            let status = response.status();
            if !status.is_success() {
                return Err(status_to_error(FAMILY, status, &self.model));
            }
            let parsed: MessagesResponse =
                response.json().await.map_err(|e| AdapterError::Malformed {
                    family: FAMILY,
                    detail: e.to_string(),
                })?;
            let text = parsed
                .content
                .into_iter()
                .filter(|c| c.kind == "text")
                .filter_map(|c| c.text)
                .collect::<Vec<_>>()
                .join("");
            if text.is_empty() {
                return Err(AdapterError::Malformed {
                    family: FAMILY,
                    detail: "response had no text content parts".into(),
                });
            }
            let usage = parsed.usage.map(|u| PromptUsage {
                input_tokens: u.input_tokens,
                output_tokens: u.output_tokens,
            });
            Ok(PromptResponse { text, usage })
        })
    }
}

fn status_to_error(family: &'static str, status: StatusCode, model: &str) -> AdapterError {
    match status.as_u16() {
        401 | 403 => AdapterError::Auth {
            family,
            detail: format!("HTTP {}", status.as_u16()),
        },
        404 => AdapterError::ModelNotFound {
            family,
            model: model.to_owned(),
        },
        429 => AdapterError::RateLimited { family },
        500..=599 => AdapterError::ServerError {
            family,
            status: status.as_u16(),
        },
        _ => AdapterError::Malformed {
            family,
            detail: format!("unexpected HTTP {}", status.as_u16()),
        },
    }
}
