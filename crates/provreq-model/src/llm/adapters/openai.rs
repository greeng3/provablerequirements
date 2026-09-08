//! OpenAI-compatible Chat Completions adapter.
//!
//! Speaks the `POST /v1/chat/completions` dialect that
//! OpenAI, Azure OpenAI, Ollama, LMStudio, vLLM, llama.cpp,
//! OpenRouter, and LiteLLM all implement — so operators
//! point `endpoint` at any of them and the same adapter
//! works. The adapter always passes `system` through as a
//! `{role: "system"}` message when present.

use std::time::Duration;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::llm::config::ProviderFamily;
use crate::llm::provider::{
    Adapter, AdapterError, BoxFuture, PromptRequest, PromptResponse, PromptRole, PromptUsage,
};

use super::common::{HttpEndpoint, classify_reqwest_error};

const FAMILY: &str = "openai-compatible";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const TIMEOUT_MS: u64 = 30_000;

pub struct OpenAiCompatibleAdapter {
    endpoint: HttpEndpoint,
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl OpenAiCompatibleAdapter {
    pub fn new(endpoint: String, model: String, api_key: Option<String>) -> Self {
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
        self.endpoint.join("/v1/chat/completions")
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<ChatUsage>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

impl Adapter for OpenAiCompatibleAdapter {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::OpenaiCompatible
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }
    fn api_key_available(&self) -> bool {
        // OpenAI-compatible covers both keyed (api.openai.com,
        // OpenRouter) and keyless (local Ollama, LMStudio)
        // deployments. Without a way to know which the operator
        // configured, optimistically report "available" — the
        // chain learns the truth from the first send_prompt
        // failure.
        true
    }

    fn send_prompt<'a>(
        &'a self,
        req: &'a PromptRequest,
    ) -> BoxFuture<'a, Result<PromptResponse, AdapterError>> {
        Box::pin(async move {
            let api_key = self.api_key.clone();
            let mut messages = Vec::with_capacity(req.messages.len() + 1);
            if let Some(sys) = req.system.as_deref() {
                messages.push(ChatMessage {
                    role: "system",
                    content: sys,
                });
            }
            for m in &req.messages {
                messages.push(ChatMessage {
                    role: match m.role {
                        PromptRole::User => "user",
                        PromptRole::Assistant => "assistant",
                    },
                    content: &m.content,
                });
            }
            let body = ChatRequest {
                model: &self.model,
                messages,
                max_tokens: req.max_tokens,
                temperature: req.temperature,
            };

            let timeout_ms = req.timeout_ms.unwrap_or(TIMEOUT_MS);
            let mut builder = self
                .client
                .post(self.request_url())
                .timeout(Duration::from_millis(timeout_ms))
                .json(&body);
            if let Some(key) = &api_key {
                builder = builder.bearer_auth(key);
            }
            let response = builder
                .send()
                .await
                .map_err(|e| classify_reqwest_error(FAMILY, timeout_ms, e))?;

            let status = response.status();
            if !status.is_success() {
                return Err(status_to_error(FAMILY, status, &self.model));
            }
            let parsed: ChatResponse =
                response.json().await.map_err(|e| AdapterError::Malformed {
                    family: FAMILY,
                    detail: e.to_string(),
                })?;
            let text = parsed
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message.content)
                .ok_or(AdapterError::Malformed {
                    family: FAMILY,
                    detail: "response had no choices[0].message.content".into(),
                })?;
            let usage = parsed.usage.map(|u| PromptUsage {
                input_tokens: u.prompt_tokens,
                output_tokens: u.completion_tokens,
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
