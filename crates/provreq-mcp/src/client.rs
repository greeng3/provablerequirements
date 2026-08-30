//! Thin HTTP client over the `provreq server` REST API.
//!
//! Every MCP tool / resource / prompt handler goes through here
//! so URL construction, timeout handling, and error translation
//! live in one place. Read-only for 10c — the only verb is GET.

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::error::HandlerError;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

pub struct ProvreqClient {
    base: Url,
    http: Client,
}

impl ProvreqClient {
    pub fn new(base: Url) -> Self {
        let http = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .expect("reqwest client build");
        Self { base, http }
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    /// GET `path` on the configured base URL and return the
    /// parsed JSON body. Raises [`HandlerError::Upstream`] on
    /// HTTP errors + transport failures, including the 4xx /
    /// 5xx body if the server emitted one.
    pub async fn get_json(&self, path: &str) -> Result<Value, HandlerError> {
        let url = self.join(path)?;
        let response = self
            .http
            .get(url.clone())
            .send()
            .await
            .map_err(|e| HandlerError::Upstream(format!("GET {url}: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            let body_preview = response.text().await.unwrap_or_default();
            return Err(HandlerError::Upstream(format!(
                "GET {url} → HTTP {}: {}",
                status.as_u16(),
                truncate(&body_preview, 500)
            )));
        }
        response
            .json::<Value>()
            .await
            .map_err(|e| HandlerError::Upstream(format!("GET {url}: decode: {e}")))
    }

    /// Typed variant of [`get_json`] that walks one extra
    /// `serde_json::from_value` step — convenient when the
    /// caller wants a typed response.
    pub async fn get_typed<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
    ) -> Result<T, HandlerError> {
        let value = self.get_json(path).await?;
        serde_json::from_value::<T>(value)
            .map_err(|e| HandlerError::Upstream(format!("decode {path}: {e}")))
    }

    fn join(&self, path: &str) -> Result<Url, HandlerError> {
        // `Url::join` replaces the path component wholesale if
        // `path` begins with '/'. Our callers always pass
        // absolute-ish paths like `/api/projects`, so that's the
        // intended behaviour.
        self.base
            .join(path)
            .map_err(|e| HandlerError::Internal(format!("url join '{path}': {e}")))
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        // Stop at a char boundary so slicing doesn't panic.
        let mut end = max;
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        &s[..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_client(base: &str) -> ProvreqClient {
        ProvreqClient::new(Url::parse(base).unwrap())
    }

    #[tokio::test]
    async fn get_json_returns_parsed_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/ping"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ok": true
            })))
            .mount(&server)
            .await;

        let client = make_client(&server.uri());
        let body = client.get_json("/api/ping").await.unwrap();
        assert_eq!(body["ok"], true);
    }

    #[tokio::test]
    async fn get_json_propagates_http_errors_with_body_preview() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/missing"))
            .respond_with(ResponseTemplate::new(404).set_body_string("{\"error\":\"not here\"}"))
            .mount(&server)
            .await;

        let client = make_client(&server.uri());
        let err = client.get_json("/api/missing").await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("HTTP 404"));
        assert!(msg.contains("not here"));
    }

    #[tokio::test]
    async fn truncate_handles_non_ascii_char_boundary() {
        // 500-byte max, input that cuts through a multi-byte
        // char — must not panic.
        let mut s = String::new();
        for _ in 0..300 {
            s.push('🦀');
        }
        let out = truncate(&s, 500);
        assert!(out.len() <= 500);
    }
}
