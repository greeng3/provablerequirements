//! Phase 12a engine: prompt construction + response parsing for
//! LLM-assisted link suggestion.
//!
//! [`build_prompt`] flattens a project's artifact set + the link
//! catalog into a generic [`PromptRequest`] the 10a chain can hand
//! to any adapter family. [`parse_suggestions`] turns the model's
//! response text back into validated [`Suggestion`] records,
//! filtering self-links, empty link types, and triples that match
//! the declined-suggestions sidecar.
//!
//! Kept as a pure-compute module so unit tests cover prompt shape
//! and parser without spinning up an adapter; the integration
//! layer in `http::handlers` glues this to `LlmRuntime::run_prompt`.

use serde::Deserialize;
use uuid::Uuid;

use super::declined::is_declined;
use super::types::{DeclineRecord, Suggestion};
use crate::links::LinkType;
use crate::llm::{PromptMessage, PromptRequest, PromptRole};
use crate::load::LoadedProject;

/// Hard cap so a hallucinating model can't flood the inbox.
pub const MAX_SUGGESTIONS: usize = 50;

/// Per-artifact body truncation point. Keeps the prompt bounded
/// even when the project has long-form artifacts. Conservative
/// default; the chunking fallback handles oversize prompts at
/// the project level.
pub const MAX_BODY_CHARS: usize = 1_500;

/// Soft ceiling on the prompt size (system + user text) before
/// we fall back to per-Collection chunking. ~30k characters is
/// roughly 7.5k tokens at the 4-chars-per-token rule of thumb —
/// well below every shipped provider's context window, but
/// conservative enough to stay correct on smaller local models
/// (Llama-class) without per-provider budget plumbing.
pub const MAX_PROMPT_CHARS: usize = 30_000;

const DEFAULT_TEMPERATURE: f32 = 0.2;
const DEFAULT_MAX_TOKENS: u32 = 4_096;
/// 5-minute per-call timeout. The default 30 s in the OpenAI
/// adapter is fine for short prompts (rename suggestion, single
/// artifact); analyzing 100+ artifacts on a local 32 B model can
/// easily run several minutes per call. The chunking fallback
/// makes per-call work bounded; this gives each call enough
/// budget without timing out the operator's coffee break.
const DEFAULT_TIMEOUT_MS: u64 = 300_000;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ParseError {
    #[error("LLM response did not contain a JSON array")]
    NoArray,
    #[error("LLM response is not valid JSON: {0}")]
    InvalidJson(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ProposalError {
    #[error("LLM chain failed: {0}")]
    Chain(#[from] crate::llm::chain::ChainError),
    #[error("LLM response parse error: {0}")]
    Parse(#[from] ParseError),
}

/// Wire-shape for what the LLM emits. The server mints a UUIDv7
/// `id` after parsing so the model doesn't have to. Unknown
/// fields are ignored so tomorrow's looser shapes (e.g. an extra
/// `notes` field) deserialise without changes here.
#[derive(Debug, Deserialize)]
struct LlmSuggestion {
    from: Uuid,
    to: Uuid,
    #[serde(rename = "linkType")]
    link_type: String,
    #[serde(default)]
    confidence: f32,
    #[serde(default)]
    rationale: String,
}

/// Build the prompt the LLM consumes. Flattens every artifact
/// in the project (uuid + collection prefix + name + title +
/// description + body) alongside the link catalog so the model
/// has everything it needs in one shot. `max_tokens` /
/// `temperature` are pinned here.
pub fn build_prompt(project: &LoadedProject, link_catalog: &[LinkType]) -> PromptRequest {
    let user = build_user_prompt_for_artifacts(
        project
            .collections
            .iter()
            .flat_map(|c| c.artifacts.iter().map(move |a| (c, a))),
        link_catalog,
    );
    wrap_prompt(user)
}

/// Build a prompt for a single Collection's artifacts. Used by
/// the chunking fallback when the full-project prompt exceeds
/// `MAX_PROMPT_CHARS`. The link catalog is included verbatim so
/// each chunk gets the same vocabulary.
pub fn build_collection_prompt(
    collection: &crate::load::LoadedCollection,
    link_catalog: &[LinkType],
) -> PromptRequest {
    let user = build_user_prompt_for_artifacts(
        collection.artifacts.iter().map(|a| (collection, a)),
        link_catalog,
    );
    wrap_prompt(user)
}

fn wrap_prompt(user: String) -> PromptRequest {
    PromptRequest {
        system: Some(build_system_prompt()),
        messages: vec![PromptMessage {
            role: PromptRole::User,
            content: user,
        }],
        max_tokens: DEFAULT_MAX_TOKENS,
        temperature: DEFAULT_TEMPERATURE,
        timeout_ms: Some(DEFAULT_TIMEOUT_MS),
    }
}

/// Returns the total character count of the prompt's system +
/// user text. Used to decide when to fall back to per-Collection
/// chunking — see [`MAX_PROMPT_CHARS`].
pub fn prompt_size_chars(req: &PromptRequest) -> usize {
    let sys = req.system.as_deref().unwrap_or("").len();
    let user: usize = req.messages.iter().map(|m| m.content.len()).sum();
    sys + user
}

fn build_system_prompt() -> String {
    let mut s = String::new();
    s.push_str("You are a requirements-traceability assistant for ReqForge.\n");
    s.push_str(
        "Read the artifact list and propose links between artifacts using only the typed link \
         catalog provided.\n\n",
    );
    s.push_str("Output format: a single JSON array. Each entry has the shape:\n");
    s.push_str(
        r#"{ "from": "<uuid>", "to": "<uuid>", "linkType": "<name>", "confidence": <0.0-1.0>, "rationale": "<short explanation>" }"#,
    );
    s.push_str("\n\nRules:\n");
    s.push_str("- `from` and `to` are artifact UUIDs from the list below.\n");
    s.push_str("- `linkType` is one of the catalog entries.\n");
    s.push_str("- `confidence` reflects your certainty (0.0=guess, 1.0=obvious).\n");
    s.push_str("- `rationale` is one short sentence in the operator's voice.\n");
    s.push_str("- Do not propose self-links (from == to).\n");
    s.push_str("- Do not wrap the JSON in prose or code fences.\n");
    s.push_str("- If you are not confident in any links, return `[]`.\n");
    s
}

fn build_user_prompt_for_artifacts<'a, I>(artifacts: I, link_catalog: &[LinkType]) -> String
where
    I: IntoIterator<
        Item = (
            &'a crate::load::LoadedCollection,
            &'a crate::load::LoadedArtifact,
        ),
    >,
{
    let mut s = String::new();
    s.push_str("Link catalog:\n");
    for lt in link_catalog {
        let dir = if lt.directed {
            "directed"
        } else {
            "undirected"
        };
        s.push_str(&format!(
            "- {} ({}, inverse: {})\n",
            lt.name, dir, lt.inverse_name
        ));
    }
    s.push_str("\nArtifacts:\n");
    for (collection, artifact) in artifacts {
        s.push_str(&format!(
            "- uuid: {}\n  collection: {}\n  name: {}\n  title: {}\n",
            artifact.metadata.uuid,
            collection.config.prefix,
            artifact.name,
            artifact.metadata.title,
        ));
        if let Some(desc) = artifact.metadata.description.as_deref()
            && !desc.is_empty()
        {
            s.push_str(&format!("  description: {desc}\n"));
        }
        if let Some(body) = artifact.body.as_deref() {
            let truncated: String = body.chars().take(MAX_BODY_CHARS).collect();
            if !truncated.is_empty() {
                s.push_str("  body: |\n");
                for line in truncated.lines() {
                    s.push_str(&format!("    {line}\n"));
                }
            }
        }
    }
    s.push_str(
        "\nReturn a single JSON array of link proposals. Bare JSON only — no code fences, no \
         prose.",
    );
    s
}

/// Parse the model's response into validated `Suggestion`
/// records. Self-links, empty `linkType` values, and triples
/// that match the declined sidecar are dropped silently. Each
/// returned suggestion gets a fresh UUIDv7 `id`.
pub fn parse_suggestions(
    text: &str,
    declined: &[DeclineRecord],
) -> Result<Vec<Suggestion>, ParseError> {
    let array_text = extract_json_array(text).ok_or(ParseError::NoArray)?;
    let raw: Vec<LlmSuggestion> =
        serde_json::from_str(array_text).map_err(|e| ParseError::InvalidJson(e.to_string()))?;

    let mut out: Vec<Suggestion> = Vec::with_capacity(raw.len().min(MAX_SUGGESTIONS));
    for entry in raw {
        if entry.from == entry.to {
            continue;
        }
        if entry.link_type.trim().is_empty() {
            continue;
        }
        if is_declined(declined, entry.from, entry.to, &entry.link_type) {
            continue;
        }
        let confidence = if entry.confidence.is_finite() {
            entry.confidence.clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(Suggestion {
            id: Uuid::now_v7(),
            from: entry.from,
            to: entry.to,
            link_type: entry.link_type,
            confidence,
            rationale: entry.rationale,
        });
        if out.len() >= MAX_SUGGESTIONS {
            break;
        }
    }
    Ok(out)
}

/// Glue function: build prompt → run through the LLM chain →
/// parse response. Wraps any chain or parse failure in
/// [`ProposalError`]. The HTTP layer adds persistence
/// (writing the result to `pending.json`).
///
/// When the full-project prompt exceeds [`MAX_PROMPT_CHARS`],
/// falls back to per-Collection chunks: one LLM call per
/// Collection, results merged and deduplicated by the conceptual
/// `(from, to, link_type)` key. Cross-Collection links are out
/// of scope in chunked mode — finding them would require pair-
/// of-Collections chunking, which is deferred. The whole-project
/// path catches them when the project fits.
pub async fn propose_links(
    runtime: &crate::llm::runtime::LlmRuntime,
    project: &LoadedProject,
    link_catalog: &[LinkType],
    declined: &[DeclineRecord],
) -> Result<Vec<Suggestion>, ProposalError> {
    let full_prompt = build_prompt(project, link_catalog);
    if prompt_size_chars(&full_prompt) <= MAX_PROMPT_CHARS {
        let (_, response) = runtime.run_prompt(&full_prompt).await?;
        return parse_suggestions(&response.text, declined).map_err(Into::into);
    }
    propose_links_chunked(runtime, project, link_catalog, declined).await
}

async fn propose_links_chunked(
    runtime: &crate::llm::runtime::LlmRuntime,
    project: &LoadedProject,
    link_catalog: &[LinkType],
    declined: &[DeclineRecord],
) -> Result<Vec<Suggestion>, ProposalError> {
    let mut merged: Vec<Suggestion> = Vec::new();
    for collection in &project.collections {
        let prompt = build_collection_prompt(collection, link_catalog);
        let (_, response) = runtime.run_prompt(&prompt).await?;
        let chunk = parse_suggestions(&response.text, declined)?;
        for s in chunk {
            let already_seen = merged
                .iter()
                .any(|m| m.from == s.from && m.to == s.to && m.link_type == s.link_type);
            if !already_seen {
                merged.push(s);
            }
            if merged.len() >= MAX_SUGGESTIONS {
                return Ok(merged);
            }
        }
    }
    Ok(merged)
}

/// Try to extract a JSON array from the model's response,
/// stripping markdown code fences and prose around it. Returns
/// `None` only when no `[` / `]` pair can be found at all.
fn extract_json_array(text: &str) -> Option<&str> {
    let stripped = strip_code_fence(text);
    let trimmed = stripped.trim();
    if trimmed.starts_with('[') {
        return Some(trimmed);
    }
    let first = trimmed.find('[')?;
    let last = trimmed.rfind(']')?;
    if last <= first {
        return None;
    }
    Some(&trimmed[first..=last])
}

fn strip_code_fence(text: &str) -> &str {
    let t = text.trim();
    let prefixes = ["```json\n", "```\n", "```json ", "``` "];
    for p in prefixes {
        if let Some(rest) = t.strip_prefix(p) {
            if let Some(end) = rest.rfind("```") {
                return &rest[..end];
            }
            return rest;
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::builtin_catalog;

    fn uuid_byte(b: u8) -> Uuid {
        Uuid::from_bytes([
            0x01, 0x94, 0xf6, 0xd0, 0, 0, 0x70, 0, 0x80, 0, 0, 0, 0, 0, 0, b,
        ])
    }

    // ----- parse_suggestions -----

    #[test]
    fn parse_bare_json_array() {
        let text = format!(
            r#"[{{"from":"{}","to":"{}","linkType":"derives-from","confidence":0.9,"rationale":"strong overlap"}}]"#,
            uuid_byte(1),
            uuid_byte(2)
        );
        let out = parse_suggestions(&text, &[]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].from, uuid_byte(1));
        assert_eq!(out[0].to, uuid_byte(2));
        assert_eq!(out[0].link_type, "derives-from");
        assert!((out[0].confidence - 0.9).abs() < 1e-6);
        assert_eq!(out[0].rationale, "strong overlap");
    }

    #[test]
    fn parse_handles_markdown_code_fence() {
        let text = format!(
            "```json\n[{{\"from\":\"{}\",\"to\":\"{}\",\"linkType\":\"satisfies\",\"confidence\":0.5,\"rationale\":\"plausible\"}}]\n```",
            uuid_byte(1),
            uuid_byte(2)
        );
        let out = parse_suggestions(&text, &[]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].link_type, "satisfies");
    }

    #[test]
    fn parse_handles_prose_wrapper() {
        let text = format!(
            "Here are the proposals you asked for:\n\n[{{\"from\":\"{}\",\"to\":\"{}\",\"linkType\":\"verifies\",\"confidence\":0.7,\"rationale\":\"tests cover this\"}}]\n\nLet me know if you want more.",
            uuid_byte(3),
            uuid_byte(4)
        );
        let out = parse_suggestions(&text, &[]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].link_type, "verifies");
    }

    #[test]
    fn parse_empty_array_returns_empty() {
        let out = parse_suggestions("[]", &[]).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn parse_drops_self_links_and_empty_types() {
        let text = format!(
            r#"[
              {{"from":"{u1}","to":"{u1}","linkType":"derives-from","confidence":0.9,"rationale":"self"}},
              {{"from":"{u1}","to":"{u2}","linkType":"","confidence":0.9,"rationale":"empty type"}},
              {{"from":"{u1}","to":"{u2}","linkType":"derives-from","confidence":0.9,"rationale":"keeper"}}
            ]"#,
            u1 = uuid_byte(1),
            u2 = uuid_byte(2)
        );
        let out = parse_suggestions(&text, &[]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rationale, "keeper");
    }

    #[test]
    fn parse_filters_declined_triples() {
        use chrono::{TimeZone, Utc};
        let declined = vec![DeclineRecord {
            suggestion: Suggestion {
                id: uuid_byte(99),
                from: uuid_byte(1),
                to: uuid_byte(2),
                link_type: "derives-from".to_owned(),
                confidence: 0.5,
                rationale: "previously rejected".to_owned(),
            },
            declined_at: Utc.with_ymd_and_hms(2026, 5, 4, 12, 0, 0).unwrap(),
        }];

        let text = format!(
            r#"[
              {{"from":"{u1}","to":"{u2}","linkType":"derives-from","confidence":0.9,"rationale":"r1"}},
              {{"from":"{u1}","to":"{u2}","linkType":"satisfies","confidence":0.9,"rationale":"r2"}}
            ]"#,
            u1 = uuid_byte(1),
            u2 = uuid_byte(2)
        );
        let out = parse_suggestions(&text, &declined).unwrap();
        // Declined triple is filtered; the same endpoints with a
        // different link type still survive.
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].link_type, "satisfies");
    }

    #[test]
    fn parse_clamps_confidence_into_zero_one_range() {
        let text = format!(
            r#"[
              {{"from":"{u1}","to":"{u2}","linkType":"derives-from","confidence":1.5,"rationale":"over"}},
              {{"from":"{u3}","to":"{u4}","linkType":"derives-from","confidence":-0.2,"rationale":"under"}}
            ]"#,
            u1 = uuid_byte(1),
            u2 = uuid_byte(2),
            u3 = uuid_byte(3),
            u4 = uuid_byte(4),
        );
        let out = parse_suggestions(&text, &[]).unwrap();
        assert_eq!(out.len(), 2);
        assert!((out[0].confidence - 1.0).abs() < 1e-6);
        assert!((out[1].confidence - 0.0).abs() < 1e-6);
    }

    #[test]
    fn parse_returns_no_array_when_response_lacks_brackets() {
        let err = parse_suggestions("Sorry, I cannot help with that.", &[]).unwrap_err();
        assert_eq!(err, ParseError::NoArray);
    }

    #[test]
    fn parse_returns_invalid_json_for_garbage() {
        let err = parse_suggestions("[{not valid}]", &[]).unwrap_err();
        assert!(matches!(err, ParseError::InvalidJson(_)));
    }

    #[test]
    fn parse_caps_at_max_suggestions() {
        let mut entries = Vec::new();
        for i in 0..(MAX_SUGGESTIONS as u8 + 5) {
            entries.push(format!(
                r#"{{"from":"{}","to":"{}","linkType":"related-to","confidence":0.5,"rationale":"r"}}"#,
                uuid_byte(i),
                uuid_byte(i.wrapping_add(100)),
            ));
        }
        let text = format!("[{}]", entries.join(","));
        let out = parse_suggestions(&text, &[]).unwrap();
        assert_eq!(out.len(), MAX_SUGGESTIONS);
    }

    #[test]
    fn parse_mints_a_fresh_uuidv7_id_per_suggestion() {
        let text = format!(
            r#"[
              {{"from":"{u1}","to":"{u2}","linkType":"derives-from","confidence":0.5,"rationale":"a"}},
              {{"from":"{u1}","to":"{u3}","linkType":"derives-from","confidence":0.5,"rationale":"b"}}
            ]"#,
            u1 = uuid_byte(1),
            u2 = uuid_byte(2),
            u3 = uuid_byte(3),
        );
        let out = parse_suggestions(&text, &[]).unwrap();
        assert_eq!(out.len(), 2);
        assert_ne!(out[0].id, out[1].id);
        // UUIDv7 has version 7 in nibble 12.
        assert_eq!(out[0].id.get_version_num(), 7);
        assert_eq!(out[1].id.get_version_num(), 7);
    }

    // ----- propose_links (error-path glue) -----

    #[tokio::test]
    async fn propose_links_surfaces_no_providers_as_chain_error() {
        // Empty LlmRuntime => run_prompt returns ChainError::NoProviders;
        // propose_links should wrap that in ProposalError::Chain rather
        // than panicking or losing the typed error.
        let runtime = crate::llm::runtime::LlmRuntime::build(Vec::new()).unwrap();
        let project = empty_project();
        let err = propose_links(&runtime, &project, builtin_catalog(), &[])
            .await
            .unwrap_err();
        assert!(matches!(err, ProposalError::Chain(_)));
    }

    // ----- build_prompt -----

    fn empty_project() -> LoadedProject {
        LoadedProject {
            root: std::path::PathBuf::new(),
            config: crate::schema::ProjectConfig {
                schema_version: 1,
                slug: "p".to_owned(),
                name: "P".to_owned(),
                description: None,
                artifacts_path: None,
                scan_paths: None,
                overflow: Default::default(),
            },
            collections: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn prompt_size_chars_sums_system_and_user() {
        let project = empty_project();
        let req = build_prompt(&project, builtin_catalog());
        let expected = req.system.as_deref().unwrap().len() + req.messages[0].content.len();
        assert_eq!(prompt_size_chars(&req), expected);
        assert!(prompt_size_chars(&req) > 0);
    }

    // ----- build_collection_prompt -----

    #[test]
    fn build_collection_prompt_lists_only_that_collections_artifacts() {
        // Synthesise two collections with one artifact each. The
        // per-Collection prompt should mention only that
        // Collection's prefix + artifact name.
        let collection_a = synthetic_collection("REQ", "alpha", uuid_byte(10));
        let collection_b = synthetic_collection("ART", "beta", uuid_byte(20));
        let prompt_a = build_collection_prompt(&collection_a, builtin_catalog());
        let user_a = &prompt_a.messages[0].content;
        assert!(user_a.contains("alpha"));
        assert!(user_a.contains("REQ"));
        assert!(!user_a.contains("beta"));
        assert!(!user_a.contains("ART"));
        let prompt_b = build_collection_prompt(&collection_b, builtin_catalog());
        let user_b = &prompt_b.messages[0].content;
        assert!(user_b.contains("beta"));
        assert!(user_b.contains("ART"));
        assert!(!user_b.contains("alpha"));
        assert!(!user_b.contains("REQ"));
    }

    #[test]
    fn full_project_prompt_fits_when_project_is_small() {
        // A two-artifact project's prompt must stay under
        // MAX_PROMPT_CHARS — otherwise the chunking heuristic is
        // mistuned for the common case.
        let project = synthetic_project_two_artifacts();
        let req = build_prompt(&project, builtin_catalog());
        assert!(
            prompt_size_chars(&req) < MAX_PROMPT_CHARS,
            "expected small project to fit, got {} chars",
            prompt_size_chars(&req)
        );
    }

    fn synthetic_collection(
        prefix: &str,
        artifact_name: &str,
        artifact_uuid: Uuid,
    ) -> crate::load::LoadedCollection {
        crate::load::LoadedCollection {
            dir_name: prefix.to_lowercase(),
            dir_path: std::path::PathBuf::new(),
            config: crate::schema::CollectionConfig {
                schema_version: 1,
                prefix: prefix.to_owned(),
                name: format!("{prefix} name"),
                description: None,
                expects_code_trace: None,
                import_notes: None,
                overflow: Default::default(),
            },
            artifacts: vec![synthetic_artifact(artifact_name, artifact_uuid)],
        }
    }

    fn synthetic_artifact(name: &str, uuid: Uuid) -> crate::load::LoadedArtifact {
        crate::load::LoadedArtifact {
            name: name.to_owned(),
            source_path: std::path::PathBuf::new(),
            metadata: crate::schema::Artifact {
                schema_version: 1,
                uuid,
                title: format!("Title for {name}"),
                shape: crate::schema::ArtifactShape::Content,
                created_at: chrono::Utc::now(),
                modified_at: chrono::Utc::now(),
                links: Vec::new(),
                review_log: Vec::new(),
                description: None,
                expects_code_trace: None,
                active: None,
                derived: None,
                tags: None,
                outline_level: None,
                legacy: None,
                blob_path: None,
                url: None,
                checked_at: None,
                check_status: None,
                overflow: Default::default(),
            },
            body: Some(format!("body for {name}")),
            blob: None,
        }
    }

    fn synthetic_project_two_artifacts() -> LoadedProject {
        let mut project = empty_project();
        project
            .collections
            .push(synthetic_collection("REQ", "alpha", uuid_byte(1)));
        project
            .collections
            .push(synthetic_collection("ART", "beta", uuid_byte(2)));
        project
    }

    #[test]
    fn build_prompt_includes_artifact_uuids_and_link_catalog() {
        // We can't easily construct a LoadedProject here without
        // touching the filesystem (LoadedArtifact is a thick
        // type). For now, verify the prompt builder works
        // against an empty project — the integration test in
        // tests/ exercises a real project.
        let project = empty_project();
        let req = build_prompt(&project, builtin_catalog());
        assert!(req.system.is_some());
        let user = &req.messages[0].content;
        assert!(user.contains("Link catalog:"));
        assert!(user.contains("derives-from"));
        assert!(user.contains("satisfies"));
        assert!(user.contains("Artifacts:"));
        assert_eq!(req.temperature, DEFAULT_TEMPERATURE);
        assert_eq!(req.max_tokens, DEFAULT_MAX_TOKENS);
    }
}
