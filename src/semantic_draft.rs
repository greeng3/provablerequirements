//! The A6 **semantic contract-drafting** channel (REQ040): the LLM-driven follow-up to the
//! marker-only draft ([`crate::contract_draft`], REQ033). Marking a predicate function transparent
//! lets a deductive engine read inside it; it does not state what the function *guarantees*. A
//! deductive prover reasons through pre- and post-conditions, so this module asks the configured
//! LLM to propose the `#[requires]`/`#[ensures]` clauses for each resolved predicate function.
//!
//! Like the marker draft, the write surface stops at the subject working tree: the caller stages an
//! uncommitted edit and never runs git (A6, D12). The clause is a *proposal* from an untrusted
//! model — it carries no correctness of its own; the operator reviews the diff and the deductive
//! verifier re-checks it on the next run. A clause the prover cannot discharge is caught there,
//! never trusted because the model wrote it.
//!
//! The single network call sits behind [`crate::llm::LlmBackend`], mirroring
//! [`crate::formalize::Translator`], so prompt-build and reply-parse are unit-tested with a stub —
//! no live endpoint needed. A function whose source cannot be read, or for which the model proposes
//! no faithful clause, is skipped rather than annotated with a guess.
//!
//! Implements: REQ040 (draft semantic pre-/post-conditions onto resolved predicate functions).

use crate::contract_draft::Marker;
use crate::llm::LlmBackend;
use crate::rust_adapter::{fn_source_at, Resolution};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};

/// One predicate function's proposed deductive contract: the `#[requires(...)]`/`#[ensures(...)]`
/// attribute lines to stage directly above the signature at `file:line` (1-based). `clauses` is
/// never empty — a function the model proposes nothing for yields no draft at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractDraft {
    /// Subject-relative path of the file holding the predicate function.
    pub file: String,
    /// 1-based line of the signature the clauses go above (a [`Resolution::Resolved`]'s `at.line`).
    pub line: usize,
    /// The contract attribute lines to insert, in order, bare of indentation.
    pub clauses: Vec<String>,
}

/// Proposes deductive contracts for a requirement's resolved predicate functions. Generic over its
/// backend so tests inject a stub, mirroring [`crate::formalize::Translator`].
pub struct Drafter<B: LlmBackend> {
    backend: B,
}

impl<B: LlmBackend> Drafter<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Draft contracts for each resolved predicate function. One backend call per *distinct*
    /// function — `(file, line)` dedups predicates that resolve to the same function, exactly as
    /// [`crate::contract_draft::plan_markers`] does. A function whose source cannot be extracted, or
    /// for which the model proposes no clause, is skipped: honest silence, never a fabricated
    /// contract. `intent` is the requirement prose, `claim` the PRL candidate, `marker` selects the
    /// verifier dialect, and `sources` maps each resolved file to its full text.
    pub async fn draft(
        &self,
        intent: &str,
        claim: &str,
        marker: Marker,
        resolutions: &BTreeMap<String, Resolution>,
        sources: &BTreeMap<String, String>,
    ) -> Result<Vec<ContractDraft>> {
        let mut seen = BTreeSet::new();
        let mut drafts = Vec::new();
        for res in resolutions.values() {
            let Resolution::Resolved { at, .. } = res else {
                continue;
            };
            if !seen.insert((at.file.clone(), at.line)) {
                continue;
            }
            let Some(fn_src) = sources.get(&at.file).and_then(|t| fn_source_at(t, at.line)) else {
                continue;
            };
            let reply = self
                .backend
                .complete(&build_prompt(intent, claim, &fn_src, marker))
                .await?;
            let clauses = parse_clauses(&reply);
            if clauses.is_empty() {
                continue;
            }
            drafts.push(ContractDraft {
                file: at.file.clone(),
                line: at.line,
                clauses,
            });
        }
        Ok(drafts)
    }
}

/// The verifier dialect the drafted clauses target: engine name and its contracts crate, picked
/// from the subject's marker so the prompt names the right one (pure).
fn dialect(marker: Marker) -> (&'static str, &'static str) {
    match marker {
        Marker::Logic => ("Creusot", "creusot-contracts"),
        Marker::Pure => ("Prusti", "prusti-contracts"),
    }
}

/// Build the contract-drafting prompt for one function (pure). Names the verifier dialect, gives
/// the requirement's intent and formal claim for context, and shows the function's own source so
/// the model states the function's real contract rather than guessing from a signature alone. The
/// "respond with NOTHING" escape is what lets [`parse_clauses`] honestly skip a function.
fn build_prompt(intent: &str, claim: &str, fn_src: &str, marker: Marker) -> String {
    let (engine, krate) = dialect(marker);
    format!(
        "You are drafting deductive contracts for the Rust verifier {engine} ({krate}). Given a \
software requirement and one Rust function it is grounded to, propose that function's \
pre-conditions and post-conditions — the `#[requires(...)]` and `#[ensures(...)]` clauses a \
deductive prover needs to reason about the requirement. Use `result` for the return value and \
reference only the function's own parameters and `result`. If the function needs no contract to \
state its behaviour, or you cannot state one faithfully, respond with NOTHING.\n\n\
Respond with ONLY attribute lines — one `#[requires(...)]` or `#[ensures(...)]` per line — with no \
prose, no code fences, and no function signature.\n\n\
Requirement (intent):\n{intent}\n\n\
Formal claim (PRL):\n{claim}\n\n\
Function:\n{fn_src}\n"
    )
}

/// Keep only the lines that state a contract clause, tolerating prose or code fences the model may
/// wrap around them (a fence line does not start with the attribute, so it drops out on its own). A
/// line counts when, trimmed, it starts with `#[requires` or `#[ensures`; everything else — prose,
/// blanks, a stray signature, a bare marker — is dropped (pure).
fn parse_clauses(reply: &str) -> Vec<String> {
    reply
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("#[requires") || l.starts_with("#[ensures"))
        .map(String::from)
        .collect()
}

/// Stage this file's contract drafts into its source, returning the new text. Inserts run bottom-up
/// (highest line first) so an earlier insert never shifts a later target's line number. Each clause
/// copies the signature line's own indentation, so the block reads like hand-written code above the
/// function.
///
/// ponytail: the bottom-up-insert-with-indent mechanic is shared in spirit with
/// [`crate::contract_draft::apply_to_source`] (the marker path), but that one inserts a single line
/// per draft and this inserts an ordered block; extract a common helper if a third caller appears.
pub fn apply_to_source(src: &str, drafts: &[ContractDraft]) -> String {
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let mut sorted: Vec<&ContractDraft> = drafts.iter().collect();
    sorted.sort_by_key(|d| std::cmp::Reverse(d.line));
    for d in sorted {
        if d.line == 0 || d.line > lines.len() {
            continue;
        }
        let indent: String = lines[d.line - 1]
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();
        // Insert bottom-first (reversed) at the same index so the block's final order matches
        // `clauses`: each insert pushes the previously-inserted lines down by one.
        for clause in d.clauses.iter().rev() {
            lines.insert(d.line - 1, format!("{indent}{clause}"));
        }
    }
    let mut out = lines.join("\n");
    // Preserve a trailing newline if the original had one (`lines()` drops it).
    if src.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rust_adapter::{CodeMatch, ParamMode};

    fn resolved(file: &str, line: usize) -> Resolution {
        Resolution::Resolved {
            at: CodeMatch {
                file: file.to_string(),
                line,
                text: "fn p() -> bool {".to_string(),
            },
            params: vec![ParamMode::ByValue],
        }
    }

    /// A backend that records every prompt and replies from a fixed map keyed by a substring the
    /// prompt must contain — enough to give different functions different contracts.
    struct StubBackend {
        by_marker: BTreeMap<String, String>,
        prompts: std::sync::Mutex<Vec<String>>,
    }
    impl StubBackend {
        fn new(pairs: &[(&str, &str)]) -> Self {
            Self {
                by_marker: pairs
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                prompts: std::sync::Mutex::new(Vec::new()),
            }
        }
    }
    impl LlmBackend for StubBackend {
        async fn complete(&self, prompt: &str) -> Result<String> {
            self.prompts.lock().unwrap().push(prompt.to_string());
            let reply = self
                .by_marker
                .iter()
                .find(|(k, _)| prompt.contains(k.as_str()))
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            Ok(reply)
        }
    }

    // Verifies: REQ040 — only genuine contract lines survive; prose, code fences, blanks, and a
    // stray signature are dropped.
    #[test]
    fn parse_clauses_keeps_only_attribute_lines() {
        let reply = "Here are the contracts:\n```rust\n#[requires(u.is_valid())]\n\
                     #[ensures(result == u.active)]\nfn logged_in(u: &User) -> bool\n```\nDone.";
        assert_eq!(
            parse_clauses(reply),
            vec![
                "#[requires(u.is_valid())]",
                "#[ensures(result == u.active)]"
            ]
        );
        // A model that declines (NOTHING / only prose) yields no clause.
        assert!(parse_clauses("NOTHING").is_empty());
    }

    // Verifies: REQ040 — the prompt names the subject's verifier dialect and carries the intent,
    // the formal claim, and the function's own source.
    #[test]
    fn prompt_carries_dialect_intent_claim_and_source() {
        let p = build_prompt(
            "users stay logged in",
            "requirement r { require { always logged_in(u) } }",
            "fn logged_in(u: &User) -> bool { u.active }",
            Marker::Pure,
        );
        assert!(p.contains("Prusti"));
        assert!(p.contains("prusti-contracts"));
        assert!(p.contains("users stay logged in"));
        assert!(p.contains("always logged_in(u)"));
        assert!(p.contains("fn logged_in(u: &User) -> bool { u.active }"));
        // The Creusot dialect is named for a Logic subject.
        assert!(build_prompt("i", "c", "s", Marker::Logic).contains("Creusot"));
    }

    // Verifies: REQ040 — the drafter proposes clauses per resolved function, dedups predicates that
    // resolve to the same function, and skips a function the model declines.
    #[tokio::test]
    async fn drafts_per_function_dedups_and_skips_declined() {
        let mut res = BTreeMap::new();
        res.insert("logged_in".to_string(), resolved("src/lib.rs", 2));
        res.insert("alias".to_string(), resolved("src/lib.rs", 2)); // same fn
        res.insert("quiet".to_string(), resolved("src/lib.rs", 6)); // model declines
        res.insert("gone".to_string(), Resolution::NotFound); // nothing to draft
        let sources = BTreeMap::from([(
            "src/lib.rs".to_string(),
            "mod m {\n    fn logged_in(u: &User) -> bool { u.active }\n}\n\n\
             mod n {\n    fn quiet() -> bool { true }\n}\n"
                .to_string(),
        )]);
        // `logged_in` gets a contract; `quiet` (its source contains "quiet") returns NOTHING.
        let backend = StubBackend::new(&[
            ("fn logged_in", "#[ensures(result == u.active)]"),
            ("fn quiet", "NOTHING"),
        ]);
        let drafts = Drafter::new(backend)
            .draft("intent", "claim", Marker::Pure, &res, &sources)
            .await
            .unwrap();
        assert_eq!(
            drafts.len(),
            1,
            "one distinct function drafted, got {drafts:?}"
        );
        assert_eq!(drafts[0].file, "src/lib.rs");
        assert_eq!(drafts[0].line, 2);
        assert_eq!(drafts[0].clauses, vec!["#[ensures(result == u.active)]"]);
    }

    // Verifies: REQ040 — staged clauses are inserted as an ordered block above the signature with
    // the function's own indentation, and multiple functions in one file do not corrupt each
    // other's line targets. The result reads back as already-carrying the clauses.
    #[test]
    fn applies_ordered_block_with_indentation() {
        let src = "mod m {\n    fn a(u: &User) -> bool { u.ok }\n    fn b() -> bool { true }\n}\n";
        let drafts = vec![
            ContractDraft {
                file: "x".into(),
                line: 2,
                clauses: vec![
                    "#[requires(u.valid())]".into(),
                    "#[ensures(result == u.ok)]".into(),
                ],
            },
            ContractDraft {
                file: "x".into(),
                line: 3,
                clauses: vec!["#[ensures(result)]".into()],
            },
        ];
        let out = apply_to_source(src, &drafts);
        assert_eq!(
            out,
            "mod m {\n    \
             #[requires(u.valid())]\n    #[ensures(result == u.ok)]\n    fn a(u: &User) -> bool { u.ok }\n    \
             #[ensures(result)]\n    fn b() -> bool { true }\n}\n"
        );
    }
}
