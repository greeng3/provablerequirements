//! Google Gemini `generateContent` adapter.
//!
//! Posts to
//! `<endpoint>/v1beta/models/{model}:generateContent?key=<API_KEY>`
//! with the native `{contents: [{role, parts: [{text}]}]}`
//! payload. System prompt rides on the top-level
//! `systemInstruction` field.
//!
//! Gemini is the one family that carries the API key on the
//! URL rather than in a header — per the Gemini convention.
//! The key never appears in logs since the adapter doesn't
//! log the request URL; classify_reqwest_error scrubs via
//! reqwest's own error formatting (which strips query strings
//! by default on `connect` / `send` failures).

use std::time::Duration;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::llm::config::ProviderFamily;
use crate::llm::provider::{
    Adapter, AdapterError, BoxFuture, PromptRequest, PromptResponse, PromptRole, PromptUsage,
};

use super::common::{HttpEndpoint, classify_reqwest_error};

const FAMILY: &str = "gemini";
const DEFAULT_ENDPOINT: &str = "https://generativelanguage.googleapis.com";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const TIMEOUT_MS: u64 = 30_000;

pub struct GeminiAdapter {
    endpoint: HttpEndpoint,
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl GeminiAdapter {
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

    fn request_url(&self, api_key: Option<&str>) -> String {
        let path = format!("/v1beta/models/{}:generateContent", self.model);
        let mut url = self.endpoint.join(&path);
        if let Some(key) = api_key {
            url.push_str("?key=");
            url.push_str(&urlencoding(key));
        }
        url
    }
}

fn urlencoding(s: &str) -> String {
    // Encode only characters disallowed in a URL query value.
    // Avoid pulling in a new dep for this one hot spot — the
    // set is small and well-defined.
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let safe = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if safe {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    contents: Vec<GeminiContent<'a>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "systemInstruction")]
    system_instruction: Option<GeminiSystemInstruction<'a>>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct GeminiContent<'a> {
    role: &'a str,
    parts: Vec<GeminiPart<'a>>,
}

#[derive(Serialize)]
struct GeminiPart<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct GeminiSystemInstruction<'a> {
    parts: Vec<GeminiPart<'a>>,
}

#[derive(Serialize)]
struct GenerationConfig {
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
    temperature: f32,
}

#[derive(Deserialize)]
struct GenerateResponse {
    candidates: Vec<Candidate>,
    #[serde(default, rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Option<CandidateContent>,
}

#[derive(Deserialize)]
struct CandidateContent {
    #[serde(default)]
    parts: Vec<CandidatePart>,
}

#[derive(Deserialize)]
struct CandidatePart {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct UsageMetadata {
    #[serde(default, rename = "promptTokenCount")]
    prompt_token_count: u32,
    #[serde(default, rename = "candidatesTokenCount")]
    candidates_token_count: u32,
}

impl Adapter for GeminiAdapter {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::Gemini
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }
    fn api_key_available(&self) -> bool {
        // Gemini always requires a key (it's part of the URL).
        self.api_key.is_some()
    }

    fn send_prompt<'a>(
        &'a self,
        req: &'a PromptRequest,
    ) -> BoxFuture<'a, Result<PromptResponse, AdapterError>> {
        Box::pin(async move {
            let api_key = self.api_key.clone();
            let contents: Vec<GeminiContent<'_>> = req
                .messages
                .iter()
                .map(|m| GeminiContent {
                    role: match m.role {
                        PromptRole::User => "user",
                        // Gemini uses "model" for assistant turns.
                        PromptRole::Assistant => "model",
                    },
                    parts: vec![GeminiPart { text: &m.content }],
                })
                .collect();
            let system_instruction = req.system.as_deref().map(|s| GeminiSystemInstruction {
                parts: vec![GeminiPart { text: s }],
            });
            let body = GenerateRequest {
                contents,
                system_instruction,
                generation_config: GenerationConfig {
                    max_output_tokens: req.max_tokens,
                    temperature: req.temperature.min(2.0),
                },
            };

            let url = self.request_url(api_key.as_deref());
            let timeout_ms = req.timeout_ms.unwrap_or(TIMEOUT_MS);
            let response = self
                .client
                .post(url)
                .timeout(Duration::from_millis(timeout_ms))
                .json(&body)
                .send()
                .await
                .map_err(|e| classify_reqwest_error(FAMILY, timeout_ms, e))?;

            let status = response.status();
            if !status.is_success() {
                return Err(status_to_error(FAMILY, status, &self.model));
            }
            let parsed: GenerateResponse =
                response.json().await.map_err(|e| AdapterError::Malformed {
                    family: FAMILY,
                    detail: e.to_string(),
                })?;
            let text = parsed
                .candidates
                .into_iter()
                .next()
                .and_then(|c| c.content)
                .map(|c| {
                    c.parts
                        .into_iter()
                        .filter_map(|p| p.text)
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();
            if text.is_empty() {
                return Err(AdapterError::Malformed {
                    family: FAMILY,
                    detail: "response had no candidates[0].content.parts[].text".into(),
                });
            }
            let usage = parsed.usage_metadata.map(|u| PromptUsage {
                input_tokens: u.prompt_token_count,
                output_tokens: u.candidates_token_count,
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
