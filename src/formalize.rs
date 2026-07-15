//! Step 3 D11: the untrusted LLM **forward-translate**. An LLM proposes a candidate
//! PRL for a requirement's prose; the output is stored as a draft candidate the
//! operator reviews and edits. It is **ungated**: the mechanical gate (PRL parser,
//! type/fragment-check, vacuity) and the generate-then-repair loop are a later
//! slice, and the D12 deterministic read-back is another. The candidate carries
//! exactly the trust level of a hand-authored draft candidate — none until the
//! formalization gate runs.
//!
//! The single network call sits behind [`crate::llm::LlmBackend`], so prompt-build
//! and reply-cleanup are unit-tested with a stub — no live endpoint needed.
//!
//! Implements: REQ015 (D11 LLM forward-translate to a candidate PRL)

use crate::llm::LlmBackend;
use crate::source::Item;
use anyhow::{bail, Result};

/// Forward-translates one requirement's prose into a candidate PRL. Generic over
/// its backend so tests inject a stub, mirroring [`crate::llm::LlmClassifier`].
pub struct Translator<B: LlmBackend> {
    backend: B,
}

impl<B: LlmBackend> Translator<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Propose a candidate PRL for `item`. The reply is cleaned of any code fence
    /// the model wraps around it; an empty proposal is an error rather than a
    /// silently-stored blank draft.
    pub async fn translate(&self, item: &Item) -> Result<String> {
        let raw = self.backend.complete(&build_prompt(item)).await?;
        let candidate = strip_fences(&raw).trim();
        if candidate.is_empty() {
            bail!("the LLM returned an empty PRL candidate for {}", item.id);
        }
        Ok(candidate.to_string())
    }
}

const PROMPT: &str = "\
You translate one software requirement written in prose into a candidate PRL \
(Provable Requirement Language) requirement. PRL is pattern-based: you pick named \
specification patterns and fill typed slots — never write raw temporal-logic \
symbols.

A PRL requirement has this shape:

  requirement <name> {
    category:   1 | 2a | 2b | 3        // 1=code pre/post, 2a=model, 2b=runtime monitor, 3=UI
    vocabulary { ... }                 // the predicates/events/states it speaks about
    assume     { ... }                 // fairness / environment / delivery assumptions
    require    { ... }                 // the claim, using the patterns below
    strength:  <expected verdict>
    evidence:  <engine + params>
  }

Patterns (use these, not logic operators): never P; always P; eventually P; \
P leads_to Q (optionally: within T); S precedes P; P occurs at most k times. \
Scopes: globally; before R; after Q; between Q and R. \
Quantify over collections with: each m: Message . <claim about m>. \
Possibility (branching): can_reach P.

Example — prose \"every accepted message eventually succeeds or is dead-lettered, \
with at most 5 retries\" becomes:

  requirement no_message_lost {
    category: 2a + 2b
    vocabulary {
      event accepted(m: Message)
      state succeeded(m), dead_lettered(m: Message, reason: String)
    }
    assume { retries_bounded(N = 5) }
    require {
      each m: Message .
        accepted(m) leads_to (succeeded(m) or dead_lettered(m, r) with r != \"\")
    }
    strength: model_checked over Model, monitored(deadline = 30s)
    evidence: tla+ (bounded: |Message| <= 8), monpoly(stream = queue.events)
  }

Translate the requirement below. Respond with ONLY the PRL requirement block, no \
prose and no code fences.

Requirement ";

/// Build the forward-translate prompt for one item (pure). Keeps the prose intact
/// (unlike triage's flattening) — structure in the requirement text helps the
/// model pick patterns.
fn build_prompt(item: &Item) -> String {
    format!("{PROMPT}{}:\n{}", item.id, item.text)
}

/// Strip a single ```…``` code fence the model may wrap the PRL in, tolerating an
/// optional language tag (```prl). Returns the inner text, or the input unchanged
/// when there is no fence (pure).
fn strip_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    // Drop the opening fence's language tag / remainder of that line.
    let after_open = rest.split_once('\n').map(|(_, body)| body).unwrap_or("");
    match after_open.rfind("```") {
        Some(end) => after_open[..end].trim(),
        None => after_open.trim(),
    }
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

    // Verifies: REQ015 — the translator returns the model's PRL candidate.
    #[tokio::test]
    async fn translate_returns_candidate() {
        let backend = StubBackend {
            reply: "requirement respond { require { accepted leads_to done within 30s } }".into(),
        };
        let out = Translator::new(backend)
            .translate(&item("REQ001", "respond within 30 seconds"))
            .await
            .unwrap();
        assert!(out.starts_with("requirement respond"));
    }

    // Verifies: REQ015 — a fenced reply (```prl … ```) is unwrapped to the PRL.
    #[tokio::test]
    async fn translate_strips_code_fence() {
        let backend = StubBackend {
            reply: "```prl\nrequirement r { require { always ok } }\n```".into(),
        };
        let out = Translator::new(backend)
            .translate(&item("REQ001", "stay ok"))
            .await
            .unwrap();
        assert_eq!(out, "requirement r { require { always ok } }");
    }

    // Verifies: REQ015 — an empty proposal errors rather than storing a blank draft.
    #[tokio::test]
    async fn translate_rejects_empty_candidate() {
        let backend = StubBackend {
            reply: "   \n  ".into(),
        };
        let err = Translator::new(backend)
            .translate(&item("REQ001", "x"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("empty PRL candidate"));
    }

    #[test]
    fn prompt_carries_id_prose_and_patterns() {
        let p = build_prompt(&item("REQ042", "the system shall respond quickly"));
        assert!(p.contains("REQ042"));
        assert!(p.contains("the system shall respond quickly"));
        assert!(p.contains("leads_to"));
    }

    #[test]
    fn strip_fences_passes_through_unfenced() {
        assert_eq!(strip_fences("requirement r { }"), "requirement r { }");
    }
}
