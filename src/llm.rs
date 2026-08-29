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
use anyhow::{Context, Result, anyhow, bail};
use reqforge_model::llm::{LlmRuntime, ProviderConfig, ProviderFamily};
// Re-exported so provreq's LLM features (and their test stubs) build a request and read a response
// without depending on `reqforge_model` directly — the seam is `crate::llm`.
pub use reqforge_model::llm::{PromptMessage, PromptRequest, PromptResponse, PromptRole};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

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
    /// Which fields an environment variable replaced (#225). Never read from or written to the
    /// manifest — it is a fact about *this run*, not about the subject, which is the whole reason
    /// the override exists.
    ///
    /// Carried so a caller printing "with `<model>` via `<url>`" can say where those came from. An
    /// export set months ago and forgotten is otherwise invisible: the banner would name an
    /// endpoint the committed file does not, and nothing would explain the difference.
    #[serde(skip)]
    pub overridden: Vec<&'static str>,
}

/// Environment overrides for the two `llm:` fields that are **machine topology, not project
/// configuration** (#225): which host answers, and which model it serves.
///
/// The manifest is committed and shared, so an operator pointing provreq at the box actually on
/// their network had to edit a tracked file and remember to revert it — a dirty-working-tree trap
/// on a file that also carries `environment:`, the doorstop paths and `tla.constants`, all of which
/// genuinely belong in the repo. Same split `WEBDRIVER_URL` (#245), `MONPOLY_BIN` (#233) and
/// `api_key_env` already draw, and the same one the manifest's own comment conceded.
pub const BASE_URL_VAR: &str = "PROVREQ_LLM_BASE_URL";
pub const MODEL_VAR: &str = "PROVREQ_LLM_MODEL";

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
    Ok(manifest.llm.map(|config| {
        apply_overrides(
            config,
            env_override(BASE_URL_VAR).as_deref(),
            env_override(MODEL_VAR).as_deref(),
        )
    }))
}

/// The `llm:` block with any environment override applied — the one place the two sources are
/// reconciled, so every caller (`triage`, `draft --translate`, `verify --draft-semantic`) sees the
/// same answer without any of them knowing an override exists.
///
/// Pure over both inputs, so the precedence rule is testable without touching the process
/// environment — the same split [`crate::ui`] draws for `WEBDRIVER_URL`.
///
/// A subject with **no** `llm:` block stays unconfigured whatever is exported. An override
/// replaces a declared endpoint; it does not conjure a provider, a key, or a timeout the operator
/// never chose, and "no LLM configured" is a real answer triage already handles by falling back to
/// the prose floor.
fn apply_overrides(config: LlmConfig, base_url: Option<&str>, model: Option<&str>) -> LlmConfig {
    let overridden = [base_url.map(|_| BASE_URL_VAR), model.map(|_| MODEL_VAR)]
        .into_iter()
        .flatten()
        .collect();
    LlmConfig {
        base_url: base_url.map(str::to_string).unwrap_or(config.base_url),
        model: model.map(str::to_string).unwrap_or(config.model),
        overridden,
        ..config
    }
}

/// An override's value, or `None` when it is unset **or blank**. Exporting an empty string is how a
/// shell says "no value"; honoring it literally would point provreq at nothing and report a
/// connection error instead of the configuration mistake.
fn env_override(var: &str) -> Option<String> {
    std::env::var(var)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

impl LlmConfig {
    /// What to append to a run banner when an environment variable chose the endpoint or the model.
    /// Empty when the manifest is speaking for itself.
    pub fn override_note(&self) -> String {
        match self.overridden.as_slice() {
            [] => String::new(),
            vars => format!(" ({} in effect)", vars.join(" + ")),
        }
    }
}

/// The single LLM call, factored out for offline testing. A feature builds a [`PromptRequest`]
/// (via [`user_request`]) and reads the model's text from the [`PromptResponse`]; a stub returns a
/// canned response so prompt-building and reply-parsing are unit-tested with no live endpoint.
///
/// This is ReqForge's `PromptRequest`/`PromptResponse` seam (absorbed in slice 3a), reached here
/// through [`RuntimeBackend`] so provreq's features inherit its fallback chain, health tracking and
/// privacy gate — see `crates/reqforge-model/src/llm`.
pub trait LlmBackend {
    fn run_prompt(
        &self,
        req: &PromptRequest,
    ) -> impl std::future::Future<Output = Result<PromptResponse>> + Send;
}

/// Upper bound on output tokens for every provreq LLM call. ReqForge's `PromptRequest` requires an
/// explicit cap; provreq's outputs are short structured text (a bucket array, a PRL block, a
/// contract) so one generous cap covers every feature. Raise a per-feature cap only if one truncates.
// ponytail: one shared cap; split per-feature if a feature ever truncates.
const MAX_OUTPUT_TOKENS: u32 = 8192;

/// Wrap a prompt string as a single user-turn [`PromptRequest`]: no system prime, temperature 0
/// (every provreq feature wants a deterministic reply), and `timeout_ms` left unset so
/// [`RuntimeBackend`] fills it from the operator's configured timeout — the adapter's own 30 s
/// default is far too short for a local model finishing a large completion.
pub fn user_request(prompt: String) -> PromptRequest {
    PromptRequest {
        system: None,
        messages: vec![PromptMessage {
            role: PromptRole::User,
            content: prompt,
        }],
        max_tokens: MAX_OUTPUT_TOKENS,
        temperature: 0.0,
        timeout_ms: None,
    }
}

/// The production backend: ReqForge's [`LlmRuntime`] over a single configured provider.
pub struct RuntimeBackend {
    runtime: Arc<LlmRuntime>,
    /// The operator's configured per-request timeout, in milliseconds, injected into every prompt
    /// whose caller left `timeout_ms` unset.
    timeout_ms: u64,
}

impl RuntimeBackend {
    /// Build from config, resolving the API key from its named env var. Errors if the named
    /// variable is missing (fail fast, no silent keyless downgrade) — provreq keeps its committed
    /// manifest free of secrets, so the key is never read from the config the way ReqForge's own
    /// `apiKey` field allows.
    pub fn from_config(config: LlmConfig) -> Result<Self> {
        let api_key =
            match &config.api_key_env {
                Some(var) => Some(std::env::var(var).with_context(|| {
                    format!("environment variable {var} (LLM API key) is not set")
                })?),
                None => None,
            };
        let timeout_ms = config.timeout_seconds.saturating_mul(1000);
        let runtime = LlmRuntime::build(vec![provider_config_for(&config, api_key)])
            .context("building the LLM runtime")?;
        // provreq's CLI drafting has no privacy-ack UI; the operator configuring an endpoint has
        // already consented, so acknowledge the single provider up front (ReqForge's runtime would
        // otherwise skip a non-local endpoint until acked). Local endpoints need no ack anyway.
        runtime.privacy().acknowledge(0);
        Ok(Self {
            runtime,
            timeout_ms,
        })
    }
}

/// Map provreq's single-provider [`LlmConfig`] onto ReqForge's [`ProviderConfig`] (pure). The
/// resolved API key is passed in so the env read stays out of this function.
///
/// The endpoint is normalised: provreq manifests write the OpenAI-compatible `base_url` with its
/// `/v1` segment (`http://localhost:11434/v1`), but ReqForge's adapter appends `/v1/chat/completions`
/// to a host root, so the `/v1` is stripped here to avoid a doubled segment.
fn provider_config_for(config: &LlmConfig, api_key: Option<String>) -> ProviderConfig {
    ProviderConfig {
        provider: match config.provider {
            Provider::OpenaiCompatible => ProviderFamily::OpenaiCompatible,
            Provider::Anthropic => ProviderFamily::Anthropic,
        },
        model: config.model.clone(),
        endpoint: Some(normalize_endpoint(&config.base_url)),
        api_key,
        enabled: None,
    }
}

/// Strip a trailing `/v1` (and any trailing slash) from an endpoint so ReqForge's adapter, which
/// joins `/v1/chat/completions` onto a host root, does not produce `…/v1/v1/…` (pure). An endpoint
/// without the segment is returned unchanged.
fn normalize_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    trimmed.strip_suffix("/v1").unwrap_or(trimmed).to_string()
}

/// Reject an empty completion (REQ052): a reply with nothing in it — a reasoning model that spent
/// its budget thinking is one way to get one — is a failed request, not an empty answer, and
/// passing it on walks straight into a fabricated result downstream.
fn reject_empty(text: &str) -> Result<String> {
    if text.trim().is_empty() {
        bail!(
            "the LLM returned empty content — a reply with nothing in it is a failed request, not an answer"
        );
    }
    Ok(text.to_string())
}

impl LlmBackend for RuntimeBackend {
    async fn run_prompt(&self, req: &PromptRequest) -> Result<PromptResponse> {
        let req = if req.timeout_ms.is_none() {
            PromptRequest {
                timeout_ms: Some(self.timeout_ms),
                ..req.clone()
            }
        } else {
            req.clone()
        };
        let (_slot, resp) = self
            .runtime
            .run_prompt(&req)
            .await
            .map_err(|e| anyhow!("the LLM request failed: {e}"))?;
        let text = reject_empty(&resp.text)?;
        Ok(PromptResponse {
            text,
            usage: resp.usage,
        })
    }
}

/// What the subject declares, for the classifier's prompt (REQ072, #259): the caller builds it
/// from the code adapter's inventory. Whether a claim can be lowered depends on what there is to
/// bind to, and a classifier shown only prose is guessing at that — the measured failure mode.
/// Empty lists render nothing: a subject whose observables live elsewhere (a TLA+ model) must
/// not be described as declaring nothing.
#[derive(Debug, Default, Clone)]
pub struct SubjectContext {
    pub predicates: Vec<String>,
    pub sorts: Vec<String>,
}

/// The bulk pre-sort classifier. Generic over its backend so tests inject a stub. Carries the
/// subject's [`SubjectContext`] so every batch prompt states what there is to bind to (REQ072) —
/// the `Classifier` seam stays unchanged.
pub struct LlmClassifier<B: LlmBackend> {
    backend: B,
    context: SubjectContext,
}

impl<B: LlmBackend> LlmClassifier<B> {
    pub fn new(backend: B, context: SubjectContext) -> Self {
        Self { backend, context }
    }
}

impl<B: LlmBackend + Send + Sync> Classifier for LlmClassifier<B> {
    async fn classify(&self, items: &[Item]) -> Result<Vec<Option<Classification>>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let raw = self
            .backend
            .run_prompt(&user_request(build_prompt(items, &self.context)))
            .await?
            .text;
        parse_buckets(&raw, items)
    }
}

const PROMPT_HEADER: &str = "\
You are triaging software requirements for a provable-requirements tool.

The question is NOT how a requirement is worded. It is whether the claim it makes can be \
LOWERED to something a checking engine can evaluate: a predicate bound to real code, a \
property of a model, a pattern matched against a trace, or a step driven against a running \
deployment. Judge the claim, not the vocabulary. That a requirement happens to mention a \
command, an endpoint, a user interface or a release tells you nothing about which bucket it \
belongs in, and sorting on that wording is the most common way to get this wrong.

Classify each requirement into exactly one bucket:
- formalizable-now: the claim lowers to an invariant over program state — something always \
or never true, written as a predicate over values a deductive verifier can see. A claim \
about a pure function, about which states a data type may hold, or about a decision the code \
makes belongs here even when the prose sounds informal.
- falsifiable-only: the claim can be checked from finite observations of a running system — \
a monitor reading its trace, or a browser driven against a live deployment — but only \
refuted that way, never proved. Deadlines and other timing bounds, and anything stated about \
what a user interface shows.
- stays-prose: the claim cannot be lowered at all. This asserts the requirement will NEVER be \
formalized, so use it only where there is no claim carrying a definite truth value — not \
merely because lowering it would be hard, or because the wording is loose.

If you cannot place a requirement, OMIT it from your answer. An omitted requirement stays \
untriaged for a human to look at, and that is more useful than a guess.
";

const PROMPT_FOOTER: &str = "\n\nRespond with ONLY a JSON array, no prose and no code fences, \
one object per requirement you can place — omit the ones you cannot: \
[{\"id\": \"<id>\", \"bucket\": \"formalizable-now|falsifiable-only|stays-prose\"}]";

/// How many observable names a prompt lists per kind before cutting the list — with the cut
/// stated in the prompt, never applied silently (REQ072).
const OBSERVABLE_CAP: usize = 100;

/// Build the classification prompt (pure): the header, the gate's own category boundaries,
/// the subject's declared observables (capped, openly), the items, the answer format.
///
/// Implements: REQ072
fn build_prompt(items: &[Item], context: &SubjectContext) -> String {
    let mut prompt = String::from(PROMPT_HEADER);
    prompt.push_str(&boundary_section());
    prompt.push_str(&observables_section(context));
    prompt.push_str("\nRequirements:\n");
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

/// The category boundaries, rendered from the gate's own fragment encoding (REQ072) — the same
/// `rule` that will later admit or refuse the formalization, so the classifier is told the
/// boundary the pipeline actually enforces, not a paraphrase that can drift from it.
fn boundary_section() -> String {
    let mut out = String::from(
        "\nThe exact expressibility boundary, from the tool's own gate. Categories 1 (deductive \
         over code) and 2a (model checking) are the formalizable-now engines; 2b (runtime \
         monitor) and 3 (UI driver) are the falsifiable-only engines. A claim whose temporal \
         shape only a falsifiable-only engine admits belongs in falsifiable-only even when it \
         reads like a code property:\n",
    );
    for boundary in crate::prl::triage_boundaries() {
        out.push_str(&format!(
            "- category {}: expresses {}; cannot express {}\n",
            boundary.category,
            join_or_none(&boundary.admits),
            join_or_none(&boundary.refuses),
        ));
    }
    out
}

fn join_or_none(verbs: &[&str]) -> String {
    if verbs.is_empty() {
        "nothing beyond the others".to_string()
    } else {
        verbs.join(", ")
    }
}

/// The subject's declared observables (REQ072): what a formalizable claim has to bind to. A
/// list over [`OBSERVABLE_CAP`] is cut and the cut is stated — a silently trimmed list would
/// read as the whole subject. Empty context renders nothing at all: a subject whose
/// observables live elsewhere (a TLA+ model) must not be described as declaring nothing.
fn observables_section(context: &SubjectContext) -> String {
    if context.predicates.is_empty() && context.sorts.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\nThe subject's code declares these observables. A claim that binds to them has \
         something real to lower to — but binding decides only WHERE a claim could attach, and \
         the boundary above still decides HOW: a claim about these names whose shape categories \
         1 or 2a admit is formalizable-now; one whose shape only a monitor or driver admits is \
         falsifiable-only even though it names them. A claim mentioning none of them may still \
         lower through a model's observables:\n",
    );
    out.push_str(&capped_list("predicates", &context.predicates));
    out.push_str(&capped_list("sorts", &context.sorts));
    out
}

fn capped_list(label: &str, names: &[String]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let shown: Vec<&str> = names
        .iter()
        .take(OBSERVABLE_CAP)
        .map(String::as_str)
        .collect();
    let cut = names.len().saturating_sub(OBSERVABLE_CAP);
    let suffix = if cut > 0 {
        format!(" …(and {cut} more not listed)")
    } else {
        String::new()
    };
    format!("- {label}: {}{suffix}\n", shown.join(", "))
}

/// Map the model's reply back to one bucket per input item, in order.
///
/// Any item the model omits or mislabels gets **no** classification — `None`, which leaves it
/// un-triaged (REQ052).
///
/// This used to fall back to `stays-prose`, described as the honest floor because it "claims
/// nothing and leaves the work visible". Both halves were wrong, and #226 measured the cost.
/// `stays-prose` is not a floor: it is the lifecycle state meaning *this will not be formalized*,
/// which REQ011 keeps deliberately distinct from un-triaged precisely because they are different
/// facts. An item defaulted into it is recorded as judged unformalizable rather than as unjudged,
/// and — worse — it leaves the `untriaged` count, so the work still owed vanishes from the one
/// report built to show what is owed. Running this over provreq's own backlog took `untriaged`
/// from 67 to 0 while eleven of the classifications were defensibly wrong.
///
/// The floor is the absence of a classification. There is no bucket that claims nothing, so the
/// fallback cannot be a bucket.
///
/// A reply carrying **no** usable assignment stays a different event: the request failed, and a
/// blanket fallback across the whole backlog would be a fabricated classification, indistinguishable
/// from a model that read every requirement and judged them all unformalizable. That case is an
/// error, not a result. Pure.
fn parse_buckets(raw: &str, items: &[Item]) -> Result<Vec<Option<Classification>>> {
    let map = parse_assignments(raw);
    if map.is_empty() {
        bail!(
            "the model returned no usable classification — expected a JSON array of \
             {{id, bucket}}, got: {}",
            excerpt(raw)
        );
    }
    Ok(items.iter().map(|i| map.get(&i.id).copied()).collect())
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

    // Verifies: REQ072 / #259 — the prompt names the subject's declared observables, and a list
    // longer than the cap is cut *openly*: the prompt says how many were omitted, never silently.
    #[test]
    fn prompt_carries_observables_and_states_its_cap() {
        let context = SubjectContext {
            predicates: (0..150).map(|i| format!("pred{i:03}")).collect(),
            sorts: vec!["User".into()],
        };
        let prompt = build_prompt(&[item("REQ001", "some claim")], &context);
        assert!(prompt.contains("pred000"), "observables are listed");
        assert!(prompt.contains("User"), "sorts are listed");
        assert!(
            !prompt.contains("pred149"),
            "the cap actually cuts the list"
        );
        assert!(
            prompt.contains("50 more not listed"),
            "a cut list says how much was omitted:\n{prompt}"
        );
    }

    // Verifies: REQ072 / #259 — an empty context renders no observables section at all: a subject
    // whose observables live elsewhere (a TLA+ model) must not be described as declaring nothing.
    #[test]
    fn an_empty_context_renders_no_observables_section() {
        let prompt = build_prompt(&[item("REQ001", "t")], &SubjectContext::default());
        assert!(!prompt.contains("declares these observables"), "{prompt}");
    }

    // Verifies: REQ072 / #259 — the category boundaries in the prompt come from the gate's own
    // fragment encoding, not parallel prose: every verb the gate places appears, including the
    // one (`can_reach`) only the gate's rules would put anywhere.
    #[test]
    fn prompt_boundaries_come_from_the_gate() {
        let prompt = build_prompt(&[item("REQ001", "t")], &SubjectContext::default());
        for boundary in crate::prl::triage_boundaries() {
            for verb in boundary.admits.iter().chain(boundary.refuses.iter()) {
                assert!(
                    prompt.contains(verb),
                    "the gate places `{verb}` but the prompt never mentions it"
                );
            }
        }
        assert!(prompt.contains("can_reach"), "{prompt}");
    }

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
        async fn run_prompt(&self, _req: &PromptRequest) -> Result<PromptResponse> {
            Ok(PromptResponse {
                text: self.reply.clone(),
                usage: None,
            })
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
        let buckets = LlmClassifier::new(backend, SubjectContext::default())
            .classify(&items)
            .await
            .unwrap();
        assert_eq!(
            buckets,
            vec![
                Some(Classification::FormalizableNow),
                Some(Classification::StaysProse)
            ]
        );
    }

    // Verifies: REQ052 (#226) — an omitted or unreadable bucket yields NO classification, so the
    // item stays un-triaged. It used to fall back to `stays-prose`, which is not a floor but the
    // lifecycle claim *this will not be formalized*, and which drops the item out of `untriaged`.
    // The output length still matches the input, so triage can never be knocked out of step.
    #[tokio::test]
    async fn classify_leaves_missing_and_unknown_untriaged() {
        let items = [item("A", "x"), item("B", "y"), item("C", "z")];
        let backend = StubBackend {
            // A mislabeled, B present, C omitted entirely.
            reply: r#"[{"id":"A","bucket":"nonsense"},{"id":"B","bucket":"falsifiable-only"}]"#
                .into(),
        };
        let buckets = LlmClassifier::new(backend, SubjectContext::default())
            .classify(&items)
            .await
            .unwrap();
        assert_eq!(
            buckets,
            vec![None, Some(Classification::FalsifiableOnly), None],
            "a bucket the model did not give is not a bucket the tool may invent"
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
            let err = LlmClassifier::new(backend, SubjectContext::default())
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
        let buckets = LlmClassifier::new(backend, SubjectContext::default())
            .classify(&items)
            .await
            .unwrap();
        assert_eq!(
            buckets,
            vec![None, Some(Classification::FormalizableNow)],
            "one item was placed and one was not; the unplaced one stays un-triaged (#226)"
        );
    }

    // Verifies: REQ052 — a present-but-empty completion is a failed response, not an empty answer.
    // `RuntimeBackend::run_prompt` applies this at the seam boundary regardless of what the adapter
    // returns, so an empty `text` never reaches a feature as `Ok("")` and gets read as a real reply.
    #[test]
    fn reject_empty_treats_blank_completion_as_failure() {
        assert_eq!(reject_empty("ok").unwrap(), "ok");
        assert!(reject_empty("").is_err());
        assert!(
            reject_empty("   \n  ")
                .expect_err("whitespace is empty")
                .to_string()
                .contains("empty")
        );
    }

    // Verifies: #364 — the OpenAI-compatible `base_url` provreq manifests carry (with its `/v1`
    // segment) maps to a ReqForge endpoint at the host root, so the adapter's appended
    // `/v1/chat/completions` does not double the segment. A key resolved from the env is carried
    // through; the provider family is mapped.
    #[test]
    fn openai_base_url_v1_segment_is_stripped_for_the_adapter() {
        let config = LlmConfig {
            base_url: "http://localhost:11434/v1".into(),
            ..declared()
        };
        let pc = provider_config_for(&config, Some("sk-test".into()));
        assert_eq!(pc.provider, ProviderFamily::OpenaiCompatible);
        assert_eq!(pc.endpoint.as_deref(), Some("http://localhost:11434"));
        assert_eq!(pc.model, config.model);
        assert_eq!(pc.api_key.as_deref(), Some("sk-test"));
    }

    // Verifies: #364 — an endpoint without a `/v1` segment (an Anthropic host root, or a gateway
    // that omits it) is passed through unchanged; only a trailing `/v1` and slashes are trimmed.
    #[test]
    fn an_endpoint_without_v1_is_left_untouched() {
        assert_eq!(
            normalize_endpoint("https://api.anthropic.com"),
            "https://api.anthropic.com"
        );
        assert_eq!(
            normalize_endpoint("http://localhost:11434/v1/"),
            "http://localhost:11434"
        );
        assert_eq!(
            normalize_endpoint("http://gw.test/openai"),
            "http://gw.test/openai"
        );
    }

    // Verifies: REQ012 — a reply wrapped in a code fence still parses.
    #[tokio::test]
    async fn classify_tolerates_code_fenced_json() {
        let items = [item("A", "x")];
        let backend = StubBackend {
            reply: "Here you go:\n```json\n[{\"id\":\"A\",\"bucket\":\"formalizable-now\"}]\n```"
                .into(),
        };
        let buckets = LlmClassifier::new(backend, SubjectContext::default())
            .classify(&items)
            .await
            .unwrap();
        assert_eq!(buckets, vec![Some(Classification::FormalizableNow)]);
    }

    #[test]
    fn prompt_lists_every_item_id() {
        let prompt = build_prompt(
            &[item("REQ001", "a"), item("REQ042", "b")],
            &SubjectContext::default(),
        );
        assert!(prompt.contains("REQ001"));
        assert!(prompt.contains("REQ042"));
        assert!(prompt.contains("formalizable-now"));
    }

    // Verifies: REQ052 (#226) — the prompt must put the question the buckets answer, which is
    // whether a claim can be *lowered* to something an engine can check. Measured over this repo's
    // own 67 requirements: naming the buckets and leaving their meaning to be inferred produced a
    // sort on surface phrasing — mentions a command → falsifiable-only, mentions a UI or a release
    // → stays-prose — which put three requirements implemented by one pure, unit-tested module into
    // three different buckets.
    #[test]
    fn the_prompt_asks_about_lowering_not_about_wording() {
        let prompt = build_prompt(&[item("REQ001", "a")], &SubjectContext::default());
        assert!(
            prompt.contains("lower"),
            "the buckets answer a lowering question and the prompt must say so: {prompt}"
        );
        assert!(
            prompt.contains("invariant"),
            "category 1 is the temporal-free fragment (REQ024) — an invariant over a state \
             predicate is what `formalizable-now` actually means: {prompt}"
        );
        assert!(
            prompt.contains("wording") || prompt.contains("phrasing"),
            "the measured failure was a sort on phrasing, so the prompt warns against it \
             explicitly: {prompt}"
        );
        assert!(
            prompt.contains("omit"),
            "an item the model cannot place must be omitted rather than guessed — omission is now \
             the floor, and the model has to be told that: {prompt}"
        );
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

    /// The manifest's own portable default, as every subject ships it.
    fn declared() -> LlmConfig {
        LlmConfig {
            provider: Provider::OpenaiCompatible,
            base_url: "http://localhost:11434/v1".to_string(),
            model: "qwen3:32b".to_string(),
            api_key_env: None,
            timeout_seconds: DEFAULT_TIMEOUT_SECS,
            batch_size: DEFAULT_BATCH_SIZE,
            overridden: Vec::new(),
        }
    }

    // Verifies: #225 — the endpoint and model can be pointed at the box actually on this network
    // without editing a tracked file, and NOTHING ELSE moves. The manifest carries `environment:`,
    // the doorstop paths and `tla.constants` too, so "edit it and remember to revert" was a
    // dirty-working-tree trap on a file that genuinely belongs in the repo.
    #[test]
    fn an_environment_override_replaces_the_endpoint_and_model_only() {
        let overridden = apply_overrides(
            declared(),
            Some("http://192.168.222.108:11434/v1"),
            Some("llama3"),
        );
        assert_eq!(overridden.base_url, "http://192.168.222.108:11434/v1");
        assert_eq!(overridden.model, "llama3");
        assert_eq!(
            overridden.provider,
            declared().provider,
            "an override says WHERE, never how to speak to it"
        );
        assert_eq!(overridden.timeout_seconds, declared().timeout_seconds);
        assert_eq!(overridden.batch_size, declared().batch_size);
        assert_eq!(overridden.api_key_env, declared().api_key_env);
    }

    // Verifies: #225 — the manifest value stays the portable default. A subject that sets no
    // override behaves exactly as it did before this existed.
    #[test]
    fn no_override_leaves_the_manifest_speaking_for_itself() {
        let untouched = apply_overrides(declared(), None, None);
        assert_eq!(untouched, declared());
        assert_eq!(
            untouched.override_note(),
            "",
            "a run banner must stay quiet when nothing overrode anything"
        );
    }

    // Verifies: #225 — each field overrides on its own. Pointing at another host while keeping the
    // model is the ordinary case, and it must not require restating the other.
    #[test]
    fn each_field_overrides_independently() {
        let host_only = apply_overrides(declared(), Some("http://elsewhere:11434/v1"), None);
        assert_eq!(host_only.base_url, "http://elsewhere:11434/v1");
        assert_eq!(host_only.model, declared().model);

        let model_only = apply_overrides(declared(), None, Some("llama3"));
        assert_eq!(model_only.base_url, declared().base_url);
        assert_eq!(model_only.model, "llama3");
    }

    // Verifies: #225 — an override that took effect is VISIBLE. An export set months ago and
    // forgotten is otherwise invisible: the banner names an endpoint the committed file does not,
    // and nothing on screen explains the difference.
    #[test]
    fn an_override_in_effect_is_named_in_the_banner() {
        let both = apply_overrides(declared(), Some("http://elsewhere/v1"), Some("llama3"));
        let note = both.override_note();
        assert!(note.contains(BASE_URL_VAR), "{note}");
        assert!(note.contains(MODEL_VAR), "{note}");

        let one = apply_overrides(declared(), Some("http://elsewhere/v1"), None);
        assert!(one.override_note().contains(BASE_URL_VAR));
        assert!(
            !one.override_note().contains(MODEL_VAR),
            "an unset variable must not be reported as in effect: {}",
            one.override_note()
        );
    }

    // Verifies: #225 — the override never reaches the manifest. It is a fact about this run on this
    // machine, which is the entire reason it exists; serializing it back would re-commit the
    // address the override was invented to keep out of the repo.
    #[test]
    fn the_override_is_never_written_back_to_the_manifest() {
        let overridden = apply_overrides(declared(), Some("http://elsewhere/v1"), Some("llama3"));
        let yaml = serde_yaml::to_string(&overridden).unwrap();
        assert!(!yaml.contains("overridden"), "{yaml}");
        assert!(!yaml.contains(BASE_URL_VAR), "{yaml}");
        // And a manifest that never carried the field still loads.
        let round_tripped: LlmConfig = serde_yaml::from_str(&yaml).unwrap();
        assert!(round_tripped.overridden.is_empty());
    }

    // Verifies: REQ042 — an explicit `timeout_seconds` overrides the default, and the backend
    // builds with it (a keyless local endpoint constructs the runtime without error).
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
        assert!(RuntimeBackend::from_config(cfg).is_ok());
    }

    #[test]
    fn load_config_absent_llm_block_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(crate::adopt::MANIFEST_FILE), "schema: 1\n").unwrap();
        assert!(load_config(tmp.path()).unwrap().is_none());
    }
}
