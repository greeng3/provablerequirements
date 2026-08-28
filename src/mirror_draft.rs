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

use crate::llm::{user_request, LlmBackend};
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

/// The ways a mirror is abandoned. Each is a different thing for the operator to do, which is
/// why the verdict names which one rather than reporting a generic failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropWall {
    /// The model returned nothing that parses as the requested mirror function.
    NoMirrorInReply,
    /// A mirror was returned but is not a well-formed item — splicing it would break the source.
    MalformedItem,
    /// provreq could not build the linking `#[ensures]` from the function's signature.
    Unlinkable,
    /// provreq could not write the mirror's own signature from the function's source (#191) — a
    /// parameter is a pattern rather than a plain name, a type it will not render (a tuple, a
    /// slice, an `impl Trait`), or the function returns nothing for a mirror to return.
    Unwritable,
    /// The body calls a **mirror that does not exist** (#202) — a name with provreq's own mirror
    /// suffix that is neither staged this round nor already declared by the subject.
    ///
    /// Kept apart from every other wall because of what staging it would do. The others risk an
    /// unchecked meaning; this one **stops the subject from compiling**, so every requirement's
    /// verdict degrades, not just this one's, and the operator's working tree is left broken by the
    /// tool that was meant to help. Measured: a model asked for a mirror of a function calling a
    /// trait method — which has no mirror, because a trait method cannot yet be resolved (#200) —
    /// invented `crate::status::is_healthy_logic`, and Creusot answered `error[E0425]: cannot find
    /// function`.
    CallsUnstagedMirror {
        /// The name it invented, so the operator is told what to look for rather than hunting.
        called: String,
    },
}

impl DropWall {
    /// What stopped it, and what the operator can do — the same shape as an engine's own limit.
    pub fn explain(&self) -> String {
        match self {
            DropWall::CallsUnstagedMirror { called } => format!(
                "the proposed mirror calls `{called}`, which does not exist — it is neither a \
                 mirror provreq staged in this round nor one the subject already declares. Staging \
                 it would leave the subject unable to compile, so it is dropped instead. This \
                 usually means the function it mirrors calls something that has no mirror of its \
                 own, and the model invented one rather than being told it could not; write this \
                 mirror by hand, or ground that other predicate so it gets a mirror too"
            ),
            _ => self.explain_fixed().to_string(),
        }
    }

    /// The walls whose explanation is the same every time.
    fn explain_fixed(&self) -> &'static str {
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
            DropWall::Unwritable => {
                "provreq could not write this mirror's signature from the function's own source — a \
                 parameter is a pattern rather than a plain name, a type is one it will not render \
                 exactly (a tuple, a slice, an `impl Trait`), or the function returns nothing. The \
                 signature is provreq's to write, so it declines rather than asking the model to \
                 guess it"
            }
            DropWall::Unlinkable => {
                "provreq could not build the linking `#[ensures]` from this signature (a parameter \
                 is a pattern rather than a plain name), and a mirror without its link is an \
                 unchecked meaning, never a weaker proof — so it is not staged at all"
            }
            // Carries a name, so its text is built per-drop by `explain`.
            DropWall::CallsUnstagedMirror { .. } => unreachable!("explained with its own name"),
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
    format!("{base}{MIRROR_SUFFIX}")
}

/// What makes a name a mirror's. provreq owns this suffix — every mirror name it ever writes is
/// derived through [`mirror_name`] — which is what lets it say authoritatively that a mirror-shaped
/// call names nothing (#202).
const MIRROR_SUFFIX: &str = "_logic";

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

        // A mirror an earlier round already committed lives in the subject's own source, and calling
        // it is correct — so this set is not an indulgence, it is what keeps the check below from
        // dropping a mirror that would have compiled.
        let declared_mirrors: BTreeSet<String> =
            sources.values().flat_map(|t| declared_fns(t)).collect();

        let mut out = MirrorDrafts::default();
        for (at, observable, name, fn_src, path) in &targets {
            let peers = peer_note(&targets, name);
            let dropped = |wall: DropWall| DroppedMirror {
                file: at.file.clone(),
                line: at.line,
                function: observable.clone(),
                name: name.clone(),
                wall,
            };
            // provreq writes the signature; the model is asked only for the meaning (#191).
            let self_ty = sources
                .get(&at.file)
                .and_then(|text| crate::rust_adapter::impl_type_at(text, at.line));
            let Some(signature) =
                mirror_signature(fn_src, name, self_ty.as_deref(), mirror_visibility(fn_src))
            else {
                out.dropped.push(dropped(DropWall::Unwritable));
                continue;
            };
            let reply = self
                .backend
                .run_prompt(&user_request(build_prompt(
                    intent, claim, fn_src, &signature, &peers,
                )))
                .await?
                .text;
            let Some(body) = parse_body(&reply) else {
                out.dropped.push(dropped(DropWall::NoMirrorInReply));
                continue;
            };
            // A bare call to a sibling mirror is a path provreq already knows (#192).
            let siblings: Vec<(String, String)> = targets
                .iter()
                .filter(|(_, _, n, _, _)| n != name)
                .map(|(_, _, n, _, p)| (n.clone(), p.clone()))
                .collect();
            let body = qualify_peer_calls(&body, &siblings);
            let item = format!("#[logic(open)]\n{signature} {{\n    {body}\n}}");
            // Still checked: the model's body can be unbalanced or not an expression, and splicing
            // that would break the subject's source (see [`is_well_formed_item`]).
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
        drop_mirrors_calling_nothing(&mut out, &declared_mirrors, &targets);
        Ok(out)
    }
}

/// Drop every mirror whose body calls a mirror that is **not going to exist** (#202), repeating
/// until none is left.
///
/// This runs after drafting, not during it, and that is the whole point. Whether a call is legal
/// depends on what provreq *actually staged*, and that is not known while it is still deciding: the
/// set of intended targets is known up front, but a target can still be dropped for its own reasons
/// afterwards. Measured live, checking against the intended set instead: the invented call was
/// caught and its mirror dropped, and the mirror that *called that dropped mirror* was staged
/// anyway — so the subject still did not compile, with `error[E0425]: cannot find function
/// healthy_now_logic`, one name further along. The passing test did not see it; the run did.
///
/// Hence the loop: dropping one mirror can invalidate another that called it, and that can cascade.
/// It terminates because every pass removes at least one draft.
fn drop_mirrors_calling_nothing(
    out: &mut MirrorDrafts,
    declared: &BTreeSet<String>,
    targets: &[(CodeMatch, String, String, String, String)],
) {
    loop {
        let live: Vec<String> = out.drafts.iter().map(|d| d.name.clone()).collect();
        let live: BTreeSet<&str> = live.iter().map(String::as_str).collect();
        let found = out.drafts.iter().enumerate().find_map(|(i, d)| {
            unstaged_mirror_call(&d.item, &live, declared).map(|called| (i, called))
        });
        let Some((i, called)) = found else { return };
        let gone = out.drafts.remove(i);
        out.dropped.push(DroppedMirror {
            file: gone.file,
            line: gone.line,
            // The observable is what the operator knows the function by; the draft carries only the
            // mirror name, so it comes back from the targets that produced it.
            function: targets
                .iter()
                .find(|(_, _, n, _, _)| *n == gone.name)
                .map(|(_, observable, _, _, _)| observable.clone())
                .unwrap_or_else(|| gone.name.clone()),
            name: gone.name,
            wall: DropWall::CallsUnstagedMirror { called },
        });
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

/// The mirror's whole signature, derived from the function's own source (#191).
///
/// The model is asked for the mirror's **body** and nothing else, because a signature is mechanics
/// and mechanics are provreq's job — the same rule that already makes the linking `#[ensures]`, the
/// visibility and the transparency provreq's to write. Measured twice against a live model: told in
/// the prompt that the claim's sorts are the requirement's vocabulary and not Rust types, it still
/// wrote `pub fn is_clear_logic(s: &Ent)`, taking the sort symbol straight out of
/// `state clear(e: Ent)`. Restating the rule was not going to fix it, and a mirror is drafted once
/// by design, so each miss is an operator correction rather than a repair round.
///
/// `self_ty` is the enclosing `impl` type for a method ([`crate::rust_adapter::impl_type_at`]) —
/// the one thing a method's source does not carry. A receiver becomes an ordinary first parameter
/// keeping its reference, named `s`, because `self` is legal only in an associated function.
fn mirror_signature(
    fn_src: &str,
    mirror_name: &str,
    self_ty: Option<&str>,
    vis: &str,
) -> Option<String> {
    let item: syn::ItemFn = syn::parse_str(fn_src).ok()?;
    let mut params = Vec::new();
    for arg in &item.sig.inputs {
        match arg {
            syn::FnArg::Receiver(r) => {
                let ty = self_ty?;
                let amp = if r.reference.is_some() { "&" } else { "" };
                params.push(format!("s: {amp}{ty}"));
            }
            syn::FnArg::Typed(t) => {
                let syn::Pat::Ident(name) = &*t.pat else {
                    return None;
                };
                params.push(format!("{}: {}", name.ident, render_ty(&t.ty)?));
            }
        }
    }
    let returns = match &item.sig.output {
        syn::ReturnType::Default => return None,
        syn::ReturnType::Type(_, ty) => render_ty(ty)?,
    };
    Some(format!(
        "{vis} fn {mirror_name}({}) -> {returns}",
        params.join(", ")
    ))
}

/// A parsed type back as source, rendered by structure — the crate has no `quote`, and the shapes a
/// predicate signature actually uses are few.
///
/// Deliberately narrow: a reference or a plain path, and nothing else. A type this cannot write is
/// `None`, which drops the mirror with a named reason rather than staging a signature provreq
/// guessed at — the same rule as a mirror it cannot link.
///
/// Type arguments are now **written out** rather than refused (#187): a sort may mean
/// `Wrapper<u32>`, so a predicate taking one has a mirror, and the mirror's signature must say the
/// same type the program function does. What is still refused is a type this cannot reproduce
/// exactly — writing `Wrapper` for `Wrapper<u32>` was never the alternative, and is not one now.
fn render_ty(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Reference(r) => {
            let inner = render_ty(&r.elem)?;
            Some(match r.mutability {
                Some(_) => format!("&mut {inner}"),
                None => format!("&{inner}"),
            })
        }
        syn::Type::Path(p) if p.qself.is_none() => {
            let mut out = Vec::new();
            let last = p.path.segments.len().saturating_sub(1);
            for (i, seg) in p.path.segments.iter().enumerate() {
                // Only the final segment may carry arguments. A mid-path `a::B<u32>::C` is a shape
                // this does not write, and guessing at where the arguments belong is exactly the
                // kind of approximation the refusal exists to prevent.
                let args = match (&seg.arguments, i == last) {
                    (syn::PathArguments::None, _) => String::new(),
                    (syn::PathArguments::AngleBracketed(a), true) => {
                        let mut parts = Vec::new();
                        for arg in &a.args {
                            match arg {
                                syn::GenericArgument::Type(t) => parts.push(render_ty(t)?),
                                _ => return None,
                            }
                        }
                        format!("<{}>", parts.join(", "))
                    }
                    _ => return None,
                };
                out.push(format!("{}{args}", seg.ident));
            }
            (!out.is_empty()).then(|| out.join("::"))
        }
        _ => None,
    }
}

/// The first mirror-shaped call in `item` that names no mirror — neither one being staged this
/// round (`staged`) nor one the subject already declares (`declared`) — or `None` when every such
/// call resolves (#202).
///
/// Only names carrying [`MIRROR_SUFFIX`] are judged, and that narrowness is deliberate. provreq
/// *owns* that suffix — every mirror name it writes comes from [`mirror_name`] — so it can say with
/// authority that such a name is invented. A call to an ordinary **program** function is a different
/// mistake with an owner that already reports it well: Creusot answers `called program function f in
/// logic context`, which is the wall this whole channel exists to route around, and duplicating that
/// judgement here would mean guessing at name resolution provreq does not do.
///
/// A path is read at its last segment, because `crate::status::is_healthy_logic` and a bare
/// `is_healthy_logic` are the same claim about the same missing item — and the qualified form is
/// exactly what a model writes when it has been shown peer paths.
fn unstaged_mirror_call(
    item: &str,
    staged: &BTreeSet<&str>,
    declared: &BTreeSet<String>,
) -> Option<String> {
    called_names(item).into_iter().find(|called| {
        called.ends_with(MIRROR_SUFFIX)
            && !staged.contains(called.as_str())
            && !declared.contains(called)
    })
}

/// Every identifier in `src` that is immediately applied to an argument list — the last segment of a
/// path, so `a::b::f(x)` reads as `f`. Deliberately syntactic: this is a scan for one specific
/// mistake, not a resolver.
///
/// The mirror's own signature (`fn name_logic(…)`) matches this shape too, which is harmless because
/// its name is always in the staged set — the mirror being drafted is one of the targets.
fn called_names(src: &str) -> Vec<String> {
    let chars: Vec<char> = src.chars().collect();
    let is_word = |c: char| c == '_' || c.is_alphanumeric();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !is_word(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_word(chars[i]) {
            i += 1;
        }
        let mut j = i;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        if chars.get(j) == Some(&'(') {
            out.push(chars[start..i].iter().collect());
        }
    }
    out
}

/// The names of the functions a source file declares, for the mirrors an earlier round committed.
/// Syntactic, and matching [`called_names`]'s narrowness on purpose: it feeds one comparison.
fn declared_fns(src: &str) -> Vec<String> {
    src.match_indices("fn ")
        .filter_map(|(i, _)| {
            let rest = &src[i + 3..];
            let name: String = rest
                .chars()
                .take_while(|c| *c == '_' || c.is_alphanumeric())
                .collect();
            (!name.is_empty()).then_some(name)
        })
        .collect()
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
///
/// The last two are the typing rules, and they matter more than their size suggests, because a
/// mirror is drafted **once** — it states what a function means, and prover failure does not change
/// its meaning, so there is no repair round to absorb a type error. On the simplest numeric
/// predicate a live model can write (`*failures == 0`) both fire in turn: matching `s: &Session`
/// binds `failures: &u32`, and then an unsuffixed literal is `Int`, so correcting the obvious
/// mistake reproduces `error[E0308]` at the next column and only `*failures == 0u32` compiles
/// (#181). Neither is something a Rust programmer would guess; both are mechanical facts about the
/// language the model is being asked to write.
pub const PEARLITE_RULES: &str = "\
Pearlite rules you must obey:\n\
- NO macros. `matches!`, `assert!`, `println!` and friends are rejected outright. Write a `match` \
expression instead of `matches!`.\n\
- Do NOT call the program function, or any other ordinary (non-`#[logic]`) function, from inside a \
specification. Only `#[logic]` functions and pure pearlite are available there.\n\
- Use `==>` for implication. There is NO `<==>` operator — for a biconditional between two \
booleans write `==`.\n\
- An unsuffixed integer literal is NOT the type beside it, so a comparison against a sized \
integer needs that type's suffix: write `n == 0u32`, never `n == 0`. Do NOT cast — there is no \
`as` conversion available here; the suffix is the whole fix.\n\
- A `match` on a REFERENCE binds its fields by reference, so reading one needs a deref. Matching \
`s: &Session` on `Session::SignedIn { failures }` binds `failures: &u32`, and the comparison is \
`*failures == 0u32`.\n";

/// Build the mirror-drafting prompt for one function (pure).
fn build_prompt(intent: &str, claim: &str, fn_src: &str, signature: &str, peers: &str) -> String {
    format!(
        "You are writing the BODY of a Creusot LOGIC MIRROR for one Rust function. A mirror is a \
`#[logic]` function that states, in Creusot's specification language (pearlite), exactly what the \
program function means. The program function is then linked to it by a post-condition, and the \
prover CHECKS the mirror against the real body — so state the function's actual meaning, never a \
guess.\n\n\
provreq has already written the signature, and will wrap your answer in it exactly as shown:\n\n\
{signature} {{\n    <YOUR ANSWER>\n}}\n\n\
Respond with EXACTLY ONE thing and nothing else — no prose, no code fences, no explanation, no \
signature, no attributes: the body, which must be `pearlite! {{ ... }}`. Refer to the parameters by \
the names in the signature above.\n\n\
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
/// Pull the pearlite BODY out of a model reply (#191).
///
/// The model is now asked for a body and nothing else, so this takes the `pearlite! { … }` block
/// through its own balanced brace — the same balance rule [`parse_mirror`] uses on an item, and for
/// the same reason: an unbalanced reply is truncated or malformed, and splicing it would break the
/// subject's source.
///
/// A reply that wraps the body in a whole function anyway is still usable: the `pearlite!` block is
/// found wherever it sits, and the signature around it is discarded — which is the point, since the
/// signature is the half the model kept getting wrong.
fn parse_body(reply: &str) -> Option<String> {
    let cleaned: String = reply
        .lines()
        .filter(|l| !l.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n");
    // Balance from the `pearlite!` token, NOT from the line it sits on: a reply that wrapped the
    // body in a whole function would otherwise carry that function's closing brace along with it.
    let from = cleaned.find("pearlite!")?;
    let rest = &cleaned[from..];
    let open = rest.find('{')?;
    let mut depth = 0usize;
    for (i, c) in rest.char_indices().skip(open) {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(rest[..=i].trim().to_string());
                }
            }
            _ => {}
        }
    }
    None
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
/// Rewrite a bare call to a sibling mirror into its full crate path (#192).
///
/// A mirror body legitimately calls its peers, but the peers live in other modules, so a bare call
/// does not compile: `is_clear_logic(entry)` inside `src/book.rs` when the mirror is declared in
/// `src/pending.rs` is `error[E0425]: cannot find function ... in this scope`, and the whole harness
/// fails.
///
/// The prompt already offers each peer's crate path, and the offer was **present and ignored** —
/// `targets` is built in full before any drafting, so `peer_note` had the path and stated it.
/// Asking again is not a fix; a mirror is drafted once by design, so every miss is an operator
/// correction rather than a repair round. This is the same rule the channel already applies to the
/// linking `#[ensures]`, the visibility and the transparency: mechanics are provreq's job, and only
/// the *meaning* is the model's. A call's path is mechanics.
///
/// Only a call is rewritten — the name must be followed by `(` — and never one that is already
/// qualified, so a model that did follow the instruction is left alone.
fn qualify_peer_calls(item: &str, peers: &[(String, String)]) -> String {
    peers.iter().fold(item.to_string(), |body, (name, path)| {
        let mut out = String::with_capacity(body.len());
        let mut rest = body.as_str();
        while let Some(i) = rest.find(name.as_str()) {
            let (before, at) = rest.split_at(i);
            let after = &at[name.len()..];
            // A call, not a declaration or a substring of a longer identifier.
            let is_call = after.trim_start().starts_with('(');
            let whole_word = !before.ends_with(|c: char| c.is_alphanumeric() || c == '_')
                && !after.starts_with(|c: char| c.is_alphanumeric() || c == '_');
            let already_qualified = before.ends_with("::");
            out.push_str(before);
            if is_call && whole_word && !already_qualified {
                out.push_str(path);
            } else {
                out.push_str(name);
            }
            rest = after;
        }
        out.push_str(rest);
        out
    })
}

fn is_well_formed_item(item: &str) -> bool {
    syn::parse_str::<syn::ItemFn>(item).is_ok()
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
    use crate::llm::{PromptRequest, PromptResponse};
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
        async fn run_prompt(&self, req: &PromptRequest) -> Result<PromptResponse> {
            self.prompts.lock().unwrap().push(
                req.messages
                    .last()
                    .map(|m| m.content.clone())
                    .unwrap_or_default(),
            );
            Ok(PromptResponse {
                text: self.reply.clone(),
                usage: None,
            })
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

    // Verifies: the prompt carries the two TYPING rules. Measured twice on `*failures == 0` against
    // real Creusot 0.12.0 (#181): matching a reference binds the field by reference, and an
    // unsuffixed literal is `Int`, so the obvious correction fails again at the next column. A
    // mirror is drafted once, so each miss is an operator correction rather than a repair round.
    #[test]
    fn the_prompt_states_the_typing_rules_that_cost_an_operator_correction() {
        let p = build_prompt("intent", "claim", "fn f() -> bool { true }", "f_logic", "");
        assert!(
            p.contains("NOT the type beside it"),
            "an unsuffixed literal's type must be stated"
        );
        // The rule must NOT name the mathematical-integer type. Naming it was the earlier wording,
        // and measured on the `ledger` subject the model reached straight for it — `*amount as Int`,
        // first as a type that was not in scope and then, once provreq imported it, as a cast Rust
        // does not allow (`error[E0605]: non-primitive cast`). The actionable half is the suffix;
        // the name was only ever the explanation, and it read as an invitation (#194).
        assert!(
            !p.contains("`Int`"),
            "naming the type invites using it: {p}"
        );
        assert!(
            p.contains("Do NOT cast"),
            "the wrong repair must be closed off"
        );
        assert!(
            p.contains("0u32"),
            "the suffix must be shown, not described"
        );
        assert!(
            p.contains("binds its fields by reference"),
            "matching a reference is the other half of the same error"
        );
    }

    // Verifies: #192 — a bare call to a sibling mirror is rewritten to its crate path at staging.
    // Measured on the `ledger` subject: `post_logic` in src/book.rs called `is_clear_logic(entry)`
    // bare while that mirror was declared in src/pending.rs, and the harness failed with
    // `error[E0425]: cannot find function `is_clear_logic` in this scope`. The prompt had already
    // offered the path — `targets` is built in full before drafting, so `peer_note` stated it — so
    // the offer was present and ignored. A mirror is drafted once, so asking again is not a fix.
    #[test]
    fn a_bare_call_to_a_sibling_mirror_is_qualified_at_staging() {
        let peers = [(
            "is_clear_logic".to_string(),
            "crate::pending::is_clear_logic".to_string(),
        )];
        let body = "pearlite! { if is_clear_logic(e) { A } else { B } }";
        assert_eq!(
            qualify_peer_calls(body, &peers),
            "pearlite! { if crate::pending::is_clear_logic(e) { A } else { B } }"
        );

        // A model that DID follow the instruction is left alone — never double-qualified.
        let already = "pearlite! { crate::pending::is_clear_logic(e) }";
        assert_eq!(qualify_peer_calls(already, &peers), already);

        // Only calls: the mirror's own declaration and a longer identifier are untouched.
        let decl = "pub fn is_clear_logic_helper = 1; let is_clear_logic_x = is_clear_logic;";
        assert_eq!(
            qualify_peer_calls(decl, &peers),
            decl,
            "a non-call occurrence must not be rewritten"
        );
    }

    // Verifies: #191 — the prompt asks for a BODY and shows the signature provreq will wrap it in.
    // Telling the model that the claim's sorts are not Rust types was tried first and measurably
    // did not work: it wrote `pub fn is_clear_logic(s: &Ent)` anyway, straight out of
    // `state clear(e: Ent)`. A mirror is drafted once, so the fix had to remove the opportunity
    // rather than restate the rule.
    #[test]
    fn the_prompt_asks_only_for_a_body_and_shows_the_signature() {
        let p = build_prompt(
            "i",
            "c",
            "fn f() -> bool { true }",
            "pub fn f_logic() -> bool",
            "",
        );
        assert!(
            p.contains("pub fn f_logic() -> bool"),
            "the exact signature must be shown: {p}"
        );
        assert!(p.contains("provreq has already written the signature"));
        assert!(
            p.contains("no signature, no attributes"),
            "the reply must be a body and nothing else"
        );
        assert!(p.contains("pearlite!"), "the body's required form");
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

    // Verifies: #191 — a method's receiver becomes an ordinary first parameter of the impl type,
    // written by provreq. This used to be a prompt rule the model could break, and did: it wrote
    // `s: &Self` (the receiver's real type, which names nothing at module level) and later `s: &Ent`
    // (the requirement's SORT symbol, which is not a Rust type at all). Neither can happen now —
    // the model never writes a signature.
    #[test]
    fn a_receiver_becomes_a_first_parameter_of_the_impl_type() {
        let sig = mirror_signature(
            "pub fn is_clear(&self) -> bool { true }",
            "is_clear_logic",
            Some("Entry"),
            "pub",
        )
        .expect("a method with a known impl type is writable");
        assert_eq!(sig, "pub fn is_clear_logic(s: &Entry) -> bool");

        // Without the impl type there is nothing honest to write, so it is refused rather than
        // guessed — the guess is exactly what produced `&Self` and `&Ent`.
        assert!(mirror_signature(
            "pub fn is_clear(&self) -> bool { true }",
            "is_clear_logic",
            None,
            "pub"
        )
        .is_none());
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
        assert_eq!(
            drafts.drafts.len(),
            1,
            "same function, one mirror; dropped: {:?}",
            drafts.dropped
        );
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

    // Verifies: #191 — a signature provreq cannot write is refused BEFORE the model is called, and
    // named. A pattern parameter used to reach the model, come back as a mirror, and be dropped at
    // the link step; now it never spends a request, because the signature is provreq's first job.
    #[tokio::test]
    async fn a_signature_provreq_cannot_write_is_dropped_and_named() {
        let backend = StubBackend {
            reply: "pearlite! { true }".to_string(),
            prompts: std::sync::Mutex::new(Vec::new()),
        };
        let mut resolutions = BTreeMap::new();
        resolutions.insert(
            "ready".to_string(),
            resolved(
                "src/engine.rs",
                2,
                "pub fn is_ready((a, b): (u8, u8)) -> bool {",
            ),
        );
        let mut sources = BTreeMap::new();
        sources.insert(
            "src/engine.rs".to_string(),
            "\npub fn is_ready((a, b): (u8, u8)) -> bool { true }\n".to_string(),
        );
        let drafts = Mirrorer::new(backend)
            .draft("intent", "claim", &resolutions, &sources)
            .await
            .expect("draft");
        assert!(
            drafts.drafts.is_empty(),
            "an unwritable signature stages nothing"
        );
        assert_eq!(drafts.dropped.len(), 1);
        assert_eq!(drafts.dropped[0].wall, DropWall::Unwritable);
        assert!(drafts.dropped[0]
            .wall
            .explain()
            .contains("pattern rather than a plain name"));
    }

    // Verifies: #202 — a mirror whose body calls a mirror that was never staged is DROPPED, not
    // spliced. This is the one drop wall where staging would not merely risk an unchecked meaning:
    // it stops the subject compiling, so every requirement's verdict degrades with it.
    //
    // The reply is the exact body a live model produced (6th pass): asked to mirror a function whose
    // own body calls a trait method — which has no mirror, because a trait method cannot be resolved
    // (#200) — it invented one and provreq spliced it. Creusot: `error[E0425]: cannot find function
    // is_healthy_logic in module crate::status`.
    #[tokio::test]
    async fn a_mirror_calling_a_mirror_that_was_never_staged_is_dropped() {
        let backend = StubBackend {
            reply: "pearlite! { crate::status::is_healthy_logic(status) }".to_string(),
            prompts: std::sync::Mutex::new(Vec::new()),
        };
        let src = "\npub fn healthy_now(status: &Status) -> bool { status.is_healthy() }\n";
        let resolutions = BTreeMap::from([(
            "healthy".to_string(),
            resolved(
                "src/job.rs",
                2,
                "pub fn healthy_now(status: &Status) -> bool {",
            ),
        )]);
        let sources = BTreeMap::from([("src/job.rs".to_string(), src.to_string())]);
        let drafts = Mirrorer::new(backend)
            .draft("intent", "claim", &resolutions, &sources)
            .await
            .expect("draft");

        assert!(
            drafts.drafts.is_empty(),
            "a mirror calling nothing must not be staged: {:?}",
            drafts.drafts
        );
        assert_eq!(drafts.dropped.len(), 1);
        assert_eq!(
            drafts.dropped[0].wall,
            DropWall::CallsUnstagedMirror {
                called: "is_healthy_logic".to_string()
            },
            "the drop names the invented call, so the operator is not left hunting"
        );
        let text = drafts.dropped[0].wall.explain();
        assert!(text.contains("is_healthy_logic"), "names it: {text}");
        assert!(
            text.contains("compile"),
            "says what staging would cost: {text}"
        );
    }

    // Verifies: #202 — dropping one mirror drops every mirror that CALLED it, transitively. The
    // first cut of this fix checked against the mirrors provreq *intended* to write, and a live run
    // showed why that is not enough: the invented call was caught and its mirror dropped, and the
    // mirror calling that dropped mirror was staged anyway, so the subject still did not compile —
    // `error[E0425]: cannot find function healthy_now_logic`, one name further along.
    //
    // Here `b_logic` is dropped for calling an invented name, so `a_logic`, which calls `b_logic`,
    // must fall with it. Nothing may be staged at all.
    #[test]
    fn dropping_a_mirror_drops_whatever_called_it() {
        let draft = |name: &str, body: &str| MirrorDraft {
            file: "src/m.rs".into(),
            line: 1,
            name: name.into(),
            path: format!("crate::m::{name}"),
            item: format!("#[logic(open)]\npub fn {name}(x: bool) -> bool {{ {body} }}"),
            link: String::new(),
        };
        let mut out = MirrorDrafts {
            drafts: vec![
                draft("a_logic", "pearlite! { crate::m::b_logic(x) }"),
                draft("b_logic", "pearlite! { crate::m::invented_logic(x) }"),
                draft("c_logic", "pearlite! { x }"),
            ],
            dropped: Vec::new(),
        };
        drop_mirrors_calling_nothing(&mut out, &BTreeSet::new(), &[]);

        assert_eq!(
            out.drafts
                .iter()
                .map(|d| d.name.as_str())
                .collect::<Vec<_>>(),
            vec!["c_logic"],
            "only the mirror that depends on nothing missing survives"
        );
        assert_eq!(out.dropped.len(), 2, "both the cause and its caller drop");
        let by_name: Vec<(&str, String)> = out
            .dropped
            .iter()
            .map(|d| (d.name.as_str(), format!("{:?}", d.wall)))
            .collect();
        assert!(
            by_name
                .iter()
                .any(|(n, w)| *n == "b_logic" && w.contains("invented_logic")),
            "the cause names what it invented: {by_name:?}"
        );
        assert!(
            by_name
                .iter()
                .any(|(n, w)| *n == "a_logic" && w.contains("b_logic")),
            "the caller names the mirror that went missing under it: {by_name:?}"
        );
    }

    // Verifies: #202 — the check judges ONLY provreq's own mirror suffix, and only names that are
    // really unknown. A peer being staged in the same round, the mirror's own recursive call, and a
    // mirror an earlier round already committed to the subject are all legitimate; so is a call to
    // an ordinary program function, whose refusal belongs to Creusot and is reported there.
    #[test]
    fn only_an_invented_mirror_name_is_refused() {
        let staged: BTreeSet<&str> = ["a_logic", "b_logic"].into_iter().collect();
        let declared: BTreeSet<String> = ["old_logic".to_string()].into_iter().collect();
        let check = |body: &str| unstaged_mirror_call(body, &staged, &declared);

        // Legitimate, each for its own reason.
        assert_eq!(check("pearlite! { b_logic(x) }"), None, "a staged peer");
        assert_eq!(check("pearlite! { a_logic(x) }"), None, "itself");
        assert_eq!(
            check("pearlite! { crate::m::old_logic(x) }"),
            None,
            "a mirror the subject already declares"
        );
        assert_eq!(
            check("pearlite! { ordinary_fn(x) }"),
            None,
            "a program call is Creusot's to refuse, not this scan's"
        );
        assert_eq!(check("pearlite! { *x < 3u32 }"), None, "no calls at all");

        // Invented — and read at the last segment, since that is what a qualified path claims.
        assert_eq!(
            check("pearlite! { c_logic(x) }").as_deref(),
            Some("c_logic")
        );
        assert_eq!(
            check("pearlite! { crate::status::is_healthy_logic(s) }").as_deref(),
            Some("is_healthy_logic"),
            "a qualified invented call is the shape a live model actually wrote"
        );
    }

    // Verifies: REQ069 — a mirror's signature writes type arguments out. A sort may mean
    // `Wrapper<u32>`, so a predicate taking one now has a mirror at all; before #187 this returned
    // `None` and dropped the mirror. Writing `Wrapper` for `Wrapper<u32>` was never the
    // alternative — a signature that disagrees with the program function cannot be linked to it.
    #[test]
    fn a_mirror_signature_writes_type_arguments_out() {
        let ty = |src: &str| {
            let item: syn::ItemFn = syn::parse_str(&format!("fn f(w: {src}) -> bool {{ true }}"))
                .expect("fixture parses");
            let syn::FnArg::Typed(t) = item.sig.inputs.first().expect("one param") else {
                panic!("typed param")
            };
            render_ty(&t.ty)
        };
        assert_eq!(ty("Wrapper<u32>").as_deref(), Some("Wrapper<u32>"));
        assert_eq!(ty("&Wrapper<u32>").as_deref(), Some("&Wrapper<u32>"));
        assert_eq!(
            ty("wrap::Wrapper<auth::User>").as_deref(),
            Some("wrap::Wrapper<auth::User>")
        );
        assert_eq!(ty("Pair<u32, User>").as_deref(), Some("Pair<u32, User>"));
        // Still refused: a shape this cannot reproduce exactly. A dropped mirror is the honest
        // outcome, an approximated signature is not.
        assert_eq!(ty("(u8, u8)"), None);
        assert_eq!(ty("Wrapper<'a, u32>"), None);
    }
}
