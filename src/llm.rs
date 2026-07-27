//! LLM bulk pre-sort triage classifier (R-triage-1 primary flow). Multi-provider
//! and configurable: the operator picks a provider, endpoint, and model in
//! `provreq.yml`; the API key (if any) comes only from a named environment
//! variable, never the file. The classifier's output is advisory — the operator
//! still reviews and confirms/overrides.
//!
//! The single network call is factored behind [`LlmBackend`] so prompt-building
//! and response-parsing are unit-tested with a stub, no live endpoint needed.
//!
//! Implements: REQ012 (LLM bulk pre-sort classifier, provider-configurable)

use crate::source::{Classification, Item};
use crate::triage::Classifier;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// Wire protocol of the configured endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    /// OpenAI-compatible `/chat/completions` — covers OpenAI, Ollama, and most
    /// local gateways. `base_url` includes the version segment
    /// (`https://api.openai.com/v1`, `http://localhost:11434/v1`).
    OpenaiCompatible,
    /// Anthropic `/v1/messages`. `base_url` is the host root
    /// (`https://api.anthropic.com`).
    Anthropic,
}

/// LLM configuration, read from the `llm:` block of `provreq.yml`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LlmConfig {
    pub provider: Provider,
    pub base_url: String,
    pub model: String,
    /// Name of the environment variable holding the API key. Omit for keyless
    /// endpoints (Ollama). The key itself never lives in the config file.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Per-request timeout in seconds. Bounds a hung endpoint (a local model that
    /// stalls) so a triage/translate/draft call fails loudly instead of blocking
    /// forever. Defaults to [`DEFAULT_TIMEOUT_SECS`] when omitted — generous, so it
    /// catches a true hang without cutting off legitimately-slow local generation.
    #[serde(default = "default_timeout_secs")]
    pub timeout_seconds: u64,
    /// How many requirements go into one bulk pre-sort request (REQ054). Bounding is per request,
    /// so this is what makes `timeout_seconds` a bound the operator can reason about: it is the
    /// unit of work a failure can cost, and the unit of prompt that has to fit a context window.
    /// Tune it down for a slow local model, up for a fast hosted one.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

/// Default per-request LLM timeout: 10 minutes. Long enough that a slow local model
/// finishing a large completion is never cut off, short enough that a wedged endpoint
/// does not block a run indefinitely.
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;

fn default_timeout_secs() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

/// Default bulk pre-sort batch size. Chosen so the default batch finishes inside
/// [`DEFAULT_TIMEOUT_SECS`] on a slow endpoint rather than on a fast one: measured against
/// `qwen3:32b` on a local Ollama, five requirements classify in 5m42s — comfortably inside the ten
/// minute bound, where ten requirements would not have been.
pub const DEFAULT_BATCH_SIZE: usize = 5;

fn default_batch_size() -> usize {
    DEFAULT_BATCH_SIZE
}

/// Anthropic requires an explicit output cap; generous enough for a JSON array
/// over a whole backlog.
const ANTHROPIC_MAX_TOKENS: u32 = 4096;

/// Read the optional `llm:` block from a companion tree's manifest. `None` means
/// the operator has not configured an LLM — triage falls back to the prose floor.
pub fn load_config(companion_root: &Path) -> Result<Option<LlmConfig>> {
    let path = companion_root.join(crate::adopt::MANIFEST_FILE);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    #[derive(serde::Deserialize)]
    struct ManifestLlm {
        #[serde(default)]
        llm: Option<LlmConfig>,
    }
    let manifest: ManifestLlm =
        serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(manifest.llm)
}

/// The single network call, factored out for offline testing.
pub trait LlmBackend {
    fn complete(&self, prompt: &str) -> impl std::future::Future<Output = Result<String>> + Send;
}

/// The production backend: a provider-aware HTTP call.
pub struct HttpBackend {
    config: LlmConfig,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl HttpBackend {
    /// Build from config, resolving the API key from its named env var. Errors if
    /// the named variable is missing (fail fast, no silent keyless downgrade).
    pub fn from_config(config: LlmConfig) -> Result<Self> {
        let api_key =
            match &config.api_key_env {
                Some(var) => Some(std::env::var(var).with_context(|| {
                    format!("environment variable {var} (LLM API key) is not set")
                })?),
                None => None,
            };
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()
            .context("building the LLM HTTP client")?;
        Ok(Self {
            config,
            api_key,
            http,
        })
    }

    async fn complete_openai(&self, prompt: &str) -> Result<String> {
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [{ "role": "user", "content": prompt }],
            "temperature": 0,
            "stream": false,
        });
        let mut req = self.http.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        extract_openai(&send_json(req).await?)
    }

    async fn complete_anthropic(&self, prompt: &str) -> Result<String> {
        let key = self
            .api_key
            .as_deref()
            .context("anthropic provider requires api_key_env")?;
        let url = format!("{}/v1/messages", self.config.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": ANTHROPIC_MAX_TOKENS,
            "temperature": 0,
            "messages": [{ "role": "user", "content": prompt }],
        });
        let req = self
            .http
            .post(&url)
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .json(&body);
        extract_anthropic(&send_json(req).await?)
    }
}

/// Pull the assistant text out of an OpenAI-compatible chat response (pure).
///
/// A present-but-**empty** `content` is a failed response, not an empty answer (REQ052): providers
/// can return `""` — a reasoning model that spent its budget thinking is one way — and passing that
/// on as `Ok("")` walks straight into reporting a fabricated classification downstream.
fn extract_openai(json: &serde_json::Value) -> Result<String> {
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .context("LLM response missing choices[0].message.content")?;
    reject_empty(content)
}

/// Pull the assistant text out of an Anthropic messages response (pure).
fn extract_anthropic(json: &serde_json::Value) -> Result<String> {
    let text = json["content"][0]["text"]
        .as_str()
        .context("LLM response missing content[0].text")?;
    reject_empty(text)
}

fn reject_empty(text: &str) -> Result<String> {
    if text.trim().is_empty() {
        bail!("the LLM returned empty content — a reply with nothing in it is a failed request, not an answer");
    }
    Ok(text.to_string())
}

impl LlmBackend for HttpBackend {
    async fn complete(&self, prompt: &str) -> Result<String> {
        match self.config.provider {
            Provider::OpenaiCompatible => self.complete_openai(prompt).await,
            Provider::Anthropic => self.complete_anthropic(prompt).await,
        }
    }
}

/// Send a request and parse a JSON body, surfacing the endpoint's own error body
/// (the operator is the user here, so a detailed message helps rather than leaks).
async fn send_json(req: reqwest::RequestBuilder) -> Result<serde_json::Value> {
    let resp = req
        .send()
        .await
        .context("sending request to the LLM endpoint")?;
    let status = resp.status();
    let text = resp.text().await.context("reading the LLM response body")?;
    if !status.is_success() {
        bail!("LLM endpoint returned {status}: {text}");
    }
    serde_json::from_str(&text).context("parsing the LLM response as JSON")
}

/// The bulk pre-sort classifier. Generic over its backend so tests inject a stub.
pub struct LlmClassifier<B: LlmBackend> {
    backend: B,
}

impl<B: LlmBackend> LlmClassifier<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }
}

impl<B: LlmBackend + Send + Sync> Classifier for LlmClassifier<B> {
    async fn classify(&self, items: &[Item]) -> Result<Vec<Classification>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let raw = self.backend.complete(&build_prompt(items)).await?;
        parse_buckets(&raw, items)
    }
}

const PROMPT_HEADER: &str = "\
You are triaging software requirements for a provable-requirements tool. Classify \
each requirement into exactly one bucket:
- formalizable-now: makes a claim provable NOW against code by a deductive verifier \
(a definite truth value a prover can discharge).
- falsifiable-only: checkable or monitorable at runtime (safety, or timing bounded \
by a deadline) but only falsifiable from finite observations, never proved.
- stays-prose: too vague to carry a definite truth value as written.

Requirements:
";

const PROMPT_FOOTER: &str = "\n\nRespond with ONLY a JSON array, one object per \
requirement, no prose and no code fences: \
[{\"id\": \"<id>\", \"bucket\": \"formalizable-now|falsifiable-only|stays-prose\"}]";

/// Build the classification prompt (pure).
fn build_prompt(items: &[Item]) -> String {
    let mut prompt = String::from(PROMPT_HEADER);
    for item in items {
        // Flatten prose to a single line so the list stays unambiguous.
        prompt.push_str(&format!(
            "- {}: {}\n",
            item.id,
            item.text.replace('\n', " ")
        ));
    }
    prompt.push_str(PROMPT_FOOTER);
    prompt
}

/// Map the model's reply back to one bucket per input item, in order.
///
/// Any item the model omits or mislabels defaults to `stays-prose` — the honest floor for *that
/// item*, which claims nothing about the requirement and leaves the work visible.
///
/// A reply carrying **no** usable assignment is a different event: the request failed, and the
/// same floor applied to every item stops being a floor and becomes a fabricated classification —
/// indistinguishable from a model that read the whole backlog and judged it all unformalizable
/// (REQ052). So that case is an error, not a result. Pure.
fn parse_buckets(raw: &str, items: &[Item]) -> Result<Vec<Classification>> {
    let map = parse_assignments(raw);
    if map.is_empty() {
        bail!(
            "the model returned no usable classification — expected a JSON array of \
             {{id, bucket}}, got: {}",
            excerpt(raw)
        );
    }
    Ok(items
        .iter()
        .map(|i| {
            map.get(&i.id)
                .copied()
                .unwrap_or(Classification::StaysProse)
        })
        .collect())
}

/// A short, single-line rendering of a reply for an error message — enough for the operator to
/// recognise a refusal or a truncation without pasting a page of model output into the terminal.
fn excerpt(raw: &str) -> String {
    const MAX: usize = 200;
    let flat = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return "an empty reply".to_string();
    }
    match flat.char_indices().nth(MAX) {
        Some((cut, _)) => format!("`{}…`", &flat[..cut]),
        None => format!("`{flat}`"),
    }
}

fn parse_assignments(raw: &str) -> BTreeMap<String, Classification> {
    #[derive(serde::Deserialize)]
    struct Assignment {
        id: String,
        bucket: String,
    }
    let json = extract_json_array(raw).unwrap_or(raw);
    let parsed: Vec<Assignment> = serde_json::from_str(json).unwrap_or_default();
    parsed
        .into_iter()
        .filter_map(|a| Classification::parse(&a.bucket).map(|c| (a.id, c)))
        .collect()
}

/// Extract the first `[` … last `]` span, tolerating code fences or prose the
/// model wraps around the JSON.
fn extract_json_array(raw: &str) -> Option<&str> {
    let start = raw.find('[')?;
    let end = raw.rfind(']')?;
    (end > start).then(|| &raw[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, text: &str) -> Item {
        Item {
            id: id.into(),
            text: text.into(),
            revision: "r".into(),
            title: None,
            verification_hint: None,
        }
    }

    struct StubBackend {
        reply: String,
    }
    impl LlmBackend for StubBackend {
        async fn complete(&self, _prompt: &str) -> Result<String> {
            Ok(self.reply.clone())
        }
    }

    // Verifies: REQ012 — the classifier maps the model's buckets onto items by id.
    #[tokio::test]
    async fn classify_maps_buckets_by_id() {
        let items = [
            item("REQ001", "shall respond within 200ms"),
            item("REQ002", "be nice"),
        ];
        let backend = StubBackend {
            reply: r#"[{"id":"REQ001","bucket":"formalizable-now"},
                       {"id":"REQ002","bucket":"stays-prose"}]"#
                .into(),
        };
        let buckets = LlmClassifier::new(backend).classify(&items).await.unwrap();
        assert_eq!(
            buckets,
            vec![Classification::FormalizableNow, Classification::StaysProse]
        );
    }

    // Verifies: REQ012 — omitted or unknown buckets default to the prose floor,
    // and the output length always matches the input (never crashes triage).
    #[tokio::test]
    async fn classify_defaults_missing_and_unknown_to_prose() {
        let items = [item("A", "x"), item("B", "y"), item("C", "z")];
        let backend = StubBackend {
            // A mislabeled, B present, C omitted entirely.
            reply: r#"[{"id":"A","bucket":"nonsense"},{"id":"B","bucket":"falsifiable-only"}]"#
                .into(),
        };
        let buckets = LlmClassifier::new(backend).classify(&items).await.unwrap();
        assert_eq!(
            buckets,
            vec![
                Classification::StaysProse,
                Classification::FalsifiableOnly,
                Classification::StaysProse
            ]
        );
    }

    // Verifies: REQ052 — a reply that yields NO usable assignment is a failed request, not a
    // classification. Before this, every one of these produced a full, confident all-prose
    // classification of the whole backlog with the model's name printed above it — the exact
    // overclaim the tool exists to prevent, and the worst kind, because it fabricates content
    // rather than status.
    #[tokio::test]
    async fn a_reply_with_no_usable_assignment_is_an_error() {
        let items = [item("A", "x"), item("B", "y")];
        for (label, reply) in [
            ("a refusal", "I cannot classify these requirements."),
            ("prose instead of a list", "They all look like prose to me."),
            ("truncated json", r#"[{"id":"A","bucket":"formaliz"#),
            ("an empty reply", ""),
            ("whitespace only", "   \n  "),
            ("an empty array", "[]"),
            (
                "every bucket unknown",
                r#"[{"id":"A","bucket":"???"},{"id":"B","bucket":"maybe"}]"#,
            ),
        ] {
            let backend = StubBackend {
                reply: reply.into(),
            };
            let err = LlmClassifier::new(backend)
                .classify(&items)
                .await
                .expect_err(&format!("{label} must not pass as a classification"));
            let msg = format!("{err:#}");
            assert!(
                msg.contains("classif"),
                "{label}: the error must name what failed, got: {msg}"
            );
        }
    }

    // Verifies: REQ052 — the distinction is partial-vs-none, not perfect-vs-imperfect. One usable
    // assignment is an answer, and the items the model skipped still take the conservative floor.
    #[tokio::test]
    async fn one_usable_assignment_is_still_an_answer() {
        let items = [item("A", "x"), item("B", "y")];
        let backend = StubBackend {
            reply: r#"garbage before [{"id":"B","bucket":"formalizable-now"}] and after"#.into(),
        };
        let buckets = LlmClassifier::new(backend).classify(&items).await.unwrap();
        assert_eq!(
            buckets,
            vec![Classification::StaysProse, Classification::FormalizableNow]
        );
    }

    // Verifies: REQ052 — a present-but-empty `content` field is a failed response, not an empty
    // answer. `extract_openai` reads a field that a provider can legitimately return as "", and
    // returning `Ok("")` from it walks straight into the fabrication above.
    #[test]
    fn empty_assistant_content_is_a_failed_response() {
        let err = extract_openai(&serde_json::json!({
            "choices": [{ "message": { "role": "assistant", "content": "" } }]
        }))
        .expect_err("empty content is not an answer");
        assert!(format!("{err:#}").contains("empty"), "{err:#}");
    }

    // Verifies: REQ012 — a reply wrapped in a code fence still parses.
    #[tokio::test]
    async fn classify_tolerates_code_fenced_json() {
        let items = [item("A", "x")];
        let backend = StubBackend {
            reply: "Here you go:\n```json\n[{\"id\":\"A\",\"bucket\":\"formalizable-now\"}]\n```"
                .into(),
        };
        let buckets = LlmClassifier::new(backend).classify(&items).await.unwrap();
        assert_eq!(buckets, vec![Classification::FormalizableNow]);
    }

    // Verifies: REQ012 — the provider response shapes are read from the right
    // fields (OpenAI/Ollama chat vs Anthropic messages).
    #[test]
    fn extracts_provider_response_shapes() {
        let openai = serde_json::json!({
            "choices": [{ "message": { "content": "hello" } }]
        });
        assert_eq!(extract_openai(&openai).unwrap(), "hello");
        assert!(extract_openai(&serde_json::json!({"choices": []})).is_err());

        let anthropic = serde_json::json!({
            "content": [{ "type": "text", "text": "hi" }]
        });
        assert_eq!(extract_anthropic(&anthropic).unwrap(), "hi");
        assert!(extract_anthropic(&serde_json::json!({"content": []})).is_err());
    }

    #[test]
    fn prompt_lists_every_item_id() {
        let prompt = build_prompt(&[item("REQ001", "a"), item("REQ042", "b")]);
        assert!(prompt.contains("REQ001"));
        assert!(prompt.contains("REQ042"));
        assert!(prompt.contains("formalizable-now"));
    }

    #[test]
    fn load_config_reads_llm_block() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(crate::adopt::MANIFEST_FILE),
            "schema: 1\nllm:\n  provider: openai-compatible\n  base_url: http://localhost:11434/v1\n  model: llama3\n",
        )
        .unwrap();
        let cfg = load_config(tmp.path()).unwrap().unwrap();
        assert_eq!(cfg.provider, Provider::OpenaiCompatible);
        assert_eq!(cfg.model, "llama3");
        assert_eq!(cfg.api_key_env, None);
        // Omitted timeout falls back to the generous default.
        assert_eq!(cfg.timeout_seconds, DEFAULT_TIMEOUT_SECS);
        assert_eq!(cfg.batch_size, DEFAULT_BATCH_SIZE);
    }

    // Verifies: REQ042 — an explicit `timeout_seconds` overrides the default, and the client
    // builds with it (a bad timeout would fail the build here).
    #[test]
    fn load_config_honors_explicit_timeout() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(crate::adopt::MANIFEST_FILE),
            "schema: 1\nllm:\n  provider: openai-compatible\n  base_url: http://localhost:11434/v1\n  model: m\n  timeout_seconds: 42\n",
        )
        .unwrap();
        let cfg = load_config(tmp.path()).unwrap().unwrap();
        assert_eq!(cfg.timeout_seconds, 42);
        assert!(HttpBackend::from_config(cfg).is_ok());
    }

    #[test]
    fn load_config_absent_llm_block_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(crate::adopt::MANIFEST_FILE), "schema: 1\n").unwrap();
        assert!(load_config(tmp.path()).unwrap().is_none());
    }
}
