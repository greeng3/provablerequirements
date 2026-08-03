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
    /// How to call the mirror from *another module of the same crate*, e.g.
    /// `crate::engine::is_ready_logic`.
    ///
    /// The harness builds its own paths ([`crate::lowering`] qualifies by module), but a mirror body
    /// also calls its **siblings**, and those calls live in the subject's source. Measured: told
    /// only the bare name, the model wrote `is_ready_logic(detected)` inside `src/provision.rs`
    /// while the mirror was declared in `src/engine.rs` — `error[E0425]: cannot find function
    /// 'is_ready_logic' in this scope`, and no prover ran. The module is a fact the adapter already
    /// recorded, so provreq states the path rather than hoping the model guesses it.
    pub path: String,
    /// The whole `#[logic] …` item, appended at the end of the file (module level).
    pub item: String,
    /// The `#[ensures(result == …)]` line tying the program function to its mirror.
    pub link: String,
}

/// What a drafting round produced: the mirrors it will stage, and the targets it **gave up on**.
///
/// Both halves matter, and only the first used to be returned. A predicate whose mirror is dropped
/// keeps calling its program function in the harness, so the claim fails at precisely the wall the
/// channel exists to remove — and the operator, reading a count of what was staged, has no way to
/// tell a complete draft from a half one. Measured on a fresh subject (#170): of two predicates,
/// one mirror was staged and the other silently abandoned, leaving `decide` — the function the
/// prover had already named — still called inside `proof_assert!` with nothing said about it.
///
/// Dropping itself is right and stays: a mirror provreq cannot parse or link is an *unchecked
/// meaning*, and staging one is the false-`proven` hazard this whole design is built to avoid. What
/// was wrong was doing it quietly. This is the tool's own version of the rule it applies to engines
/// (REQ065): what could not be done is reported in terms of the thing that stopped it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MirrorDrafts {
    /// The mirrors to stage — each one parsed, well-formed, and linkable.
    pub drafts: Vec<MirrorDraft>,
    /// The targets abandoned, and why. Empty on a clean round.
    pub dropped: Vec<DroppedMirror>,
}

/// A predicate function the channel tried to mirror and could not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedMirror {
    /// Subject-relative path of the program function's file.
    pub file: String,
    /// 1-based line of the program function's signature.
    pub line: usize,
    /// The program function's own name, as the operator would look for it.
    pub function: String,
    /// The mirror name that was asked for — worth showing, because the operator can write it.
    pub name: String,
    /// Which wall stopped it.
    pub wall: DropWall,
}

/// The three ways a mirror is abandoned. Each is a different thing for the operator to do, which is
/// why the verdict names which one rather than reporting a generic failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropWall {
    /// The model returned nothing that parses as the requested mirror function.
    NoMirrorInReply,
    /// A mirror was returned but is not a well-formed item — splicing it would break the source.
    MalformedItem,
    /// provreq could not build the linking `#[ensures]` from the function's signature.
    Unlinkable,
}

impl DropWall {
    /// What stopped it, and what the operator can do — the same shape as an engine's own limit.
    pub fn explain(self) -> &'static str {
        match self {
            DropWall::NoMirrorInReply => {
                "the model proposed nothing that parses as this mirror — nothing was staged rather \
                 than a guess at what the function means; re-run the draft, or write the mirror by \
                 hand"
            }
            DropWall::MalformedItem => {
                "the proposed mirror is not a well-formed item, and splicing it would break the \
                 subject's source — re-run the draft, or write the mirror by hand"
            }
            DropWall::Unlinkable => {
                "provreq could not build the linking `#[ensures]` from this signature (a parameter \
                 is a pattern rather than a plain name), and a mirror without its link is an \
                 unchecked meaning, never a weaker proof — so it is not staged at all"
            }
        }
    }
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
    ///
    /// Every target is resolved *before* the first model call, because a mirror body routinely needs
    /// to ask what a **sibling** predicate says — `decide_install` tests `detected.is_ready()` — and
    /// the ban on calling program functions leaves no legal way to express that unless the prompt
    /// also names the sibling's mirror. Measured: told only the ban, a live model wrote the program
    /// call anyway, and no repair round could have rescued it. Mirror names are pure convention
    /// ([`mirror_name`]), so the whole peer set is knowable up front.
    pub async fn draft(
        &self,
        intent: &str,
        claim: &str,
        resolutions: &BTreeMap<String, Resolution>,
        sources: &BTreeMap<String, String>,
    ) -> Result<MirrorDrafts> {
        let mut seen = BTreeSet::new();
        let mut targets = Vec::new();
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
            let observable = observable_of(at, symbol);
            let name = mirror_name(&observable);
            let path = mirror_path(at, &name);
            targets.push((at.clone(), observable, name, fn_src, path));
        }

        let mut out = MirrorDrafts::default();
        for (at, observable, name, fn_src, path) in &targets {
            let peers = peer_note(&targets, name);
            let reply = self
                .backend
                .complete(&build_prompt(intent, claim, fn_src, name, &peers))
                .await?;
            let dropped = |wall: DropWall| DroppedMirror {
                file: at.file.clone(),
                line: at.line,
                function: observable.clone(),
                name: name.clone(),
                wall,
            };
            let Some(item) = parse_mirror(&reply, name) else {
                out.dropped.push(dropped(DropWall::NoMirrorInReply));
                continue;
            };
            let item = make_open(&make_visible(&item, name, mirror_visibility(fn_src)));
            // A mirror that is not a well-formed function is not staged at all (see
            // [`is_well_formed_item`]).
            if !is_well_formed_item(&item) {
                out.dropped.push(dropped(DropWall::MalformedItem));
                continue;
            }
            // provreq builds the link, never the model — and a mirror it cannot link is dropped
            // rather than staged unchecked (see [`link_for`]).
            let Some(link) = link_for(fn_src, path) else {
                out.dropped.push(dropped(DropWall::Unlinkable));
                continue;
            };
            out.drafts.push(MirrorDraft {
                file: at.file.clone(),
                line: at.line,
                name: name.clone(),
                path: path.clone(),
                item,
                link,
            });
        }
        Ok(out)
    }
}

/// The linking post-condition for a program function, built by provreq rather than asked for.
///
/// The link is the load-bearing half of the design — it is what makes the prover discharge an
/// untrusted mirror against the real body — and it is also entirely mechanical: apply the mirror to
/// the function's own parameters, in order, with `self` for a receiver. Nothing about it needs a
/// model, and a model measurably gets it wrong: asked for the link, one wrote
/// `decide_install_logic(self, …)` for a **free** function whose first parameter is `detected`
/// (`error[E0424]: expected value, found module 'self'`), having over-applied the receiver rule.
///
/// `None` when the signature cannot be read or a parameter is a pattern rather than a plain name —
/// and the caller must then **drop the mirror entirely**. A mirror without its link is not a
/// weaker proof, it is an unchecked assertion: the harness would call a meaning the model invented
/// and nothing would ever compare it to the real body. That is exactly the false `proven` this
/// channel exists to make impossible.
fn link_for(fn_src: &str, mirror_path: &str) -> Option<String> {
    let item: syn::ItemFn = syn::parse_str(fn_src).ok()?;
    let mut args = Vec::new();
    for arg in &item.sig.inputs {
        match arg {
            syn::FnArg::Receiver(_) => args.push("self".to_string()),
            syn::FnArg::Typed(t) => match &*t.pat {
                syn::Pat::Ident(i) => args.push(i.ident.to_string()),
                _ => return None,
            },
        }
    }
    Some(format!(
        "#[ensures(result == {mirror_path}({}))]",
        args.join(", ")
    ))
}

/// Where a mirror declared alongside `at` is reachable from elsewhere in the crate.
///
/// The mirror is appended at module level in the same file as the program function, so it lives in
/// that function's module. A `None` module means the adapter could not place the item in the crate
/// at all; the bare name is then the only thing that can be said, and the lowering refuses such an
/// item separately, so nothing is silently mis-pathed.
fn mirror_path(at: &CodeMatch, name: &str) -> String {
    match &at.module {
        Some(module) => std::iter::once("crate")
            .chain(module.iter().map(String::as_str))
            .chain(std::iter::once(name))
            .collect::<Vec<_>>()
            .join("::"),
        None => name.to_string(),
    }
}

/// The prompt fragment naming the *other* mirrors being drafted in this run, so a mirror body can
/// reach a sibling predicate's meaning legally. Empty when this is the only mirror — an empty list
/// stated as a list reads as "there are none available", which is true but invites the model to
/// treat the ban as unsatisfiable.
fn peer_note(targets: &[(CodeMatch, String, String, String, String)], self_name: &str) -> String {
    note_from(
        targets
            .iter()
            .filter(|(_, _, name, _, _)| name != self_name)
            .map(|(_, observable, _, _, path)| {
                let short = observable.rsplit("::").next().unwrap_or(observable);
                (short.to_string(), path.clone())
            }),
    )
}

/// The same fragment for the *contract* channel, built from the mirrors already drafted.
///
/// The two channels run in one invocation and share one wall: a spec may call no program function.
/// Measured, the contract channel hit it too — it proposed `#[ensures(result <==> self.is_available())]`
/// — so telling only the mirror channel about the mirrors leaves the other half of the run with a
/// prohibition and no alternative. The program function's name is the mirror's minus the
/// [`mirror_name`] suffix, so no extra bookkeeping is needed to state the pairing.
pub fn mirror_note(mirrors: &[MirrorDraft]) -> String {
    note_from(mirrors.iter().map(|m| {
        let short = m.name.strip_suffix("_logic").unwrap_or(&m.name);
        (short.to_string(), m.path.clone())
    }))
}

/// Render the "call the mirror, not the function" list, or nothing when there is no mirror to
/// offer — an empty list stated as a list reads as "none available", which invites the model to
/// treat the ban as unsatisfiable and write the program call anyway.
fn note_from(pairs: impl Iterator<Item = (String, String)>) -> String {
    let lines: Vec<String> = pairs
        .map(|(short, mirror)| format!("- `{short}` → call `{mirror}(…)`\n"))
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "These functions have `#[logic]` mirrors, which ARE callable from a specification. If what \
you write depends on one of them, call the mirror — never the program function:\n{}\n",
        lines.concat()
    )
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
/// `matches!(…)` — pearlite rejects every macro but `pearlite!`/`proof_assert!`/`seq!`; it wrote
/// specs calling the program function, which is the very thing a mirror exists to avoid; and it
/// wrote `a <==> b`, which pearlite has no operator for and which fails as a bare
/// `error: expected term` naming no cause. Stating them costs nothing and saves a repair round that
/// could not have succeeded anyway: no amount of repair turns a program call in a logic context
/// into a legal one.
pub const PEARLITE_RULES: &str = "\
Pearlite rules you must obey:\n\
- NO macros. `matches!`, `assert!`, `println!` and friends are rejected outright. Write a `match` \
expression instead of `matches!`.\n\
- Do NOT call the program function, or any other ordinary (non-`#[logic]`) function, from inside a \
specification. Only `#[logic]` functions and pure pearlite are available there.\n\
- Use `==>` for implication. There is NO `<==>` operator — for a biconditional between two \
booleans write `==`.\n";

/// Build the mirror-drafting prompt for one function (pure).
fn build_prompt(intent: &str, claim: &str, fn_src: &str, mirror_name: &str, peers: &str) -> String {
    format!(
        "You are writing a Creusot LOGIC MIRROR for one Rust function. A mirror is a `#[logic]` \
function that states, in Creusot's specification language (pearlite), exactly what the program \
function means. The program function is then linked to it by a post-condition, and the prover \
CHECKS the mirror against the real body — so state the function's actual meaning, never a guess.\n\n\
Respond with EXACTLY ONE thing and nothing else — no prose, no code fences, no explanation:\n\
the mirror item, beginning `#[logic]` (provreq sets the attribute's modifiers and the item's \
visibility itself — write the plain form), named EXACTLY `{mirror_name}`, taking the same parameters \
as the function and returning the same type. A method's receiver becomes an ordinary first \
parameter of the SAME type INCLUDING its reference (`&self` becomes `s: &Thing`, not `Thing`) — and \
it must NOT be called `self`, which is legal only in an associated function. Its body must be \
`pearlite! {{ ... }}`.\n\
Write nothing else: provreq builds the linking post-condition itself, from the function's own \
signature.\n\n\
{PEARLITE_RULES}\
- A logic function is a single EXPRESSION. No `return`, no statements, no `let mut`, no loops. \
Express a chain of guards as nested `if … {{ … }} else if … {{ … }} else {{ … }}`.\n\n\
{peers}\
If you cannot state the function's meaning faithfully under these rules, respond with NOTHING.\n\n\
Requirement (intent):\n{intent}\n\n\
Formal claim (PRL):\n{claim}\n\n\
Function:\n{fn_src}\n"
    )
}

/// Pull the mirror item out of a model reply, or `None` when the reply carries no usable one (pure).
///
/// Deliberately strict about the two things that would otherwise reach the compiler as a broken
/// staged edit: the item must actually declare the mirror name the tool will call — a mirror under
/// some other name is unreachable from the harness — and its braces must balance, since the item is
/// spliced into the subject's source verbatim. Everything else the model wraps around it (prose,
/// code fences) is dropped.
///
/// It does **not** require the model to write the linking `#[ensures]`. It used to, and that was a
/// silent defect (#170): provreq builds the link itself from the signature ([`link_for`]) and threw
/// the model's line away unread, so a mirror that was well-formed, correctly named and perfectly
/// linkable was refused over a clause nothing consumed. The prompt justified asking for it as
/// marking where the item ends, which was never true either — the item is taken through its own
/// balanced brace, and `#[ensures]` lines are skipped in that scan. Measured on a fresh subject: of
/// two predicates, the one whose reply omitted the clause was dropped, leaving its program function
/// still called inside `proof_assert!`.
fn parse_mirror(reply: &str, mirror_name: &str) -> Option<String> {
    let cleaned: Vec<&str> = reply
        .lines()
        .filter(|l| !l.trim_start().starts_with("```"))
        .collect();
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
            return item.contains(&format!("fn {mirror_name}")).then_some(item);
        }
    }
    None
}

/// Give the mirror crate visibility, whatever visibility the model wrote (pure).
///
/// A mirror is appended at module level and then called from **outside that module** — by the
/// generated harness as `crate::<module>::<name>`, and by a sibling mirror in another file. A model
/// writing plain `fn` (the common case, since the function it mirrors is often a method) makes both
/// callers a privacy error, and that is provreq's mistake to prevent rather than the model's to
/// remember: visibility is a consequence of where provreq chose to put the item.
///
/// `vis` is [`mirror_visibility`]'s answer, not a fixed `pub(crate)`: Creusot enforces its own rule
/// on top of Rust's — *cannot make `engine::is_ready_logic` transparent in
/// `engine::EngineStatus::is_ready` as it would call a less-visible item* — so a `pub` function's
/// contract may not mention a `pub(crate)` mirror. The mirror therefore takes the visibility of the
/// function it mirrors, floored at `pub(crate)` so the harness can always reach it.
///
/// The declaration is found *anywhere* in the line rather than at its start: a model is as likely to
/// emit `#[logic] fn f_logic(…) { … }` on one line as to put the attribute above it, and matching
/// only a line-initial `fn` silently left that form private — measured as
/// `error[E0603]: function 'decide_install_logic' is private`, one whole run spent on a one-line
/// reply. `fn <name>` cannot collide with a call to the same mirror, which never carries the `fn`.
fn make_visible(item: &str, mirror_name: &str, vis: &str) -> String {
    let decl = format!("fn {mirror_name}");
    let mut done = false;
    item.lines()
        .map(|line| {
            if done {
                return line.to_string();
            }
            let Some(at) = line.find(&decl) else {
                return line.to_string();
            };
            done = true;
            // Any visibility the model wrote is replaced, not respected: the level is forced by
            // where provreq put the item and by Creusot's less-visible-item rule, so a model
            // guessing `pub(crate)` where `pub` is required must still be corrected.
            let before = strip_visibility(&line[..at]);
            format!("{before}{vis} {}", &line[at..])
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether the drafted item actually parses as a Rust function (pure).
///
/// [`parse_mirror`] checks that the item declares the right name and that its braces balance, and
/// that is not enough. Measured against a live model: it emitted
/// `#[logic] pub fn decide_logic(s: &Status, allowed: bool) -> Outcome pearlite! { … }` — the body's
/// outer braces simply missing. The `pearlite!` block balanced, so the brace check passed, and the
/// malformed item was spliced into the subject's source, where it cost the whole run.
///
/// `syn` settles it in one call, and the same call already underwrites [`link_for`]. Pearlite's own
/// syntax inside `pearlite! { … }` is not a problem: a macro invocation only has to be a balanced
/// token tree, so `==>` and the rest pass through untouched.
///
/// An unparseable item means the mirror is dropped — honest silence, the same rule as a mirror that
/// cannot be linked. Staging source that cannot compile helps nobody and hides the real answer
/// behind a build error.
fn is_well_formed_item(item: &str) -> bool {
    syn::parse_str::<syn::ItemFn>(item).is_ok()
}

/// Make the mirror's `#[logic]` **transparent beyond its own module** (pure).
///
/// Visibility and transparency are different things in Creusot, and a mirror needs both. A `pub`
/// `#[logic]` item is *callable* everywhere, but its **body** is unfoldable only where it is
/// transparent, and `mk_opacity` in `creusot/src/ctx.rs` defaults that to
/// `Visibility::Restricted(parent_module)`. The generated harness is a different module, so a bare
/// `#[logic]` mirror reaches the prover as an **uninterpreted function**: everything compiles, the
/// prover runs, and the goal simply cannot be discharged.
///
/// Measured exactly that way — the fixture in [`crate::creusot`] compiled and ran and returned
/// `Inconclusive` until its mirrors became `#[logic(open)]`, whereupon it proved. That failure mode
/// is worth naming because it looks like a false claim rather than a missing attribute.
///
/// Like the link and the visibility, this is provreq's to get right rather than the model's: the
/// mirror is opaque only because provreq chose to put it in a module the harness does not share.
fn make_open(item: &str) -> String {
    if item.contains("#[logic(") {
        // An explicit modifier list the model wrote: add `open` unless it is already asking for it.
        // `opaque` is left alone — Creusot rejects `open` and `opaque` together, and a mirror the
        // model deliberately sealed is not something to quietly reopen.
        return if item.contains("open") || item.contains("opaque") {
            item.to_string()
        } else {
            item.replacen("#[logic(", "#[logic(open, ", 1)
        };
    }
    item.replacen("#[logic]", "#[logic(open)]", 1)
}

/// Drop a trailing `pub` / `pub(…)` from the text preceding a `fn`, keeping the spacing before it.
fn strip_visibility(before: &str) -> String {
    let trimmed = before.trim_end();
    let kept = trimmed
        .strip_suffix("pub")
        .or_else(|| {
            trimmed
                .ends_with(')')
                .then(|| trimmed.rfind("pub("))
                .flatten()
                .map(|i| &trimmed[..i])
        })
        .unwrap_or(trimmed);
    // Preserve the original leading whitespace, which carries the item's indentation.
    let indent = &before[..before.len() - before.trim_start().len()];
    if kept.trim().is_empty() {
        indent.to_string()
    } else {
        format!("{} ", kept.trim_end())
    }
}

/// The visibility a mirror of `fn_src` must carry: the mirrored function's own, floored at
/// `pub(crate)`.
///
/// Two rules meet here. Rust's: the harness is a *different module*, so a private mirror is
/// unreachable. Creusot's: a function's contract may not mention a **less-visible** item — measured
/// as *cannot make `engine::is_ready_logic` transparent in `engine::EngineStatus::is_ready` as it
/// would call a less-visible item*, from a `pub fn` linked to a `pub(crate)` mirror. Taking the
/// mirrored function's own visibility satisfies both, and never widens the subject's public surface
/// beyond what that function already exposed.
fn mirror_visibility(fn_src: &str) -> &'static str {
    match syn::parse_str::<syn::ItemFn>(fn_src) {
        Ok(item) if matches!(item.vis, syn::Visibility::Public(_)) => "pub",
        _ => "pub(crate)",
    }
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

    // Verifies: the item is taken through its own balanced closing brace rather than to the end of
    // the reply, and a stray `#[ensures]` the model volunteers is left out of it — provreq builds
    // the real link from the signature, so the model's version must not be spliced in beside it.
    #[test]
    fn the_item_stops_at_its_own_brace_and_excludes_any_volunteered_link() {
        let reply = "```rust\n\
                     #[logic]\n\
                     pub fn is_ready_logic(s: &EngineStatus) -> bool {\n\
                     \x20   pearlite! { match *s { EngineStatus::Available { .. } => true, _ => false } }\n\
                     }\n\
                     #[ensures(result == is_ready_logic(self))]\n\
                     ```";
        let item = parse_mirror(reply, "is_ready_logic").expect("a usable item");
        assert!(item.starts_with("#[logic]"), "got {item}");
        assert!(
            item.ends_with('}'),
            "the item stops at its own closing brace: {item}"
        );
        assert!(
            !item.contains("#[ensures"),
            "the link is not part of the item"
        );
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

    // Verifies (#170): a reply carrying only the item is USABLE. This test asserted the opposite
    // and encoded the defect: the reasoning was that an unlinked mirror is the unchecked assertion
    // this channel avoids, but the mirror is not unlinked — provreq builds the link from the
    // signature (`link_for`) and never read the model's version. So a well-formed, correctly named,
    // perfectly linkable mirror was refused over a clause nothing consumed, and refused in silence.
    // Measured on a fresh subject: that dropped the one predicate the claim needed.
    #[test]
    fn a_reply_carrying_only_the_item_is_usable() {
        let reply = "#[logic]\n\
                     pub fn is_ready_logic(s: &EngineStatus) -> bool { pearlite! { true } }";
        let item = parse_mirror(reply, "is_ready_logic").expect("the item is all that is needed");
        assert!(item.contains("fn is_ready_logic"));
        assert!(
            !item.contains("#[ensures"),
            "the link is provreq's to build: {item}"
        );
    }

    // Verifies: provreq builds the link from the signature — a receiver becomes `self`, a free
    // function's parameters are named in order. Measured failure: asked for the link, a live model
    // wrote `decide_install_logic(self, …)` for a FREE function whose first parameter is `detected`
    // (`error[E0424]: expected value, found module 'self'`), over-applying the receiver rule.
    #[test]
    fn provreq_builds_the_link_from_the_signature_not_the_model() {
        assert_eq!(
            link_for(
                "pub fn is_ready(&self) -> bool { true }",
                "crate::engine::is_ready_logic"
            )
            .unwrap(),
            "#[ensures(result == crate::engine::is_ready_logic(self))]"
        );
        assert_eq!(
            link_for(
                "pub fn decide_install(detected: &EngineStatus, platform_supported: bool) -> D { todo!() }",
                "crate::provision::decide_install_logic"
            )
            .unwrap(),
            "#[ensures(result == crate::provision::decide_install_logic(detected, platform_supported))]"
        );
    }

    // Verifies: a mirror provreq cannot link is DROPPED, not staged. An unlinked mirror is not a
    // weaker proof — it is a model-invented meaning the prover would never compare against the real
    // body, which is the false `proven` this whole channel exists to prevent.
    #[test]
    fn a_mirror_that_cannot_be_linked_is_dropped_not_staged() {
        assert!(
            link_for("pub fn f((a, b): (bool, bool)) -> bool { a }", "m").is_none(),
            "a destructured parameter has no name to pass"
        );
        assert!(
            link_for("this is not a function", "m").is_none(),
            "an unreadable signature links to nothing"
        );
    }

    // Verifies: an item that balances its braces but is not a function is refused. Measured against
    // a live model: `pub fn decide_logic(…) -> Outcome pearlite! { … }` — the body's outer braces
    // missing. `parse_mirror`'s brace count was satisfied by the `pearlite!` block, so the malformed
    // item reached the subject's source and cost the whole run.
    #[test]
    fn a_mirror_that_is_not_a_well_formed_function_is_refused() {
        assert!(
            !is_well_formed_item("#[logic(open)] pub fn f_logic(x: bool) -> bool pearlite! { x }"),
            "the body's braces are missing — balanced elsewhere is not the same as well formed"
        );
        assert!(is_well_formed_item(
            "#[logic(open)]\npub fn f_logic(x: bool) -> bool { pearlite! { x } }"
        ));
        // Pearlite syntax inside the macro is a balanced token tree, so it must not be rejected.
        assert!(
            is_well_formed_item(
                "#[logic(open)]\npub fn f_logic(a: bool, b: bool) -> bool { pearlite! { a ==> b } }"
            ),
            "`==>` is not Rust, but a macro body only has to balance"
        );
    }

    // Verifies: a staged mirror is `#[logic(open)]`, not bare `#[logic]`. Creusot defaults a logic
    // function's TRANSPARENCY to its own module, so a bare mirror is an uninterpreted function to
    // the harness — it compiles, the prover runs, and the goal cannot be discharged. Measured: the
    // real-Creusot fixture returned `Inconclusive` until the mirrors were opened, then proved.
    #[test]
    fn a_staged_mirror_is_transparent_beyond_its_own_module() {
        assert_eq!(
            make_open("#[logic]\npub fn f_logic(x: bool) -> bool { x }"),
            "#[logic(open)]\npub fn f_logic(x: bool) -> bool { x }"
        );
        // An explicit modifier list gains `open` alongside what the model asked for.
        assert_eq!(
            make_open("#[logic(inline)]\npub fn f_logic() -> bool { true }"),
            "#[logic(open, inline)]\npub fn f_logic() -> bool { true }"
        );
        // Idempotent, so a re-staged mirror does not accumulate modifiers.
        let opened = "#[logic(open)]\npub fn f_logic() -> bool { true }";
        assert_eq!(make_open(opened), opened);
        // `opaque` is left alone — Creusot rejects it together with `open`, and a mirror the model
        // deliberately sealed is not something to quietly reopen.
        let sealed = "#[logic(opaque)]\npub fn f_logic() -> bool { true }";
        assert_eq!(make_open(sealed), sealed);
    }

    // Verifies: a mirror is offered to its siblings by its full crate path, not its bare name. The
    // measured failure: `decide_install_logic` in `src/provision.rs` called bare `is_ready_logic`,
    // declared in `src/engine.rs` — `error[E0425]: cannot find function 'is_ready_logic' in this
    // scope`, and no prover ran. The module is a fact the adapter already recorded.
    #[test]
    fn a_sibling_mirror_is_offered_by_its_crate_path() {
        let note = mirror_note(&[engine_mirror()]);
        assert!(
            note.contains("crate::engine::is_ready_logic"),
            "a cross-module call needs the path: {note}"
        );
    }

    // Verifies: provreq sets the mirror's visibility, whatever the model wrote. Rust needs it
    // reachable from the harness (a different module); Creusot needs it no LESS visible than the
    // function it mirrors — measured as *cannot make `engine::is_ready_logic` transparent in
    // `engine::EngineStatus::is_ready` as it would call a less-visible item*, from a `pub fn`
    // linked to a `pub(crate)` mirror.
    #[test]
    fn a_staged_mirror_is_visible_to_everything_that_must_see_it() {
        // The mirrored function's visibility is the floor AND the target.
        assert_eq!(
            mirror_visibility("pub fn is_ready(&self) -> bool { true }"),
            "pub"
        );
        assert_eq!(
            mirror_visibility("fn helper() -> bool { true }"),
            "pub(crate)"
        );
        assert_eq!(
            mirror_visibility("pub(crate) fn helper() -> bool { true }"),
            "pub(crate)",
            "a private or crate-visible function still needs a crate-visible mirror for the harness"
        );

        let private = "#[logic]\nfn f_logic(x: bool) -> bool { pearlite! { x } }";
        assert_eq!(
            make_visible(private, "f_logic", "pub(crate)"),
            "#[logic]\npub(crate) fn f_logic(x: bool) -> bool { pearlite! { x } }"
        );

        // A model's own guess is REPLACED, not respected — `pub(crate)` where `pub` is required is
        // exactly the case Creusot rejects.
        assert_eq!(
            make_visible(
                "#[logic]\npub(crate) fn f_logic(x: bool) -> bool { x }",
                "f_logic",
                "pub"
            ),
            "#[logic]\npub fn f_logic(x: bool) -> bool { x }"
        );

        // The one-line form a model emits just as readily. Matching only a line-initial `fn` left
        // this private — measured as `error[E0603]: function 'decide_install_logic' is private`.
        assert_eq!(
            make_visible(
                "#[logic] fn f_logic(x: bool) -> bool { pearlite! { x } }",
                "f_logic",
                "pub"
            ),
            "#[logic] pub fn f_logic(x: bool) -> bool { pearlite! { x } }"
        );
    }

    fn engine_mirror() -> MirrorDraft {
        MirrorDraft {
            file: "src/engine.rs".to_string(),
            line: 2,
            name: "is_ready_logic".to_string(),
            path: "crate::engine::is_ready_logic".to_string(),
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
        let p = build_prompt("intent", "claim", "fn f() -> bool { true }", "f_logic", "");
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

    // Verifies: a mirror's prompt names the OTHER mirrors in the same run, and not itself. Measured
    // failure: told only "do not call the program function", a live model wrote
    // `detected.is_ready()` inside `decide_install_logic` — it had no legal way to ask the question,
    // and the staged harness failed to compile. The peer list is that legal way.
    #[test]
    fn a_prompt_names_the_sibling_mirrors_its_body_may_call() {
        let targets = vec![
            (
                CodeMatch {
                    file: "src/provision.rs".into(),
                    line: 79,
                    text: "fn decide_install(".into(),
                    module: Some(vec!["provision".into()]),
                },
                "decide_install".to_string(),
                "decide_install_logic".to_string(),
                "fn decide_install(…) {}".to_string(),
                "crate::provision::decide_install_logic".to_string(),
            ),
            (
                CodeMatch {
                    file: "src/engine.rs".into(),
                    line: 131,
                    text: "fn is_ready(".into(),
                    module: Some(vec!["engine".into()]),
                },
                "EngineStatus::is_ready".to_string(),
                "is_ready_logic".to_string(),
                "fn is_ready(&self) -> bool {}".to_string(),
                "crate::engine::is_ready_logic".to_string(),
            ),
        ];
        let note = peer_note(&targets, "decide_install_logic");
        assert!(
            note.contains("`is_ready` → call `crate::engine::is_ready_logic(…)`"),
            "the sibling's mirror must be offered by its crate path: {note}"
        );
        assert!(
            !note.contains("decide_install_logic"),
            "a mirror is never offered itself"
        );
        assert!(
            peer_note(&targets[..1], "decide_install_logic").is_empty(),
            "a lone mirror gets no list at all"
        );
    }

    // Verifies: the receiver rule is explicit about BOTH things a live model got wrong — it named
    // the parameter `self` (legal only in an associated function, so the staged edit would not
    // compile) and dropped the reference from the type.
    #[test]
    fn the_prompt_forbids_naming_a_free_function_parameter_self() {
        let p = build_prompt("i", "c", "fn f(&self) -> bool { true }", "f_logic", "");
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
        assert_eq!(drafts.drafts.len(), 1, "same function, one mirror");
        assert_eq!(drafts.drafts[0].name, "is_ready_logic");
        assert!(drafts.dropped.is_empty(), "nothing was given up on");
    }

    // Verifies (#170): a model that declines (or answers unusably) yields NO draft for that
    // function — never a fabricated mirror standing in for one — and the abandoned target is
    // REPORTED. It used to be dropped in silence, which on a real subject left the harness calling
    // the program function the prover had already named, with nothing said about it.
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
        assert!(drafts.drafts.is_empty(), "nothing fabricated");
        assert_eq!(
            drafts.dropped.len(),
            1,
            "and the operator is told: {drafts:?}"
        );
        let d = &drafts.dropped[0];
        assert_eq!(d.function, "is_ready");
        assert_eq!(d.name, "is_ready_logic");
        assert_eq!(d.file, "src/engine.rs");
        assert_eq!(d.line, 2);
        assert_eq!(d.wall, DropWall::NoMirrorInReply);
    }

    // Verifies (#170): a mirror provreq cannot LINK is dropped — and named. A parameter written as
    // a pattern gives `link_for` no name to apply the mirror to, and a mirror without its link is
    // an unchecked meaning rather than a weaker proof, so it must not be staged. The operator still
    // has to learn that this predicate has no mirror, or they read a hole as a complete draft.
    #[tokio::test]
    async fn a_mirror_that_cannot_be_linked_is_dropped_and_named() {
        let backend = StubBackend {
            reply: "#[logic]\npub fn decide_logic(a: bool, b: bool) -> bool { pearlite! { a } }\n"
                .to_string(),
            prompts: std::sync::Mutex::new(Vec::new()),
        };
        let mut resolutions = BTreeMap::new();
        resolutions.insert(
            "granted".to_string(),
            resolved(
                "src/access.rs",
                1,
                "pub fn decide((a, b): (bool, bool)) -> bool {",
            ),
        );
        let mut sources = BTreeMap::new();
        sources.insert(
            "src/access.rs".to_string(),
            "pub fn decide((a, b): (bool, bool)) -> bool { a }
"
            .to_string(),
        );
        let drafts = Mirrorer::new(backend)
            .draft("intent", "claim", &resolutions, &sources)
            .await
            .expect("draft");
        assert!(
            drafts.drafts.is_empty(),
            "an unlinkable mirror is not staged"
        );
        assert_eq!(drafts.dropped.len(), 1, "and it is named: {drafts:?}");
        assert_eq!(drafts.dropped[0].wall, DropWall::Unlinkable);
        assert_eq!(drafts.dropped[0].function, "decide");
    }
}
