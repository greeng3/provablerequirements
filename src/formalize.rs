//! Step 3 D11: the untrusted LLM **forward-translate** plus the **generate-then-repair
//! loop**. An LLM proposes a candidate PRL for a requirement's prose; the mechanical
//! gate ([`crate::prl::gate`]) then parses and type/name-checks it. On rejection, the
//! gate's structured errors are fed back to the LLM for a bounded re-translation
//! ([`MAX_ATTEMPTS`] total). Hard errors drive repair (the gate adjudicates *form*);
//! vacuity warnings ride through to the human (who adjudicates *meaning*, D12).
//!
//! The loop always returns the final candidate and its gate verdict, even when repair
//! is exhausted — a still-failing candidate is stored with its errors visible so the
//! operator can hand-edit and re-check. The candidate carries no trust until a human
//! confirms it via the D12 read-back (a later slice).
//!
//! The single network call sits behind [`crate::llm::LlmBackend`], so prompt-build
//! and reply-cleanup are unit-tested with a stub — no live endpoint needed.
//!
//! Implements: REQ015 (D11 forward-translate), REQ017 (generate-then-repair loop).

use crate::llm::LlmBackend;
use crate::prl::{gate, GateError, GateOutcome};
use crate::source::Item;
use anyhow::{bail, Result};

/// Total translate attempts before giving up: one initial proposal plus up to two
/// gate-driven repairs. Bounded so a model that cannot satisfy the gate does not loop.
const MAX_ATTEMPTS: u32 = 3;

/// The result of a full translate-then-gate cycle: the final candidate text, how many
/// attempts it took, and the gate verdict — accepted (with any warnings) or still
/// rejected after the repair budget was spent.
#[derive(Debug)]
pub struct RepairOutcome {
    pub candidate: String,
    pub attempts: u32,
    pub gate: Result<GateOutcome, Vec<GateError>>,
}

/// Forward-translates one requirement's prose into a candidate PRL. Generic over
/// its backend so tests inject a stub, mirroring [`crate::llm::LlmClassifier`].
pub struct Translator<B: LlmBackend> {
    backend: B,
}

impl<B: LlmBackend> Translator<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Propose a candidate PRL for `item` (a single, ungated LLM call). The reply is
    /// cleaned of any code fence; an empty proposal is an error rather than a
    /// silently-stored blank draft.
    pub async fn translate(&self, item: &Item) -> Result<String> {
        self.complete_prl(&build_prompt(item), &item.id).await
    }

    /// Translate `item`, then run the mechanical gate and repair on rejection, up to
    /// [`MAX_ATTEMPTS`]. Returns the final candidate with its gate verdict regardless
    /// of whether it ultimately passed — the caller stores and surfaces both.
    pub async fn translate_gated(&self, item: &Item) -> Result<RepairOutcome> {
        let mut candidate = self.translate(item).await?;
        let mut attempts = 1;
        loop {
            match gate(&candidate) {
                Ok(outcome) => {
                    return Ok(RepairOutcome {
                        candidate,
                        attempts,
                        gate: Ok(outcome),
                    })
                }
                Err(errors) => {
                    if attempts >= MAX_ATTEMPTS {
                        return Ok(RepairOutcome {
                            candidate,
                            attempts,
                            gate: Err(errors),
                        });
                    }
                    candidate = self
                        .complete_prl(&build_repair_prompt(item, &candidate, &errors), &item.id)
                        .await?;
                    attempts += 1;
                }
            }
        }
    }

    /// One backend call for a PRL block: complete, strip any code fence, and reject an
    /// empty proposal. Shared by the initial translate and each repair round.
    async fn complete_prl(&self, prompt: &str, id: &str) -> Result<String> {
        let raw = self.backend.complete(prompt).await?;
        let candidate = strip_fences(&raw).trim();
        if candidate.is_empty() {
            bail!("the LLM returned an empty PRL candidate for {id}");
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

/// Build a repair prompt (pure): restate the task, show the rejected attempt, and list
/// the gate's errors verbatim so the model can target each one. The errors carry source
/// lines, so the feedback is specific rather than "try again".
fn build_repair_prompt(item: &Item, previous: &str, errors: &[GateError]) -> String {
    let listed = errors
        .iter()
        .map(|e| format!("  - {e}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{PROMPT}{}:\n{}\n\n\
         Your previous attempt was rejected by the mechanical gate:\n\n{previous}\n\n\
         The gate reported these errors:\n{listed}\n\n\
         Fix every error above and respond with ONLY the corrected PRL requirement block.",
        item.id, item.text
    )
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

    /// A backend that returns a fixed sequence of replies (one per call) and records
    /// the prompts it saw — enough to drive and inspect the repair loop.
    struct SeqBackend {
        replies: Vec<String>,
        calls: std::sync::Mutex<usize>,
        prompts: std::sync::Mutex<Vec<String>>,
    }
    impl SeqBackend {
        fn new(replies: &[&str]) -> Self {
            Self {
                replies: replies.iter().map(|s| s.to_string()).collect(),
                calls: std::sync::Mutex::new(0),
                prompts: std::sync::Mutex::new(Vec::new()),
            }
        }
    }
    impl LlmBackend for SeqBackend {
        async fn complete(&self, prompt: &str) -> Result<String> {
            self.prompts.lock().unwrap().push(prompt.to_string());
            let mut i = self.calls.lock().unwrap();
            let idx = (*i).min(self.replies.len() - 1);
            *i += 1;
            Ok(self.replies[idx].clone())
        }
    }

    const GOOD: &str = "requirement r { vocabulary { state ok(x) } require { always ok(x) } }";
    const BAD: &str = "requirement r { require { always gone } }"; // `gone` undeclared

    // Verifies: REQ017 — a first attempt the gate rejects is repaired, and the second
    // attempt's prompt quotes the gate errors back to the model.
    #[tokio::test]
    async fn repair_loop_recovers_on_second_attempt() {
        let backend = SeqBackend::new(&[BAD, GOOD]);
        let t = Translator::new(backend);
        let out = t.translate_gated(&item("REQ001", "stay ok")).await.unwrap();
        assert_eq!(out.attempts, 2);
        assert!(out.gate.is_ok(), "second attempt should clear the gate");
        assert_eq!(out.candidate, GOOD);
        // The repair prompt carried the previous attempt and the gate's error text.
        let prompts = t.backend.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2);
        assert!(prompts[1].contains("rejected by the mechanical gate"));
        assert!(prompts[1].contains("gone"));
    }

    // Verifies: REQ017 — repair is bounded; after MAX_ATTEMPTS the last candidate is
    // returned with its errors, not looped forever.
    #[tokio::test]
    async fn repair_loop_gives_up_after_max_attempts() {
        let backend = SeqBackend::new(&[BAD]); // always ill-typed
        let out = Translator::new(backend)
            .translate_gated(&item("REQ001", "x"))
            .await
            .unwrap();
        assert_eq!(out.attempts, MAX_ATTEMPTS);
        assert!(
            out.gate.is_err(),
            "exhausted repair should still be failing"
        );
        assert_eq!(out.candidate, BAD);
    }

    // Verifies: REQ017 — vacuity warnings do NOT trigger repair; a valid-but-vacuous
    // candidate is accepted on the first attempt with the warning attached.
    #[tokio::test]
    async fn vacuity_warnings_do_not_drive_repair() {
        let vacuous = "requirement r { vocabulary { state p(x) } require { p(m) leads_to p(m) } }";
        let out = Translator::new(SeqBackend::new(&[vacuous]))
            .translate_gated(&item("REQ001", "x"))
            .await
            .unwrap();
        assert_eq!(out.attempts, 1);
        let outcome = out.gate.expect("vacuous-but-valid clears the gate");
        assert!(!outcome.warnings.is_empty());
    }

    #[test]
    fn repair_prompt_lists_errors_and_previous_attempt() {
        let errors = crate::prl::gate(BAD).unwrap_err();
        let p = build_repair_prompt(&item("REQ007", "stay ok"), BAD, &errors);
        assert!(p.contains("REQ007"));
        assert!(p.contains(BAD));
        assert!(p.contains("gone"));
        assert!(p.contains("corrected PRL"));
    }
}
