//! The **logic mirror** draft channel: what makes a Creusot `proven` reachable for a claim about
//! ordinary program functions.
//!
//! Creusot's specification language (pearlite) may only call `#[logic]` functions — measured in
//! `creusot/src/validate/purity.rs`, and reported to the operator as *called program function `f`
//! in logic context*. A category-1 predicate normally resolves to an ordinary function the subject
//! **calls**, which is exactly where `#[logic]` cannot go: the attribute declares a *logical*
//! function and removes the item from the program namespace, so every real call site stops
//! compiling (#158). That is the wall this module exists to get around.
//!
//! The way around it is Creusot's own idiom, not a trick: leave the program function alone and give
//! it a **mirror** — a `#[logic]` twin stating the same meaning in pearlite — then tie the two
//! together with a linking post-condition on the program function:
//!
//! ```ignore
//! #[logic]
//! pub fn is_ready_logic(s: &EngineStatus) -> bool {
//!     pearlite! { match *s { EngineStatus::Available { .. } => true, _ => false } }
//! }
//!
//! impl EngineStatus {
//!     #[ensures(result == is_ready_logic(self))]   // <- the link
//!     pub fn is_ready(&self) -> bool { matches!(self, EngineStatus::Available { .. }) }
//! }
//! ```
//!
//! The program function's body and all of its callers are untouched; it gains only a contract.
//!
//! **Why this stays proof-carrying.** The mirror body is the one piece carrying semantics, and it
//! is written by an untrusted model — so it is never believed. The linking `#[ensures]` makes the
//! prover discharge the mirror against the *real* body, and a wrong mirror therefore fails at that
//! link rather than propagating: measured, a mirror claiming `is_ready` means `Missing` yields
//! `Goal Coma.vc_is_ready'0: ✘`, naming the mirrored function, never a false `proven`. provreq
//! itself asserts nothing about the subject — the lowering emits only the mirror's *name*, and the
//! meaning behind that name is the operator's to review and the prover's to check.
//!
//! Like every A6 channel the write surface stops at the subject working tree: the caller stages an
//! uncommitted edit and runs no git.

use crate::llm::LlmBackend;
use crate::rust_adapter::{fn_source_at, CodeMatch, Resolution};
use crate::semantic_draft::ContractDraft;
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};

/// One predicate function's proposed logic mirror: the `#[logic]` item to append at module level,
/// and the linking `#[ensures]` clause to insert above the program function's signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorDraft {
    /// Subject-relative path of the file holding the program function.
    pub file: String,
    /// 1-based line of the program function's signature — where `link` is inserted.
    pub line: usize,
    /// The mirror's function name, e.g. `is_ready_logic`. This is the name a Creusot harness calls.
    pub name: String,
    /// The whole `#[logic] …` item, appended at the end of the file (module level).
    pub item: String,
    /// The `#[ensures(result == …)]` line tying the program function to its mirror.
    pub link: String,
}

/// The mirror name for a resolved observable. Derived by convention from the function's own name so
/// the lowering can address a mirror without a second round-trip to the model: a name the tool
/// *chooses* is a name it can also *predict*, whereas a name the model invented would have to be
/// carried through every later stage.
///
/// The observable may be a path (`EngineStatus::is_ready`); the mirror is a free function at module
/// level, so only the final segment matters.
pub fn mirror_name(observable: &str) -> String {
    let base = observable.rsplit("::").next().unwrap_or(observable);
    format!("{base}_logic")
}

/// Proposes logic mirrors for a requirement's resolved predicate functions. Generic over its backend
/// so tests inject a stub, mirroring [`crate::semantic_draft::Drafter`].
pub struct Mirrorer<B: LlmBackend> {
    backend: B,
}

impl<B: LlmBackend> Mirrorer<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Draft a mirror for each *distinct* resolved predicate function. `(file, line)` dedups
    /// predicates resolving to the same function, exactly as the contract and marker channels do. A
    /// function whose source cannot be extracted, or for which the model proposes nothing usable, is
    /// skipped — honest silence, never a fabricated mirror.
    pub async fn draft(
        &self,
        intent: &str,
        claim: &str,
        resolutions: &BTreeMap<String, Resolution>,
        sources: &BTreeMap<String, String>,
    ) -> Result<Vec<MirrorDraft>> {
        let mut seen = BTreeSet::new();
        let mut drafts = Vec::new();
        for (symbol, res) in resolutions {
            let Resolution::Resolved { at, .. } = res else {
                continue;
            };
            if !seen.insert((at.file.clone(), at.line)) {
                continue;
            }
            let Some(fn_src) = sources.get(&at.file).and_then(|t| fn_source_at(t, at.line)) else {
                continue;
            };
            let name = mirror_name(&observable_of(at, symbol));
            let reply = self
                .backend
                .complete(&build_prompt(intent, claim, &fn_src, &name))
                .await?;
            let Some((item, link)) = parse_mirror(&reply, &name) else {
                continue;
            };
            drafts.push(MirrorDraft {
                file: at.file.clone(),
                line: at.line,
                name,
                item,
                link,
            });
        }
        Ok(drafts)
    }
}

/// The function name to derive a mirror name from. The adapter records the matched signature text,
/// which names the function the resolution actually landed on; the requirement's own symbol is the
/// fallback when that text carries no `fn` name (it is the operator's binding, not a subject fact,
/// so it is used only when the subject offers nothing better).
fn observable_of(at: &CodeMatch, symbol: &str) -> String {
    at.text
        .split_once("fn ")
        .and_then(|(_, rest)| {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!name.is_empty()).then_some(name)
        })
        .unwrap_or_else(|| symbol.to_string())
}

/// The pearlite rules every drafted clause must obey, shared with
/// [`crate::semantic_draft`] so the contract channel states them too.
///
/// These are not decoration: each is a failure measured against a live model. It proposed
/// `matches!(…)` — pearlite rejects every macro but `pearlite!`/`proof_assert!`/`seq!` — and it
/// wrote specs calling the program function, which is the very thing a mirror exists to avoid.
/// Stating them costs nothing and saves a repair round that could not have succeeded anyway: no
/// amount of repair turns a program call in a logic context into a legal one.
pub const PEARLITE_RULES: &str = "\
Pearlite rules you must obey:\n\
- NO macros. `matches!`, `assert!`, `println!` and friends are rejected outright. Write a `match` \
expression instead of `matches!`.\n\
- Do NOT call the program function, or any other ordinary (non-`#[logic]`) function, from inside a \
specification. Only `#[logic]` functions and pure pearlite are available there.\n\
- Use `==>` for implication.\n";

/// Build the mirror-drafting prompt for one function (pure).
fn build_prompt(intent: &str, claim: &str, fn_src: &str, mirror_name: &str) -> String {
    format!(
        "You are writing a Creusot LOGIC MIRROR for one Rust function. A mirror is a `#[logic]` \
function that states, in Creusot's specification language (pearlite), exactly what the program \
function means. The program function is then linked to it by a post-condition, and the prover \
CHECKS the mirror against the real body — so state the function's actual meaning, never a guess.\n\n\
Respond with EXACTLY two things and nothing else — no prose, no code fences, no explanation:\n\
1. The mirror item, beginning `#[logic]`, named EXACTLY `{mirror_name}`, taking the same parameters \
as the function and returning the same type. A method's receiver becomes an ordinary first \
parameter of the SAME type INCLUDING its reference (`&self` becomes `s: &Thing`, not `Thing`) — and \
it must NOT be called `self`, which is legal only in an associated function. Its body must be \
`pearlite! {{ ... }}`.\n\
2. On its own line, the linking clause `#[ensures(result == {mirror_name}(...))]`, applying the \
mirror to the program function's own parameters (use `self` for a method receiver).\n\n\
{PEARLITE_RULES}\
- A logic function is a single EXPRESSION. No `return`, no statements, no `let mut`, no loops. \
Express a chain of guards as nested `if … {{ … }} else if … {{ … }} else {{ … }}`.\n\n\
If you cannot state the function's meaning faithfully under these rules, respond with NOTHING.\n\n\
Requirement (intent):\n{intent}\n\n\
Formal claim (PRL):\n{claim}\n\n\
Function:\n{fn_src}\n"
    )
}

/// Split a model reply into the mirror item and its linking clause, or `None` when the reply does
/// not carry a usable pair (pure).
///
/// Deliberately strict about the two things that would otherwise reach the compiler as a broken
/// staged edit: the item must actually declare the mirror name the tool will call — a mirror under
/// some other name is unreachable from the harness — and its braces must balance, since the item is
/// spliced into the subject's source verbatim. Everything else the model wraps around them (prose,
/// code fences) is dropped.
fn parse_mirror(reply: &str, mirror_name: &str) -> Option<(String, String)> {
    let cleaned: Vec<&str> = reply
        .lines()
        .filter(|l| !l.trim_start().starts_with("```"))
        .collect();
    let link = cleaned
        .iter()
        .map(|l| l.trim())
        .find(|l| l.starts_with("#[ensures"))?
        .to_string();
    let start = cleaned
        .iter()
        .position(|l| l.trim_start().starts_with("#[logic"))?;
    // Take the item through its balanced closing brace. A reply that never balances is truncated or
    // malformed; splicing it would break the subject's source, so it is refused instead.
    let mut depth = 0usize;
    let mut opened = false;
    let mut item_lines = Vec::new();
    for line in &cleaned[start..] {
        if line.trim().starts_with("#[ensures") {
            continue;
        }
        item_lines.push(*line);
        depth += line.matches('{').count();
        depth = depth.saturating_sub(line.matches('}').count());
        if line.contains('{') {
            opened = true;
        }
        if opened && depth == 0 {
            let item = item_lines.join("\n");
            return item
                .contains(&format!("fn {mirror_name}"))
                .then_some((item, link));
        }
    }
    None
}

/// The linking clauses as ordinary contract drafts, so they stage through the *same* insert pass as
/// the drafted `#[requires]`/`#[ensures]` (pure).
///
/// A link is an attribute above a signature — exactly what [`crate::semantic_draft::ContractDraft`]
/// already models. Staging the two kinds in one pass is not just less code: both are keyed to line
/// numbers in the **original** source, so applying them in separate passes would leave the second
/// pass working against text the first had already shifted.
pub fn link_clauses(drafts: &[MirrorDraft]) -> Vec<ContractDraft> {
    drafts
        .iter()
        .map(|d| ContractDraft {
            file: d.file.clone(),
            line: d.line,
            clauses: vec![d.link.clone()],
        })
        .collect()
}

/// Append this file's mirror items at module level, returning the new text (pure).
///
/// Appending, rather than splicing beside the function, is what keeps this independent of where the
/// function lives: a method sits inside an `impl` block and a `#[logic]` free function cannot go
/// there. At the end of the file the mirror is still in the same module, so a harness reaches it as
/// `crate::<module>::<name>` — the path the resolution already carries (REQ061).
///
/// Being line-independent is also why this composes with the link pass: it inserts nothing, so it
/// cannot disturb a line number the other pass depends on.
pub fn append_items(src: &str, drafts: &[MirrorDraft]) -> String {
    let mut out = src.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    // Items in the drafts' own order, so a mirror that calls an earlier mirror reads in order.
    for d in drafts {
        out.push_str(
            "\n// Drafted by provreq (--draft-semantic): a logic mirror, for review. The\n",
        );
        out.push_str("// linking #[ensures] above the program function is what makes the prover\n");
        out.push_str("// check this against the real body — it is a proposal, never a fact.\n");
        out.push_str(&d.item);
        if !d.item.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rust_adapter::{ParamMode, PredicateForm};

    fn resolved(file: &str, line: usize, text: &str) -> Resolution {
        Resolution::Resolved {
            at: CodeMatch {
                file: file.to_string(),
                line,
                text: text.to_string(),
                module: Some(vec![]),
            },
            params: vec![ParamMode::ByValue],
            form: PredicateForm::Function,
        }
    }

    struct StubBackend {
        reply: String,
        prompts: std::sync::Mutex<Vec<String>>,
    }
    impl LlmBackend for StubBackend {
        async fn complete(&self, prompt: &str) -> Result<String> {
            self.prompts.lock().unwrap().push(prompt.to_string());
            Ok(self.reply.clone())
        }
    }

    // Verifies: the mirror name is derived, not invented — the lowering must be able to predict the
    // name it will call without a second round-trip to the model.
    #[test]
    fn a_mirror_name_is_derived_from_the_function_it_mirrors() {
        assert_eq!(mirror_name("is_ready"), "is_ready_logic");
        assert_eq!(mirror_name("EngineStatus::is_ready"), "is_ready_logic");
        assert_eq!(mirror_name("decide_install"), "decide_install_logic");
    }

    // Verifies: the item and its linking clause are separated, and the item is taken through its
    // balanced closing brace rather than to the end of the reply.
    #[test]
    fn a_reply_splits_into_the_mirror_item_and_its_link() {
        let reply = "```rust\n\
                     #[logic]\n\
                     pub fn is_ready_logic(s: &EngineStatus) -> bool {\n\
                     \x20   pearlite! { match *s { EngineStatus::Available { .. } => true, _ => false } }\n\
                     }\n\
                     #[ensures(result == is_ready_logic(self))]\n\
                     ```";
        let (item, link) = parse_mirror(reply, "is_ready_logic").expect("a usable pair");
        assert!(item.starts_with("#[logic]"), "got {item}");
        assert!(
            item.ends_with('}'),
            "the item stops at its own closing brace: {item}"
        );
        assert!(
            !item.contains("#[ensures"),
            "the link is not part of the item"
        );
        assert_eq!(link, "#[ensures(result == is_ready_logic(self))]");
    }

    // Verifies (the honesty crux of parsing): a mirror declared under a DIFFERENT name than the one
    // the harness will call is unreachable, so it is refused rather than staged. Staging it would
    // produce a subject that compiles but whose harness names a function that does not exist.
    #[test]
    fn a_mirror_under_the_wrong_name_is_refused() {
        let reply = "#[logic]\n\
                     pub fn ready_pred(s: &EngineStatus) -> bool { pearlite! { true } }\n\
                     #[ensures(result == ready_pred(self))]";
        assert_eq!(parse_mirror(reply, "is_ready_logic"), None);
    }

    // Verifies: an item whose braces never balance is truncated or malformed. It is spliced into the
    // subject verbatim, so refusing it is the difference between an honest skip and a broken tree.
    #[test]
    fn an_unbalanced_item_is_refused_rather_than_spliced() {
        let reply = "#[logic]\n\
                     pub fn is_ready_logic(s: &EngineStatus) -> bool {\n\
                     \x20   pearlite! { match *s {\n\
                     #[ensures(result == is_ready_logic(self))]";
        assert_eq!(parse_mirror(reply, "is_ready_logic"), None);
    }

    // Verifies: a reply with no link clause yields nothing — an unlinked mirror is exactly the
    // unchecked assertion this channel exists to avoid.
    #[test]
    fn a_mirror_without_its_link_is_refused() {
        let reply = "#[logic]\n\
                     pub fn is_ready_logic(s: &EngineStatus) -> bool { pearlite! { true } }";
        assert_eq!(parse_mirror(reply, "is_ready_logic"), None);
    }

    fn engine_mirror() -> MirrorDraft {
        MirrorDraft {
            file: "src/engine.rs".to_string(),
            line: 2,
            name: "is_ready_logic".to_string(),
            item:
                "#[logic]\npub fn is_ready_logic(s: &EngineStatus) -> bool { pearlite! { true } }"
                    .to_string(),
            link: "#[ensures(result == is_ready_logic(self))]".to_string(),
        }
    }

    // Verifies: the link lands above the signature and the item at module level. A method's mirror
    // cannot be spliced into the `impl` block, which is why the item is appended instead. Both
    // passes compose: the link pass shifts lines, the item pass adds none.
    #[test]
    fn staging_puts_the_link_above_the_signature_and_the_item_at_module_level() {
        let src =
            "impl EngineStatus {\n    pub fn is_ready(&self) -> bool {\n        true\n    }\n}\n";
        let drafts = vec![engine_mirror()];
        let linked = crate::semantic_draft::apply_to_source(src, &link_clauses(&drafts));
        let out = append_items(&linked, &drafts);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[1], "    #[ensures(result == is_ready_logic(self))]",
            "the link takes the signature's own indentation"
        );
        assert_eq!(lines[2], "    pub fn is_ready(&self) -> bool {");
        assert!(
            out.trim_end().ends_with("pearlite! { true } }"),
            "the item lands at module level, not inside the impl: {out}"
        );
    }

    // Verifies: a link is an ordinary contract draft, so it stages through the SAME pass as the
    // drafted clauses. Two passes over line numbers taken from the original source would leave the
    // second working against text the first had already shifted.
    #[test]
    fn a_link_stages_as_an_ordinary_contract_clause() {
        let clauses = link_clauses(&[engine_mirror()]);
        assert_eq!(clauses.len(), 1);
        assert_eq!(clauses[0].file, "src/engine.rs");
        assert_eq!(clauses[0].line, 2);
        assert_eq!(
            clauses[0].clauses,
            vec!["#[ensures(result == is_ready_logic(self))]".to_string()]
        );
    }

    // Verifies: the prompt carries the dialect rules that a live model actually got wrong —
    // `matches!` and calling the program function from a spec. Both cost a repair round otherwise.
    #[test]
    fn the_prompt_states_the_pearlite_rules_the_model_measurably_breaks() {
        let p = build_prompt("intent", "claim", "fn f() -> bool { true }", "f_logic");
        assert!(p.contains("matches!"), "the macro ban must be explicit");
        assert!(p.contains("NO macros"));
        assert!(
            p.contains("single EXPRESSION"),
            "no early returns in a logic fn"
        );
        assert!(
            p.contains("Do NOT call the program function"),
            "the whole point of a mirror"
        );
        assert!(p.contains("f_logic"), "the required name must be stated");
    }

    // Verifies: the receiver rule is explicit about BOTH things a live model got wrong — it named
    // the parameter `self` (legal only in an associated function, so the staged edit would not
    // compile) and dropped the reference from the type.
    #[test]
    fn the_prompt_forbids_naming_a_free_function_parameter_self() {
        let p = build_prompt("i", "c", "fn f(&self) -> bool { true }", "f_logic");
        assert!(
            p.contains("must NOT be called `self`"),
            "measured failure: `fn is_ready_logic(self: EngineStatus)`"
        );
        assert!(
            p.contains("INCLUDING its reference"),
            "the receiver keeps its `&`"
        );
    }

    // Verifies: predicates resolving to the SAME function are mirrored once, matching the dedup the
    // contract and marker channels already use.
    #[tokio::test]
    async fn one_mirror_per_function_not_per_predicate() {
        let backend = StubBackend {
            reply:
                "#[logic]\npub fn is_ready_logic(s: &EngineStatus) -> bool { pearlite! { true } }\n\
                    #[ensures(result == is_ready_logic(self))]"
                    .to_string(),
            prompts: std::sync::Mutex::new(Vec::new()),
        };
        let mut resolutions = BTreeMap::new();
        resolutions.insert(
            "ready".to_string(),
            resolved("src/engine.rs", 2, "pub fn is_ready(&self) -> bool {"),
        );
        resolutions.insert(
            "available".to_string(),
            resolved("src/engine.rs", 2, "pub fn is_ready(&self) -> bool {"),
        );
        let mut sources = BTreeMap::new();
        sources.insert(
            "src/engine.rs".to_string(),
            "impl E {\npub fn is_ready(&self) -> bool { true }\n}\n".to_string(),
        );
        let drafts = Mirrorer::new(backend)
            .draft("intent", "claim", &resolutions, &sources)
            .await
            .expect("draft");
        assert_eq!(drafts.len(), 1, "same function, one mirror");
        assert_eq!(drafts[0].name, "is_ready_logic");
    }

    // Verifies: a model that declines (or answers unusably) yields NO draft for that function —
    // silence, never a fabricated mirror standing in for one.
    #[tokio::test]
    async fn a_declined_function_is_skipped_not_fabricated() {
        let backend = StubBackend {
            reply: "I cannot state this faithfully.".to_string(),
            prompts: std::sync::Mutex::new(Vec::new()),
        };
        let mut resolutions = BTreeMap::new();
        resolutions.insert(
            "ready".to_string(),
            resolved("src/engine.rs", 2, "pub fn is_ready(&self) -> bool {"),
        );
        let mut sources = BTreeMap::new();
        sources.insert(
            "src/engine.rs".to_string(),
            "impl E {\npub fn is_ready(&self) -> bool { true }\n}\n".to_string(),
        );
        let drafts = Mirrorer::new(backend)
            .draft("intent", "claim", &resolutions, &sources)
            .await
            .expect("draft");
        assert!(drafts.is_empty());
    }
}
