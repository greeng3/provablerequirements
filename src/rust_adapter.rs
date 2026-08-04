//! The category-1 adapter: resolve a PRL vocabulary predicate to **a state predicate at a
//! source location** in the subject's Rust — the shape the design's Adapters list requires
//! ("1 (code) a state predicate at a source location"), and the shape any verifier can
//! actually consume.
//!
//! Before REQ025 a cat-1 binding was a **substring**: `logged_in` ↦ the text `"fn login"`,
//! and "grounded" meant that text occurred somewhere in the tree. A substring cannot say
//! which function the predicate is evaluated in, what computes it, or whether the symbol
//! denotes a boolean over program state at all — so no engine could consume it. Now the
//! observable names a function, and grounding **resolves** it against the real syntax tree.
//!
//! **Syntax, not types.** `syn` parses; it does not type-check. `-> bool` is matched
//! syntactically, so a predicate returning `Result<bool>`, a type alias for `bool`, or a
//! generic `T = bool` is judged on how it is *written*. That limit is real, and
//! [`Resolution::describe`] states it in the operator's own read-back rather than letting
//! a resolved binding imply more checking than happened.
//!
//! Rust-only by design — R-eng-4's per-language adapter, and Rust is the first target, not
//! the model. `// ponytail: one language, no trait — a second language earns the seam.`
//!
//! Implements: REQ025 (cat-1 binding resolves to a state predicate at a source location).

use std::path::Path;
use walkdir::WalkDir;

/// Where a predicate lives in the subject: file (relative to the subject root), 1-based
/// line, and that source line's own text — so the operator confirms against the real code
/// rather than a signature this tool reconstructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeMatch {
    pub file: String,
    pub line: usize,
    pub text: String,
    /// The item's module path within its crate — `Some(vec![])` at the crate root,
    /// `Some(vec!["provision"])` for an item in `src/provision.rs`, and **`None` when this adapter
    /// cannot say** (REQ061).
    ///
    /// A harness has to name the item through this path, so `None` is not "the root": it is the
    /// refusal that keeps lowering from emitting a path it invented. Existence and reachability are
    /// separate questions, which is why this does not stop a binding from **resolving** — the item
    /// is really there, at that line, and grounding is right to say so.
    pub module: Option<Vec<String>>,
}

/// Whether a directory is pruned from the walk: the companion tree (whose `drafts.yml` holds the
/// observables themselves — resolving there would be a spurious self-hit), or anything
/// [`crate::subject_tree::is_pruned_dir`] excludes from every walk of a subject.
fn is_skipped_dir(path: &Path, depth: usize, companion_root: &Path) -> bool {
    path == companion_root || crate::subject_tree::is_pruned_dir(path, depth)
}

/// How one parameter of a resolved predicate takes its argument. The only thing an engine
/// needs in order to *call* the predicate, and — consistent with this module's
/// syntax-not-types limit — the only thing `syn` can honestly report: whether the parameter
/// is **written** as a reference.
///
/// `&mut` and a `self` receiver both read as [`ParamMode::ByRef`]. Neither is a sensible
/// state predicate, and a generated call would fail to compile rather than mislead — an
/// honest `unknown`, which is the right outcome for a shape we cannot faithfully call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    ByRef,
    ByValue,
}

/// How a resolved predicate is *called* (REQ055). Well-modelled Rust keeps decisions in enums and
/// behaviour on types, so requiring a free `fn … -> bool` at the binding boundary would mean the
/// more carefully a subject models its states, the less of it provreq can reach. The form carries
/// everything [`crate::lowering`] needs to emit the call, so the observable string is parsed once,
/// here, and never re-read downstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredicateForm {
    /// A free function written `-> bool`: `prefix::<observable>(args)`. Carries no name because
    /// for this form the observable *is* the call — only the qualified forms name something the
    /// binding string does not already say.
    Function,
    /// An inherent method written `-> bool`. The receiver is the predicate's **first** argument,
    /// so `state ready(u)` against `fn is_ready(&self)` lowers to `u.is_ready()`. Determined by
    /// the signature having a `self` receiver, not by how the operator wrote the binding — a
    /// method found by its bare name is still a method.
    Method { name: String },
    /// A function returning an enum, tested against one variant:
    /// `match prefix::name(args) { prefix::enum_name::variant { .. } => true, _ => false }`. The
    /// `{ .. }` form matches unit, tuple, and struct variants alike, so the binding does not have
    /// to restate the variant's shape. Written out rather than as `matches!`, which is the same
    /// thing but unreadable to a checker whose assertions are its own logic language (REQ062).
    VariantTest {
        name: String,
        enum_name: String,
        variant: String,
        /// The module the **enum** is declared in, which need not be the function's — the harness
        /// names the two independently (REQ061). `None` for the same reason
        /// [`CodeMatch::module`] is: nothing a harness can write would reach it.
        enum_module: Option<Vec<String>>,
    },
}

/// What resolving one cat-1 binding against the subject's Rust found. Every non-resolved
/// variant is a *distinct operator action* — a typo, a name collision, a wrong predicate,
/// or a non-boolean — so they stay distinct rather than collapsing to "not found".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Exactly one predicate of that name, with matching arity, that yields a boolean — either
    /// syntactically returning `bool` or naming one variant of the enum it returns. The only
    /// variant that grounds. `params` carries how each parameter takes its argument, in
    /// declaration order, so [`crate::lowering`] can generate a call that matches the subject's
    /// real signature instead of guessing at `&u` versus `u`; `form` says how to call it.
    Resolved {
        at: CodeMatch,
        params: Vec<ParamMode>,
        form: PredicateForm,
    },
    /// No function of that name anywhere in the subject's Rust.
    NotFound,
    /// Several functions share the name. Never guessed between — the operator must
    /// disambiguate, because picking one silently would bind the requirement to whichever
    /// file happened to be walked first.
    Ambiguous(Vec<CodeMatch>),
    /// Found, but it takes a different number of parameters than the PRL predicate.
    WrongArity {
        expected: usize,
        found: usize,
        at: CodeMatch,
    },
    /// Found with the right arity, but it is not written to return `bool`, so it cannot
    /// stand for a state predicate. A function returning an enum is *not* this: name a variant
    /// (`decide_install::Proceed`) and it becomes a boolean test (REQ055).
    NotBoolean { returns: String, at: CodeMatch },
    /// The function resolved, but the variant named after `::` is not one of the variants of the
    /// enum it returns (REQ055). Carries the real variants, because the useful answer to a
    /// misspelled variant is the list it was meant to be spelled from.
    NotAVariant {
        returns: String,
        variant: String,
        variants: Vec<String>,
        at: CodeMatch,
    },
    /// Found, right arity and boolean, but a parameter's written type is not the type the sort
    /// its argument ranges over is bound to (REQ057): `each u: User . logged_in(u)` bound to
    /// `fn login(s: &Session) -> bool`. Left unchecked this grounds green and then lowers to a
    /// harness that names a `User` where the subject wants a `Session`, so the operator learns
    /// the binding is wrong from a compiler error inside an `unknown` — from the wrong surface.
    WrongParamType {
        /// 1-based position, as the operator counts parameters.
        param: usize,
        /// The type the argument's sort is bound to.
        expected: String,
        /// The type the subject's signature actually writes there.
        found: String,
        at: CodeMatch,
    },
    /// The type resolved, but it has no inherent method of that name (REQ055). Carries the
    /// methods it does have, for the same reason.
    NoSuchMethod {
        ty: String,
        method: String,
        methods: Vec<String>,
        at: CodeMatch,
    },
}

impl Resolution {
    /// Whether this binding resolved — the single question [`crate::grounding::verdict`]
    /// asks. Only [`Resolution::Resolved`] grounds; everything else parks the requirement
    /// (R-ground-1: a no-resolve never fakes a verdict).
    pub fn is_resolved(&self) -> bool {
        matches!(self, Resolution::Resolved { .. })
    }

    /// The operator-facing read-back for one binding (D13: "here is what your binding
    /// resolves to — is that what you meant?"). A resolved predicate names the limit of
    /// what was actually checked, so a green line never implies a type-check that `syn`
    /// cannot perform.
    pub fn describe(&self, symbol: &str, observable: &str) -> String {
        match self {
            Resolution::Resolved { at, form, .. } => format!(
                "{symbol} → `{observable}` resolves to {}:{}  {}\n      ({}; syntactic check \
                 only — `syn` sees no types, so a `bool` alias or `Result<bool>` would \
                 pass here)",
                at.file,
                at.line,
                at.text,
                form.describe_call(symbol)
            ),
            Resolution::NotFound => format!(
                "{symbol}: nothing named `{observable}` resolves in the subject's Rust — a \
                 predicate binds to a function (`login`), an inherent method \
                 (`Session::is_active`), or one variant of the enum a function returns \
                 (`decide_install::Proceed`)"
            ),
            Resolution::Ambiguous(ats) => {
                let places = ats
                    .iter()
                    .map(|a| format!("{}:{}", a.file, a.line))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{symbol}: `{observable}` is ambiguous — {} functions share the name \
                     ({places}); qualify it, because binding to one silently would pick \
                     whichever file was walked first",
                    ats.len()
                )
            }
            Resolution::WrongArity {
                expected,
                found,
                at,
            } => format!(
                "{symbol}: `{observable}` at {}:{} takes {found} parameter(s), but the \
                 requirement declares {symbol} with {expected} — one of the two is wrong",
                at.file, at.line
            ),
            Resolution::NotBoolean { returns, at } => format!(
                "{symbol}: `{observable}` at {}:{} returns `{returns}`, not `bool` — a \
                 state predicate must be a boolean over program state. If `{returns}` is an \
                 enum, name the variant that makes the predicate true \
                 (`{observable}::<Variant>`)",
                at.file, at.line
            ),
            Resolution::NotAVariant {
                returns,
                variant,
                variants,
                at,
            } => format!(
                "{symbol}: `{observable}` at {}:{} returns `{returns}`, which has no variant \
                 `{variant}` — {}",
                at.file,
                at.line,
                if variants.is_empty() {
                    format!(
                        "no enum `{returns}` is declared in the subject's Rust, so there is \
                             no variant to test"
                    )
                } else {
                    format!("its variants are {}", variants.join(", "))
                }
            ),
            Resolution::WrongParamType {
                param,
                expected,
                found,
                at,
            } => format!(
                "{symbol}: `{observable}` at {}:{} takes `{found}` as parameter {param}, but the \
                 argument there ranges over a sort bound to `{expected}` — one of the two is \
                 wrong (written type names are compared, so an alias for `{expected}` would read \
                 as a mismatch here)",
                at.file, at.line
            ),
            Resolution::NoSuchMethod {
                ty,
                method,
                methods,
                at,
            } => format!(
                "{symbol}: `{ty}` at {}:{} has no inherent method `{method}` — {}",
                at.file,
                at.line,
                if methods.is_empty() {
                    "it has no inherent methods at all".to_string()
                } else {
                    format!("its methods are {}", methods.join(", "))
                }
            ),
        }
    }
}

impl PredicateForm {
    /// How the harness will call this predicate, in the operator's own terms. Part of the D13
    /// read-back: a binding that resolves through a method or a variant test is doing something
    /// the observable string alone does not show, so it says so.
    fn describe_call(&self, symbol: &str) -> String {
        match self {
            PredicateForm::Function => "called directly".to_string(),
            PredicateForm::Method { name } => {
                format!("an inherent method — checked as `<first argument of {symbol}>.{name}(…)`")
            }
            PredicateForm::VariantTest {
                name,
                enum_name,
                variant,
                ..
            } => format!(
                "checked as `match {name}(…) {{ {enum_name}::{variant} {{ .. }} => true, _ => \
                 false }}`"
            ),
        }
    }
}

/// What resolving one **sort** binding found (REQ026). Deliberately *not* [`Resolution`]:
/// arity and boolean-return cannot occur for a type, and an enum carrying variants a caller
/// can never see misstates the state space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeResolution {
    /// Exactly one `struct`, `enum`, or `type` alias of that name. Grounds.
    Resolved(CodeMatch),
    /// One of the language's own primitive types (REQ058). Grounds, and carries **no**
    /// [`CodeMatch`]: nothing in the subject declares `bool`, so pointing at a line of the
    /// subject's source would be a location this adapter invented.
    Primitive(String),
    /// No type of that name in the subject's Rust.
    NotFound,
    /// Several types share the name — never guessed between, for the same reason as
    /// [`Resolution::Ambiguous`].
    Ambiguous(Vec<CodeMatch>),
    /// The type name exists, but not under the module the operator qualified it with (#138) —
    /// `crate::api::User` where every `User` lives in `auth`.
    ///
    /// Kept apart from [`TypeResolution::NotFound`] because they send the operator to opposite
    /// places. "No type `User` in the subject" is false here and would start a hunt for something
    /// that is right there; what is wrong is the qualifier, and the fix is to read the paths the
    /// subject actually declares — which this carries.
    QualifierUnmatched {
        /// The bare type name, which does exist.
        name: String,
        /// Every declaration of that name, so the read-back can offer the real paths.
        candidates: Vec<CodeMatch>,
    },
}

impl TypeResolution {
    /// Whether this sort resolved. A quantified claim whose domain names no real type is not
    /// grounded (R-ground-1) — but a primitive is as real a domain as a declared type.
    pub fn is_resolved(&self) -> bool {
        matches!(
            self,
            TypeResolution::Resolved(_) | TypeResolution::Primitive(_)
        )
    }

    /// The operator-facing read-back for one sort binding (D13's "is that what you meant?").
    pub fn describe(&self, sort: &str, observable: &str) -> String {
        match self {
            TypeResolution::Resolved(at) => format!(
                "{sort} (sort) → `{observable}` resolves to {}:{}  {}",
                at.file, at.line, at.text
            ),
            TypeResolution::Primitive(name) => format!(
                "{sort} (sort) → `{observable}` is the Rust primitive `{name}` — the language's \
                 own type, not one the subject declares, so there is no source location to \
                 confirm it against"
            ),
            TypeResolution::NotFound => format!(
                "{sort} (sort): no type `{observable}` in the subject's Rust, and it is not a \
                 primitive — a quantified variable cannot range over it"
            ),
            TypeResolution::Ambiguous(ats) => {
                let places = ats
                    .iter()
                    .map(|a| format!("{}:{}", a.file, a.line))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{sort} (sort): `{observable}` is ambiguous — {} types share the name \
                     ({places}); qualify it by module{}",
                    ats.len(),
                    offer_paths(ats, observable)
                )
            }
            TypeResolution::QualifierUnmatched { name, candidates } => format!(
                "{sort} (sort): the type `{name}` exists, but not under the path `{observable}` \
                 qualifies it with — so it is the qualifier that is wrong, not the type{}",
                offer_paths(candidates, name)
            ),
        }
    }
}

/// The qualified forms an operator can actually write, taken from where the type is really
/// declared. Advice to "qualify it" is worth nothing without them: the module path is a fact of the
/// subject that provreq walked and the operator would otherwise have to reconstruct by hand.
///
/// A candidate whose module the walk could not determine (REQ061) is skipped rather than guessed
/// at, so an empty offer means provreq has nothing honest to suggest and says only that.
fn offer_paths(candidates: &[CodeMatch], name: &str) -> String {
    let bare = name.rsplit("::").next().unwrap_or(name);
    let offers: Vec<String> = candidates
        .iter()
        .filter_map(|at| at.module.as_ref())
        .filter(|m| !m.is_empty())
        .map(|m| format!("`{}::{bare}`", m.join("::")))
        .collect();
    if offers.is_empty() {
        String::new()
    } else {
        format!(" — try {}", offers.join(" or "))
    }
}

/// Resolve a PRL sort to a real Rust type: a `struct`, `enum`, or `type` alias of that
/// name (REQ026). Existence only — whether the type can be *instantiated* (Kani's
/// `Arbitrary`) is an engine's question, and the binding is core-owned, so answering it
/// here would bake one engine's shape into the core.
///
/// The observable may be **path-qualified** — `crate::auth::User`, `auth::User` — which is how an
/// operator says *which* `User` when two share a name (#138). Until this, a duplicated type name
/// was an [`TypeResolution::Ambiguous`] park whose own read-back said "qualify it", advice nothing
/// could act on: the sort side matched a bare ident only, while the predicate side ([`type_ident`])
/// already reduced any written path to its last segment. The two disagreed about what a written
/// path meant, and one of them offered a way out that did not exist.
///
/// See [`module_matches`] for exactly how much of a written path is checked, and why the rest
/// cannot be.
pub fn resolve_type(subject: &ParsedSubject, observable: &str) -> TypeResolution {
    let written = observable.trim();
    if written.is_empty() {
        return TypeResolution::NotFound;
    }
    let mut segments: Vec<&str> = written.split("::").map(str::trim).collect();
    // The last segment is the type; everything before it qualifies which one.
    let name = segments.pop().expect("split yields at least one segment");
    if name.is_empty() {
        return TypeResolution::NotFound;
    }
    let by_name = find_types(subject, name);
    let found: Vec<CodeMatch> = by_name
        .iter()
        .filter(|at| module_matches(at.module.as_deref(), &segments))
        .cloned()
        .collect();
    // The name is real and only the qualifier missed. Reporting `NotFound` here would deny a type
    // the operator can see in their own source.
    if found.is_empty() && !by_name.is_empty() {
        return TypeResolution::QualifierUnmatched {
            name: name.to_string(),
            candidates: by_name,
        };
    }
    match found.len() {
        // The subject declares nothing by that name — but the language may. A primitive is only
        // ever the fallback: a subject that declares its own `bool` has a source location the
        // operator can confirm against, and the read-back names it, so the declaration wins and
        // says so rather than being silently overruled by the language.
        0 if is_primitive(name) => TypeResolution::Primitive(name.to_string()),
        0 => TypeResolution::NotFound,
        1 => TypeResolution::Resolved(found.into_iter().next().expect("len checked")),
        _ => TypeResolution::Ambiguous(found),
    }
}

/// Whether a type declared in `module` answers to the qualifier an operator wrote (#138).
///
/// **How much of a path is checked: all of it.** The written qualifier, after dropping a leading
/// `crate`, must be a **suffix of the declaring module path** — every segment written is compared,
/// and the operator may start at any depth. `alpha::User`, `auth::alpha::User` and
/// `crate::auth::alpha::User` all name a `User` declared in `auth::alpha`.
///
/// Nothing written is treated as decoration, and that is the point. The tempting alternative —
/// compare only where the two overlap, so a leading crate name like `gatekeeper::session::Session`
/// is tolerated — cannot distinguish a crate name from a wrong module, because the module path
/// here is the *declaration* site within its crate and never includes the crate's own name.
/// Tolerating it would accept `totallywrong::session::Session` just as readily. So a segment
/// provreq cannot verify is a mismatch rather than a shrug, and the crate name is the one thing an
/// operator may not write.
///
/// That is a real cost, paid deliberately, and it is bounded by the read-back:
/// [`TypeResolution::QualifierUnmatched`] does not merely reject, it offers the paths the subject
/// actually declares — so writing the crate name out of habit costs one glance, not a hunt.
///
/// This is enough for the job the qualifier exists to do: `auth::User` and `api::User` differ on a
/// compared segment, so the ambiguity that made both a park is resolved.
///
/// `module` is `None` when the walk could not say where an item lives (REQ061). A qualifier then
/// cannot be checked at all, so it does not match — `None` is a refusal, never "the crate root",
/// and treating it as one would resolve a binding on a module path provreq invented. An unqualified
/// sort is unaffected: with nothing written to check, every candidate still matches.
/// The type name at the end of a written path — `session::Session` → `Session`, `Session` →
/// `Session`. The one form both the sort side and the parameter side can always produce.
fn last_segment(written: &str) -> &str {
    written.rsplit("::").next().unwrap_or(written).trim()
}

fn module_matches(module: Option<&[String]>, qualifier: &[&str]) -> bool {
    let qualifier: Vec<&str> = qualifier
        .iter()
        .copied()
        .skip_while(|s| *s == "crate")
        .collect();
    if qualifier.is_empty() {
        return true;
    }
    let Some(module) = module else {
        return false;
    };
    // A suffix, not an overlap: a qualifier deeper than the module path names segments the
    // declaration does not have, and those are exactly the ones provreq cannot verify.
    qualifier.len() <= module.len()
        && module
            .iter()
            .rev()
            .zip(qualifier.iter().rev())
            .all(|(m, q)| m == q)
}

/// Whether a name is one of Rust's own primitive types — a sort that grounds without the subject
/// declaring anything (REQ058), and the one kind of sort a harness must write **unprefixed**
/// (`crate::bool` does not compile), which is why [`crate::lowering`] asks this rather than
/// keeping its own list.
///
/// `str` is deliberately absent: it is unsized, so a quantifier ranging over it would lower to a
/// harness that cannot compile — an `unknown` with a compiler error, which is the failure this
/// tool exists to move earlier. `String` is absent for a different reason: it is a declared std
/// type, not a primitive, and admitting it would open "which std types may a subject quantify
/// over" without a reason to answer it yet.
pub fn is_primitive(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "char"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
    )
}

/// Every `struct`/`enum`/`type` alias named `name`, with the same walk and skip rules as
/// the predicate resolver.
fn find_types(subject: &ParsedSubject, name: &str) -> Vec<CodeMatch> {
    let mut out = Vec::new();
    subject.each(|file, rel, text, module| {
        collect_types(&file.items, name, rel, text, module, &mut out);
    });
    out
}

/// Walk items for a type named `name`, descending into inline modules.
fn collect_types(
    items: &[syn::Item],
    name: &str,
    rel: &str,
    text: &str,
    module: &Option<Vec<String>>,
    out: &mut Vec<CodeMatch>,
) {
    for item in items {
        let ident = match item {
            syn::Item::Struct(s) => Some(&s.ident),
            syn::Item::Enum(e) => Some(&e.ident),
            syn::Item::Type(t) => Some(&t.ident),
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_types(inner, name, rel, text, &inside(module, &m.ident), out);
                }
                None
            }
            _ => None,
        };
        if let Some(ident) = ident {
            if ident == name {
                out.push(at_ident(ident, rel, text, module));
            }
        }
    }
}

/// The module path inside an inline `mod` block — one segment deeper, or still nothing when the
/// enclosing file has no path a harness could use.
fn inside(module: &Option<Vec<String>>, ident: &syn::Ident) -> Option<Vec<String>> {
    module.as_ref().map(|m| {
        let mut deeper = m.clone();
        deeper.push(ident.to_string());
        deeper
    })
}

/// The [`CodeMatch`] for an item named by `ident`.
fn at_ident(ident: &syn::Ident, rel: &str, text: &str, module: &Option<Vec<String>>) -> CodeMatch {
    let line = ident.span().start().line;
    CodeMatch {
        file: rel.to_string(),
        line,
        text: source_line(text, line),
        module: module.clone(),
    }
}

/// Resolve `observable` (a function name) against the subject's Rust.
///
/// `params` describes the PRL predicate's declared parameters, one entry per parameter in
/// declaration order — so its **length is the arity** the subject's signature must match, and the
/// two can never desync. Each entry is the Rust type that position's argument is expected to take
/// (the type its sort is bound to), or `None` where the caller cannot honestly say — see
/// [`crate::grounding::expected_param_types`], which builds it.
///
/// Read-only over the subject and recomputed live — code moves under a binding exactly as prose
/// moves under a draft, so a resolution is never stored.
pub fn resolve(subject: &ParsedSubject, observable: &str, params: &[Option<String>]) -> Resolution {
    let name = observable.trim();
    let segments: Vec<&str> = name.split("::").map(str::trim).collect();
    match segments[..] {
        [one] if !one.is_empty() => resolve_bare(subject, one, params),
        [qualifier, member] if !qualifier.is_empty() && !member.is_empty() => {
            resolve_qualified(subject, qualifier, member, params)
        }
        // An empty observable, or a path deeper than `A::B`. Nothing this adapter understands
        // takes three segments, and guessing which two were meant would bind the requirement to
        // something the operator did not write.
        _ => Resolution::NotFound,
    }
}

/// A bare name: a free function, or an inherent method reached without qualifying its type. The
/// signature decides which — a `self` receiver makes it a method however it was written, because
/// lowering a method as a free call generates code that cannot compile (REQ055).
fn resolve_bare(subject: &ParsedSubject, name: &str, params: &[Option<String>]) -> Resolution {
    let found = find_functions(subject, name);
    match found.len() {
        0 => Resolution::NotFound,
        1 => {
            let f = found.into_iter().next().expect("len checked");
            let form = if f.has_receiver {
                PredicateForm::Method {
                    name: name.to_string(),
                }
            } else {
                PredicateForm::Function
            };
            classify(f, params, form)
        }
        _ => Resolution::Ambiguous(found.into_iter().map(|f| f.at).collect()),
    }
}

/// `A::B` — either a function `A` whose returned enum has a variant `B`, or a type `A` with an
/// inherent method `B`. Which one is decided by what `A` actually is in the subject; a name that
/// is both a function and a type is an ambiguity, never a guess.
fn resolve_qualified(
    subject: &ParsedSubject,
    qualifier: &str,
    member: &str,
    params: &[Option<String>],
) -> Resolution {
    let fns = find_functions(subject, qualifier);
    let types = find_types(subject, qualifier);
    match (fns.len(), types.len()) {
        (0, 0) => Resolution::NotFound,
        (1, 0) => variant_test(
            subject,
            fns.into_iter().next().expect("len checked"),
            member,
            params,
        ),
        (0, 1) => method_on(
            subject,
            types.into_iter().next().expect("len checked"),
            qualifier,
            member,
            params,
        ),
        _ => Resolution::Ambiguous(fns.into_iter().map(|f| f.at).chain(types).collect()),
    }
}

/// A function whose return type is an enum, narrowed to one variant. Arity is checked first, for
/// the same reason it is everywhere else: it names the more fundamental mismatch.
fn variant_test(
    subject: &ParsedSubject,
    f: FoundFn,
    variant: &str,
    params: &[Option<String>],
) -> Resolution {
    if f.params.len() != params.len() {
        return Resolution::WrongArity {
            expected: params.len(),
            found: f.params.len(),
            at: f.at,
        };
    }
    let mut enums = find_enums(subject, &f.returns);
    // Two enums of that name is the same unanswerable question as two functions: the variants would
    // have to be pooled from declarations in different modules, and the harness can only name one.
    if enums.len() > 1 {
        return Resolution::Ambiguous(enums.into_iter().map(|e| e.at).collect());
    }
    let declared = enums.pop();
    let variants = declared
        .as_ref()
        .map(|e| e.variants.clone())
        .unwrap_or_default();
    if !variants.iter().any(|v| v == variant) {
        return Resolution::NotAVariant {
            returns: f.returns,
            variant: variant.to_string(),
            variants,
            at: f.at,
        };
    }
    if let Some(mismatch) = wrong_param_type(&f, params) {
        return mismatch;
    }
    Resolution::Resolved {
        form: PredicateForm::VariantTest {
            name: f.name,
            enum_name: f.returns,
            variant: variant.to_string(),
            // The enum need not live where the function does, so the harness cannot reuse the
            // function's module for it (REQ061).
            enum_module: declared.and_then(|e| e.at.module),
        },
        at: f.at,
        params: f.params,
    }
}

/// An inherent method on a named type. Qualifying is how an operator disambiguates a method name
/// several types share — `is_ready` alone is an [`Resolution::Ambiguous`] as soon as two types
/// declare it.
fn method_on(
    subject: &ParsedSubject,
    ty_at: CodeMatch,
    ty: &str,
    method: &str,
    params: &[Option<String>],
) -> Resolution {
    let mut found = find_methods(subject, ty, Some(method));
    match found.len() {
        0 => Resolution::NoSuchMethod {
            ty: ty.to_string(),
            method: method.to_string(),
            methods: find_methods(subject, ty, None)
                .into_iter()
                .map(|f| f.name)
                .collect(),
            at: ty_at,
        },
        1 => {
            let f = found.pop().expect("len checked");
            let form = PredicateForm::Method {
                name: method.to_string(),
            };
            classify(f, params, form)
        }
        _ => Resolution::Ambiguous(found.into_iter().map(|f| f.at).collect()),
    }
}

/// One function declaration found in the subject, with the facts the check needs.
struct FoundFn {
    name: String,
    at: CodeMatch,
    params: Vec<ParamMode>,
    /// The comparable name of each parameter's written type, in declaration order — `None` for a
    /// position no written-name comparison can honestly speak about (see [`param_type_ident`]).
    param_types: Vec<Option<String>>,
    returns: String,
    /// Whether the signature takes a `self` receiver — the syntactic fact that makes it a method.
    has_receiver: bool,
}

/// Decide whether a single found function can stand for the predicate. The checks run
/// coarsest-first — arity, then return type, then parameter types — so the message names the most
/// fundamental mismatch rather than a consequence of it.
fn classify(f: FoundFn, params: &[Option<String>], form: PredicateForm) -> Resolution {
    if f.params.len() != params.len() {
        return Resolution::WrongArity {
            expected: params.len(),
            found: f.params.len(),
            at: f.at,
        };
    }
    if f.returns != "bool" {
        return Resolution::NotBoolean {
            returns: f.returns,
            at: f.at,
        };
    }
    if let Some(mismatch) = wrong_param_type(&f, params) {
        return mismatch;
    }
    Resolution::Resolved {
        at: f.at,
        params: f.params,
        form,
    }
}

/// The first parameter whose written type is not the one its argument's sort is bound to, if any
/// (REQ057). A position the caller could not speak for, and a type this adapter cannot compare by
/// name, are both skipped — the check only ever fires on two names that are both known and differ,
/// so it cannot turn a working binding into a park.
///
/// Both sides are compared on their **last segment**, because that is all either side can offer:
/// the parameter's type comes from [`type_ident`], which has always reduced a written path that
/// way, and a sort's observable may now carry a module qualifier (#138). Measured on a real
/// subject: without this, binding `Sess=session::Session` resolved the sort correctly and then
/// parked every predicate taking a `Session`, because `session::Session != Session` as strings —
/// so qualifying a sort, the one way out of an ambiguity, broke grounding instead. The doc above
/// promised this check could not turn a working binding into a park, and it had quietly stopped
/// being true.
///
/// The qualifier is not re-checked here, and should not be: [`type_ident`] cannot see which module
/// a parameter's type resolves to, so demanding agreement would compare a known name against an
/// unknown one. The sort resolver already checked the qualifier against the declaration, which is
/// where the fact actually lives.
fn wrong_param_type(f: &FoundFn, params: &[Option<String>]) -> Option<Resolution> {
    f.param_types
        .iter()
        .zip(params)
        .enumerate()
        .find_map(|(i, (found, expected))| {
            let (found, expected) = (found.as_ref()?, expected.as_ref()?);
            (last_segment(found) != last_segment(expected)).then(|| Resolution::WrongParamType {
                param: i + 1,
                expected: expected.clone(),
                found: found.clone(),
                at: f.at.clone(),
            })
        })
}

/// Every function named `name` in the subject's `.rs` files, including inside inline
/// `mod` blocks and `impl` blocks.
fn find_functions(subject: &ParsedSubject, name: &str) -> Vec<FoundFn> {
    let mut out = Vec::new();
    subject.each(|file, rel, text, module| {
        collect_fns(&file.items, name, rel, text, module, &mut out);
    });
    out
}

/// The module path a file's top-level items sit at, from the crate root — the standard cargo
/// layout, read off the path (REQ061).
///
/// `None` means **no harness can name items in this file**, and the three cases are different
/// reasons for the same answer:
/// - outside `src/` — `tests/`, `benches/`, `examples/` and the like are separate crate targets,
///   and a subject whose `[lib] path` is somewhere non-default is not something a path convention
///   can be trusted to know;
/// - `src/main.rs` or `src/bin/*.rs` — a binary target, which no harness can import;
/// - a path that is not valid UTF-8 or whose components are not usable as identifiers.
///
/// `src/lib.rs` is the crate root (`Some(vec![])`), `src/a/mod.rs` is `a`, and `src/a/b.rs` is
/// `a::b`.
fn file_module_path(rel: &str) -> Option<Vec<String>> {
    let rest = rel
        .strip_prefix("src/")
        .or_else(|| rel.strip_prefix("src\\"))?;
    if rest == "main.rs" || rest.starts_with("bin/") || rest.starts_with("bin\\") {
        return None;
    }
    let mut parts: Vec<String> = rest
        .split(['/', '\\'])
        .map(|s| s.trim_end_matches(".rs").to_string())
        .collect();
    // The crate root and a directory's own module file contribute no segment of their own.
    if let Some("lib" | "mod") = parts.last().map(String::as_str) {
        parts.pop();
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .then_some(parts)
}

/// Every parseable `.rs` file under the subject, walked and parsed **once**.
///
/// The single walk every resolver uses, so a predicate and a sort can never disagree about
/// which files count — the skip rules are the binding's semantics, not an implementation
/// detail. Unparseable files are skipped rather than failing the run: a subject may
/// legitimately hold a Rust file this parser cannot read (a newer edition, a generated
/// fixture), and one bad file must not blind the whole resolution.
///
/// It is a value the caller holds rather than a walk each lookup starts for itself, because each
/// lookup used to re-walk *and re-parse* the whole subject: one `A::B` binding costs three lookups
/// on its own (the qualifier as a function, the qualifier as a type, then the returned enum), so a
/// four-binding requirement came to roughly ten full parses of the same tree (#144). Resolution is
/// still recomputed live — code moves under a binding exactly as prose moves under a draft — and
/// that stays true whether the parse happens once or ten times; the freshness boundary is now the
/// [`load`](ParsedSubject::load) call, which is one grounding pass.
pub struct ParsedSubject {
    files: Vec<ParsedFile>,
}

/// One walked-and-parsed source file: its syntax tree, the text the tree's spans index into, its
/// subject-relative path, and the module path a harness would name it through (REQ061).
struct ParsedFile {
    ast: syn::File,
    rel: String,
    text: String,
    module: Option<Vec<String>>,
}

impl ParsedSubject {
    /// Walk and parse the subject once.
    pub fn load(subject_root: &Path, companion_root: &Path) -> Self {
        let mut files = Vec::new();
        for entry in WalkDir::new(subject_root)
            .into_iter()
            .filter_entry(|e| !is_skipped_dir(e.path(), e.depth(), companion_root))
        {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_file() || entry.path().extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let Ok(ast) = syn::parse_file(&text) else {
                continue;
            };
            let rel = entry
                .path()
                .strip_prefix(subject_root)
                .unwrap_or(entry.path())
                .display()
                .to_string();
            let module = file_module_path(&rel);
            files.push(ParsedFile {
                ast,
                rel,
                text,
                module,
            });
        }
        Self { files }
    }

    /// Visit every parsed file. The finders differ only in what they collect, so they share this.
    fn each(&self, mut visit: impl FnMut(&syn::File, &str, &str, &Option<Vec<String>>)) {
        for f in &self.files {
            visit(&f.ast, &f.rel, &f.text, &f.module);
        }
    }
}

/// Walk items for functions named `name`, descending into inline modules and impl blocks
/// so a predicate declared inside one is still found.
fn collect_fns(
    items: &[syn::Item],
    name: &str,
    rel: &str,
    text: &str,
    module: &Option<Vec<String>>,
    out: &mut Vec<FoundFn>,
) {
    for item in items {
        match item {
            syn::Item::Fn(f) if f.sig.ident == name => {
                out.push(found(&f.sig, rel, text, None, module))
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_fns(inner, name, rel, text, &inside(module, &m.ident), out);
                }
            }
            syn::Item::Impl(i) => {
                let self_ty = type_ident(&i.self_ty);
                for sub in &i.items {
                    if let syn::ImplItem::Fn(f) = sub {
                        if f.sig.ident == name {
                            out.push(found(&f.sig, rel, text, self_ty.as_deref(), module));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// One enum declaration found in the subject: where it is (so a harness can name it through its
/// module, REQ061) and the variants it offers.
struct FoundEnum {
    at: CodeMatch,
    variants: Vec<String>,
}

/// Every enum named `name` — empty when the subject declares none, which includes the ordinary case
/// of a function that simply returns something that is not an enum (REQ055).
fn find_enums(subject: &ParsedSubject, name: &str) -> Vec<FoundEnum> {
    let mut out = Vec::new();
    subject.each(|file, rel, text, module| {
        collect_enums(&file.items, name, rel, text, module, &mut out);
    });
    out
}

fn collect_enums(
    items: &[syn::Item],
    name: &str,
    rel: &str,
    text: &str,
    module: &Option<Vec<String>>,
    out: &mut Vec<FoundEnum>,
) {
    for item in items {
        match item {
            syn::Item::Enum(e) if e.ident == name => out.push(FoundEnum {
                at: at_ident(&e.ident, rel, text, module),
                variants: e.variants.iter().map(|v| v.ident.to_string()).collect(),
            }),
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_enums(inner, name, rel, text, &inside(module, &m.ident), out);
                }
            }
            _ => {}
        }
    }
}

/// Inherent methods declared in `impl <ty>` blocks — all of them when `name` is `None`, or just
/// the ones so named. Trait impls (`impl Trait for Ty`) are skipped: a trait method is reached
/// through the trait, and calling it on a value the harness built would depend on the trait being
/// in scope, which `syn` cannot see (REQ055).
fn find_methods(subject: &ParsedSubject, ty: &str, name: Option<&str>) -> Vec<FoundFn> {
    let mut out = Vec::new();
    subject.each(|file, rel, text, module| {
        collect_methods(&file.items, ty, name, rel, text, module, &mut out);
    });
    out
}

fn collect_methods(
    items: &[syn::Item],
    ty: &str,
    name: Option<&str>,
    rel: &str,
    text: &str,
    module: &Option<Vec<String>>,
    out: &mut Vec<FoundFn>,
) {
    for item in items {
        match item {
            syn::Item::Impl(i) if i.trait_.is_none() && self_type_is(&i.self_ty, ty) => {
                for sub in &i.items {
                    if let syn::ImplItem::Fn(f) = sub {
                        if name.is_none_or(|n| f.sig.ident == n) {
                            out.push(found(&f.sig, rel, text, Some(ty), module));
                        }
                    }
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_methods(inner, ty, name, rel, text, &inside(module, &m.ident), out);
                }
            }
            _ => {}
        }
    }
}

/// Whether an `impl` block's self type is the named type, read off the last path segment so
/// `impl crate::engine::EngineStatus` still matches `EngineStatus`.
fn self_type_is(self_ty: &syn::Type, ty: &str) -> bool {
    type_ident(self_ty).is_some_and(|n| n == ty)
}

/// Build the record for one matched signature: where it is, how each parameter takes its
/// argument, what each parameter's type is written as, and how its return type is written.
/// `impl_ty` is the type an enclosing `impl` block is for, which is what a `self` receiver's
/// type is.
fn found(
    sig: &syn::Signature,
    rel: &str,
    text: &str,
    impl_ty: Option<&str>,
    module: &Option<Vec<String>>,
) -> FoundFn {
    let generics: Vec<String> = sig
        .generics
        .type_params()
        .map(|p| p.ident.to_string())
        .collect();
    FoundFn {
        name: sig.ident.to_string(),
        at: at_ident(&sig.ident, rel, text, module),
        params: sig.inputs.iter().map(param_mode).collect(),
        param_types: sig
            .inputs
            .iter()
            .map(|arg| param_type_ident(arg, &generics, impl_ty))
            .collect(),
        returns: return_type(sig),
        has_receiver: matches!(sig.inputs.first(), Some(syn::FnArg::Receiver(_))),
    }
}

/// The comparable name of one parameter's written type: the last segment of a plain path, so
/// `&mut User` and `crate::auth::User` both read as `User` (the same last-segment convention
/// [`self_type_is`] uses), and the enclosing type for a `self` receiver.
///
/// `None` wherever a written-name comparison would say nothing true: a **generic parameter**
/// (`T` names whatever the caller instantiates — resolving it is type inference, which `syn`
/// does not do), a tuple, a slice, an `impl Trait`. Generic *arguments* are ignored rather than
/// rejected, so `Wrapper<u32>` still reads as `Wrapper`: the sort resolver matches a bare ident,
/// so the expected side never carries any. Path-qualification on the sort's own side stays
/// deferred with the rest of #118's tail.
fn param_type_ident(
    arg: &syn::FnArg,
    generics: &[String],
    impl_ty: Option<&str>,
) -> Option<String> {
    match arg {
        syn::FnArg::Receiver(_) => impl_ty.map(str::to_string),
        syn::FnArg::Typed(t) => {
            let name = type_ident(&t.ty)?;
            (!generics.contains(&name)).then_some(name)
        }
    }
}

/// The last path segment of a type, looking through references. `None` for a shape that has no
/// single name to compare — a tuple, a slice, an `impl Trait`, a qualified `<T as Trait>::Out`.
fn type_ident(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Reference(r) => type_ident(&r.elem),
        syn::Type::Path(p) if p.qself.is_none() => {
            p.path.segments.last().map(|s| s.ident.to_string())
        }
        _ => None,
    }
}

/// How one parameter takes its argument, judged on how the type is *written* — the same
/// syntactic limit [`return_type`] works under. A `self` receiver reads as by-reference;
/// see [`ParamMode`] for why that is safe to get approximately right.
fn param_mode(arg: &syn::FnArg) -> ParamMode {
    match arg {
        syn::FnArg::Receiver(r) => {
            if r.reference.is_some() {
                ParamMode::ByRef
            } else {
                ParamMode::ByValue
            }
        }
        syn::FnArg::Typed(t) => match &*t.ty {
            syn::Type::Reference(_) => ParamMode::ByRef,
            _ => ParamMode::ByValue,
        },
    }
}

/// How a signature's return type is *written* — the syntactic check this adapter can
/// honestly make. A bare `-> bool` reads as `bool`; anything else keeps its own text so
/// the operator sees exactly what the subject says.
fn return_type(sig: &syn::Signature) -> String {
    match &sig.output {
        syn::ReturnType::Default => "()".to_string(),
        syn::ReturnType::Type(_, ty) => match &**ty {
            syn::Type::Path(p) => p
                .path
                .segments
                .last()
                .map(|s| {
                    if s.arguments.is_empty() {
                        s.ident.to_string()
                    } else {
                        // Keep generics visible — `Result<bool>` must never read as `bool`.
                        format!("{}<…>", s.ident)
                    }
                })
                .unwrap_or_else(|| "?".to_string()),
            _ => "?".to_string(),
        },
    }
}

/// The subject's own source line, so the operator confirms against real code rather than
/// a signature this tool reconstructed.
fn source_line(text: &str, line: usize) -> String {
    text.lines()
        .nth(line.saturating_sub(1))
        .unwrap_or("")
        .trim()
        .to_string()
}

/// The full source of the function whose signature ident sits on `line` (1-based), sliced from
/// `text`: the signature line through the closing brace of its body. `None` when the file does not
/// parse or no such function is found. Descends into inline `mod` and `impl` blocks, matching
/// [`find_functions`]'s reach, so a predicate declared inside one is still extractable for the
/// semantic contract-drafting prompt (REQ040). `line` is the same value [`CodeMatch::line`] carries,
/// so a resolved predicate's `at.line` feeds straight in.
pub fn fn_source_at(text: &str, line: usize) -> Option<String> {
    let file = syn::parse_file(text).ok()?;
    let (start, end) = find_fn_span(&file.items, line)?;
    let lines: Vec<&str> = text.lines().collect();
    if start == 0 || start > end || end > lines.len() {
        return None;
    }
    Some(lines[start - 1..end].join("\n"))
}

/// Locate the `(start_line, end_line)` span (both 1-based) of the function whose ident is on
/// `line`, descending into inline modules and impl blocks. The end is the line of the body's
/// closing brace, available because proc-macro2's `span-locations` feature is on (the same feature
/// [`found`] relies on for the start line).
fn find_fn_span(items: &[syn::Item], line: usize) -> Option<(usize, usize)> {
    let end_line = |sig: &syn::Signature, block: &syn::Block| {
        (sig.ident.span().start().line == line)
            .then(|| (line, block.brace_token.span.close().end().line))
    };
    for item in items {
        match item {
            syn::Item::Fn(f) => {
                if let Some(span) = end_line(&f.sig, &f.block) {
                    return Some(span);
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    if let Some(span) = find_fn_span(inner, line) {
                        return Some(span);
                    }
                }
            }
            syn::Item::Impl(i) => {
                for sub in &i.items {
                    if let syn::ImplItem::Fn(f) = sub {
                        if let Some(span) = end_line(&f.sig, &f.block) {
                            return Some(span);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A subject tree with `src/auth.rs` holding `src`, plus a companion dir the walk skips.
    fn subject(src: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/auth.rs"), src).unwrap();
        tmp
    }

    /// Resolve with nothing known about the parameters' sorts — the pre-REQ057 behaviour, which
    /// every check but the parameter-type one still works under.
    fn resolve_in(tmp: &tempfile::TempDir, observable: &str, arity: usize) -> Resolution {
        resolve_typed(tmp, observable, &vec![None; arity])
    }

    fn resolve_typed(
        tmp: &tempfile::TempDir,
        observable: &str,
        params: &[Option<String>],
    ) -> Resolution {
        resolve(&parsed(tmp), observable, params)
    }

    /// The subject, walked and parsed — what every resolver now takes.
    fn parsed(tmp: &tempfile::TempDir) -> ParsedSubject {
        ParsedSubject::load(tmp.path(), &tmp.path().join("ProvableRequirements"))
    }

    fn want(ty: &str) -> Option<String> {
        Some(ty.to_string())
    }

    // Verifies: REQ025 — a predicate resolves to a real function at a real location, which
    // is what a substring match could never establish.
    #[test]
    fn resolves_a_bool_function_to_its_source_location() {
        let tmp = subject("pub fn login(user: &str) -> bool { !user.is_empty() }\n");
        let r = resolve_in(&tmp, "login", 1);
        let Resolution::Resolved { at, params, .. } = r else {
            panic!("should resolve, got {r:?}")
        };
        assert_eq!(at.file, "src/auth.rs");
        assert_eq!(at.line, 1);
        assert!(at.text.contains("fn login"));
        assert_eq!(params, vec![ParamMode::ByRef]);
    }

    // Verifies (#144): ONE walk-and-parse answers every kind of lookup a binding set makes — a
    // free function, a sort, and a qualified `A::B` (which alone consults functions, types and
    // enums). The tree is shared, not consumed, so a grounding pass pays for it once; before this,
    // each of these started its own walk and re-parsed the whole subject.
    #[test]
    fn one_parse_of_the_subject_answers_every_lookup() {
        let tmp = subject(ENUM_SUBJECT);
        let subject = parsed(&tmp);
        assert!(
            resolve(&subject, "decide::Proceed", &[None]).is_resolved(),
            "qualified variant test"
        );
        assert!(
            resolve(&subject, "Engine::is_ready", &[None]).is_resolved(),
            "qualified method"
        );
        assert!(resolve_type(&subject, "Decision").is_resolved(), "sort");
    }

    /// The shape the dogfood run hit (#129): a decision that lives in an enum, on a type that
    /// also carries behaviour. Nothing here is written `-> bool` except the methods.
    const ENUM_SUBJECT: &str = "\
pub enum Decision { AlreadyPresent, UnsupportedPlatform, Proceed }
pub fn decide(consent: bool) -> Decision { Decision::Proceed }
pub struct Engine;
impl Engine {
    pub fn is_ready(&self) -> bool { true }
    pub fn name(&self) -> String { String::new() }
}
pub struct Probe;
impl Probe { pub fn is_ready(&self) -> bool { false } }
";

    // Verifies: REQ055 — a function that returns an enum binds through one of its variants. The
    // whole point: `decide` carries a real invariant, and before this the only way to reach it was
    // to add a `-> bool` wrapper to the subject purely to satisfy the binder.
    #[test]
    fn a_function_returning_an_enum_binds_through_one_of_its_variants() {
        let tmp = subject(ENUM_SUBJECT);

        // Unqualified, it is honestly not a boolean — and the message points at the way out.
        let bare = resolve_in(&tmp, "decide", 1);
        assert!(matches!(bare, Resolution::NotBoolean { .. }), "{bare:?}");
        let msg = bare.describe("proceeds", "decide");
        assert!(msg.contains("decide::<Variant>"), "{msg}");

        let r = resolve_in(&tmp, "decide::Proceed", 1);
        let Resolution::Resolved { form, params, .. } = &r else {
            panic!("should resolve, got {r:?}")
        };
        assert_eq!(
            form,
            &PredicateForm::VariantTest {
                name: "decide".into(),
                enum_name: "Decision".into(),
                variant: "Proceed".into(),
                // The fixture lives in `src/auth.rs`, so that is the module a harness must
                // name the enum through (REQ061).
                enum_module: Some(vec!["auth".into()]),
            }
        );
        assert_eq!(params, &vec![ParamMode::ByValue]);
    }

    // Verifies: REQ055 — a variant that does not exist parks, and the park names the variants the
    // enum actually has. A misspelling is the likeliest cause, and the useful answer to one is the
    // list it was meant to be spelled from.
    #[test]
    fn a_variant_that_does_not_exist_names_the_ones_that_do() {
        let tmp = subject(ENUM_SUBJECT);
        let r = resolve_in(&tmp, "decide::Procede", 1);
        assert!(!r.is_resolved());
        let msg = r.describe("proceeds", "decide::Procede");
        assert!(msg.contains("AlreadyPresent"), "{msg}");
        assert!(msg.contains("Proceed"), "{msg}");
        assert!(msg.contains("no variant `Procede`"), "{msg}");
    }

    // Verifies: REQ055 — a function whose return type is not an enum the subject declares parks
    // without pretending there was a variant list to offer.
    #[test]
    fn a_variant_on_a_non_enum_return_says_so() {
        let tmp = subject("pub fn login(u: &str) -> bool { true }\n");
        let r = resolve_in(&tmp, "login::Yes", 1);
        let msg = r.describe("ok", "login::Yes");
        assert!(!r.is_resolved());
        assert!(msg.contains("no enum `bool`"), "{msg}");
    }

    // Verifies: REQ055 — a method name two types share is an ambiguity unqualified, and
    // qualifying by type is how the operator resolves it. This is what `Type::method` is *for*;
    // `is_ready` alone cannot say which type's state the requirement is about.
    #[test]
    fn qualifying_by_type_disambiguates_a_shared_method_name() {
        let tmp = subject(ENUM_SUBJECT);
        assert!(matches!(
            resolve_in(&tmp, "is_ready", 1),
            Resolution::Ambiguous(_)
        ));

        let r = resolve_in(&tmp, "Engine::is_ready", 1);
        let Resolution::Resolved { form, at, .. } = &r else {
            panic!("should resolve, got {r:?}")
        };
        assert_eq!(
            form,
            &PredicateForm::Method {
                name: "is_ready".into()
            }
        );
        assert!(at.text.contains("is_ready"), "{:?}", at.text);
    }

    // Verifies: REQ055 — the form follows the *signature*, not the binding syntax. A method found
    // by its bare name is still a method, because lowering it as a free call generates a harness
    // that cannot compile — which reaches the operator as an `unknown` rather than as the binding
    // mistake it is.
    #[test]
    fn a_method_found_by_its_bare_name_is_still_a_method() {
        let tmp = subject("pub struct S;\nimpl S { pub fn ready(&self) -> bool { true } }\n");
        let r = resolve_in(&tmp, "ready", 1);
        let Resolution::Resolved { form, .. } = &r else {
            panic!("should resolve, got {r:?}")
        };
        assert_eq!(
            form,
            &PredicateForm::Method {
                name: "ready".into()
            }
        );
    }

    // Verifies: REQ055 — a method the type does not have parks, naming the ones it does.
    #[test]
    fn a_method_the_type_does_not_have_names_the_ones_it_does() {
        let tmp = subject(ENUM_SUBJECT);
        let r = resolve_in(&tmp, "Engine::is_redy", 1);
        assert!(!r.is_resolved());
        let msg = r.describe("ready", "Engine::is_redy");
        assert!(msg.contains("is_ready"), "{msg}");
        assert!(msg.contains("name"), "{msg}");
    }

    // Verifies: REQ055 — a trait impl is not an inherent method. Calling one depends on the trait
    // being in scope at the harness, which `syn` cannot see, so binding to it would generate code
    // whose correctness this adapter never established.
    #[test]
    fn a_trait_method_is_not_an_inherent_method() {
        let tmp = subject(
            "pub struct S;\npub trait Ready { fn ready(&self) -> bool; }\n\
             impl Ready for S { fn ready(&self) -> bool { true } }\n",
        );
        let r = resolve_in(&tmp, "S::ready", 1);
        assert!(matches!(r, Resolution::NoSuchMethod { .. }), "{r:?}");
    }

    // Verifies: REQ055 — a path deeper than `A::B` names nothing this adapter understands, and
    // guessing which two segments were meant would bind the requirement to something the operator
    // did not write.
    #[test]
    fn a_path_deeper_than_two_segments_does_not_resolve() {
        let tmp = subject(ENUM_SUBJECT);
        let r = resolve_in(&tmp, "provreq::decide::Proceed", 1);
        assert_eq!(r, Resolution::NotFound);
        let msg = r.describe("proceeds", "provreq::decide::Proceed");
        assert!(msg.contains("decide_install::Proceed"), "{msg}");
    }

    // Verifies: REQ027 — a resolved predicate reports how each parameter takes its
    // argument, which is what lets the engine generate `login(&u)` rather than `login(u)`.
    // Judged on how the type is WRITTEN, the same syntactic limit the rest of this adapter
    // works under.
    #[test]
    fn resolved_predicate_reports_how_its_parameters_take_arguments() {
        let tmp = subject(
            "pub fn by_value(u: User) -> bool { true }
pub fn by_ref(u: &User) -> bool { true }
pub fn mixed(a: &User, b: u32) -> bool { true }
pub fn nullary() -> bool { true }\n",
        );
        let modes = |name: &str, arity: usize| match resolve_in(&tmp, name, arity) {
            Resolution::Resolved { params, .. } => params,
            other => panic!("{name} should resolve, got {other:?}"),
        };
        assert_eq!(modes("by_value", 1), vec![ParamMode::ByValue]);
        assert_eq!(modes("by_ref", 1), vec![ParamMode::ByRef]);
        assert_eq!(
            modes("mixed", 2),
            vec![ParamMode::ByRef, ParamMode::ByValue]
        );
        assert!(modes("nullary", 0).is_empty());
    }

    // Verifies: REQ057 — a predicate whose parameter is written as a different type than the sort
    // its argument ranges over does NOT ground. This is the whole point of the slice: before it,
    // `each u: User . logged_in(u)` bound to a function over `Session` was green here and failed
    // later as a compiler error inside an `unknown`.
    #[test]
    fn a_parameter_typed_against_a_different_sort_does_not_resolve() {
        let tmp = subject(
            "pub struct User;\npub struct Session;\n\
             pub fn login(s: &Session) -> bool { true }\n",
        );
        let r = resolve_typed(&tmp, "login", &[want("User")]);
        assert!(
            matches!(&r, Resolution::WrongParamType { param: 1, expected, found, .. }
                     if expected == "User" && found == "Session"),
            "got {r:?}"
        );
        assert!(!r.is_resolved());
        let msg = r.describe("logged_in", "login");
        assert!(msg.contains("`Session`"), "names what the code says: {msg}");
        assert!(
            msg.contains("`User`"),
            "names what the sort is bound to: {msg}"
        );
        // The same binding against the matching type is untouched.
        assert!(resolve_typed(&tmp, "login", &[want("Session")]).is_resolved());
    }

    // Verifies: REQ057 — the check compares written names, and skips every position where that
    // comparison would say nothing true. A generic parameter, a tuple, and a position the caller
    // knows nothing about must all resolve rather than park: a false park costs the operator a
    // real binding, which is worse than the compiler error this slice removes.
    #[test]
    fn positions_a_name_comparison_cannot_speak_for_are_skipped() {
        let tmp = subject(
            "pub fn generic<T>(t: &T) -> bool { true }
pub fn tupled(p: (u32, u32)) -> bool { true }
pub fn qualified(u: &crate::auth::User) -> bool { true }
pub fn wrapped(w: &Wrapper<u32>) -> bool { true }
pub fn unknown_side(s: &Session) -> bool { true }\n",
        );
        assert!(resolve_typed(&tmp, "generic", &[want("User")]).is_resolved());
        assert!(resolve_typed(&tmp, "tupled", &[want("User")]).is_resolved());
        // A path-qualified type is read by its last segment, as impl blocks already are.
        assert!(resolve_typed(&tmp, "qualified", &[want("User")]).is_resolved());
        // Generic arguments are ignored: the sort resolver matches a bare ident, so the expected
        // side never carries any and `Wrapper<u32>` must not read as a mismatch with `Wrapper`.
        assert!(resolve_typed(&tmp, "wrapped", &[want("Wrapper")]).is_resolved());
        assert!(resolve_typed(&tmp, "unknown_side", &[None]).is_resolved());
    }

    // Verifies: REQ057 — a `self` receiver's type is the type its `impl` block is for, so a method
    // bound to the wrong sort is caught exactly as a free function is. Reached both ways a method
    // can be named (REQ055), because the check must not depend on how the operator wrote it.
    #[test]
    fn a_receivers_type_is_the_type_it_is_implemented_on() {
        let tmp = subject(
            "pub struct User;\npub struct Session;\n\
             impl Session { pub fn is_active(&self) -> bool { true } }\n",
        );
        for observable in ["Session::is_active", "is_active"] {
            let r = resolve_typed(&tmp, observable, &[want("User")]);
            assert!(
                matches!(&r, Resolution::WrongParamType { found, .. } if found == "Session"),
                "{observable} should name its impl type, got {r:?}"
            );
            assert!(resolve_typed(&tmp, observable, &[want("Session")]).is_resolved());
        }
    }

    // Verifies: REQ057 — the coarsest mismatch is reported first, so the operator is never sent
    // to fix a parameter type on a function that is not a predicate at all.
    #[test]
    fn arity_and_return_type_are_reported_before_a_parameter_type() {
        let tmp = subject(
            "pub struct User;\n\
             pub fn count(s: &Session) -> u32 { 0 }\n\
             pub fn pair(a: &Session, b: &Session) -> bool { true }\n",
        );
        assert!(matches!(
            resolve_typed(&tmp, "pair", &[want("User")]),
            Resolution::WrongArity { .. }
        ));
        assert!(matches!(
            resolve_typed(&tmp, "count", &[want("User")]),
            Resolution::NotBoolean { .. }
        ));
    }

    // Verifies: REQ057 — a variant-test binding (REQ055) is checked too. The parameter types of the
    // function whose enum is tested are as real as any other predicate's.
    #[test]
    fn a_variant_test_checks_its_parameter_types_too() {
        let tmp = subject(
            "pub enum Decision { Proceed }\n\
             pub fn decide(s: &Session) -> Decision { Decision::Proceed }\n",
        );
        let r = resolve_typed(&tmp, "decide::Proceed", &[want("User")]);
        assert!(
            matches!(&r, Resolution::WrongParamType { found, .. } if found == "Session"),
            "got {r:?}"
        );
        assert!(resolve_typed(&tmp, "decide::Proceed", &[want("Session")]).is_resolved());
    }

    // Verifies: REQ040 — the fn source for a resolved predicate is sliced from the signature
    // line through its closing brace, including a multi-line body and a fn nested in an impl.
    #[test]
    fn fn_source_at_extracts_the_whole_function() {
        let src = "\
pub fn a() -> bool { true }

impl Session {
    pub fn logged_in(&self) -> bool {
        !self.token.is_empty()
    }
}
";
        // A one-line fn.
        assert_eq!(fn_source_at(src, 1).unwrap(), "pub fn a() -> bool { true }");
        // A multi-line fn inside an impl, sig line (4) through its closing brace (6).
        let got = fn_source_at(src, 4).unwrap();
        assert!(got.starts_with("    pub fn logged_in(&self) -> bool {"));
        assert!(got.trim_end().ends_with("}"));
        assert!(got.contains("!self.token.is_empty()"));
        // A line with no fn ident returns nothing.
        assert!(fn_source_at(src, 2).is_none());
        // An unparseable file is honestly None, never a panic.
        assert!(fn_source_at("fn broken( {", 1).is_none());
    }

    // Verifies: REQ025 — a binding to a name that is not in the subject parks the
    // requirement (R-ground-1), rather than grounding on a coincidental text match.
    #[test]
    fn missing_function_does_not_resolve() {
        let tmp = subject("pub fn login(user: &str) -> bool { true }\n");
        assert_eq!(resolve_in(&tmp, "log_in", 1), Resolution::NotFound);
        assert!(!Resolution::NotFound.is_resolved());
    }

    // Verifies: REQ025 — the arity the requirement declares must match the function's, or
    // the binding is wrong even though the name exists.
    #[test]
    fn arity_mismatch_does_not_resolve() {
        let tmp = subject("pub fn login(user: &str) -> bool { true }\n");
        let r = resolve_in(&tmp, "login", 2);
        assert!(matches!(
            r,
            Resolution::WrongArity {
                expected: 2,
                found: 1,
                ..
            }
        ));
        assert!(!r.is_resolved());
    }

    // Verifies: REQ025 — a state predicate must be a boolean; a function that is not
    // written to return bool cannot stand for one.
    #[test]
    fn non_boolean_function_does_not_resolve() {
        let tmp = subject("pub fn login(user: &str) -> String { user.into() }\n");
        let r = resolve_in(&tmp, "login", 1);
        assert!(
            matches!(&r, Resolution::NotBoolean { returns, .. } if returns == "String"),
            "got {r:?}"
        );
    }

    // Verifies: REQ025 — the syntactic limit is real and must not silently pass as `bool`:
    // `Result<bool>` keeps its generics visible so it is rejected, not mistaken for bool.
    #[test]
    fn result_bool_is_not_mistaken_for_bool() {
        let tmp = subject("pub fn login(u: &str) -> Result<bool> { Ok(true) }\n");
        let r = resolve_in(&tmp, "login", 1);
        assert!(
            matches!(&r, Resolution::NotBoolean { returns, .. } if returns.starts_with("Result")),
            "Result<bool> must not read as bool, got {r:?}"
        );
    }

    // Verifies: REQ025 — two functions sharing a name are never silently disambiguated;
    // binding to one would depend on walk order, which is not a decision this tool may make.
    #[test]
    fn duplicate_names_are_ambiguous_never_guessed() {
        let tmp = subject(
            "pub fn login(u: &str) -> bool { true }
mod admin { pub fn login(u: &str) -> bool { false } }\n",
        );
        let r = resolve_in(&tmp, "login", 1);
        let Resolution::Ambiguous(ats) = &r else {
            panic!("should be ambiguous, got {r:?}")
        };
        assert_eq!(ats.len(), 2);
        assert!(!r.is_resolved());
    }

    // Verifies: REQ025 — a predicate declared inside an inline module or an impl block is
    // still found; "comprehensive" means the whole tree, not just top-level items.
    #[test]
    fn finds_functions_in_modules_and_impls() {
        let tmp = subject(
            "mod session { pub fn active(id: u32) -> bool { true } }
struct S;
impl S { fn ready(&self) -> bool { true } }\n",
        );
        assert!(resolve_in(&tmp, "active", 1).is_resolved());
        assert!(resolve_in(&tmp, "ready", 1).is_resolved());
    }

    // Verifies: REQ025 — an unparseable Rust file does not blind the resolution of a
    // predicate that lives in a file which parses fine.
    #[test]
    fn unparseable_file_does_not_blind_resolution() {
        let tmp = subject("pub fn login(u: &str) -> bool { true }\n");
        std::fs::write(
            tmp.path().join("src/broken.rs"),
            "fn ( this is not rust @@@",
        )
        .unwrap();
        assert!(resolve_in(&tmp, "login", 1).is_resolved());
    }

    // Verifies: REQ025 (was REQ021's dry-run test) — the walk skips the companion tree and
    // `.git`. The companion's drafts.yml names the observables themselves, so resolving
    // there would be a spurious self-hit — and a self-hit would look like an ambiguity.
    #[test]
    fn skips_the_companion_tree_and_git() {
        let tmp = subject("pub fn login(u: &str) -> bool { true }\n");
        let companion = tmp.path().join("ProvableRequirements");
        std::fs::create_dir_all(&companion).unwrap();
        std::fs::write(
            companion.join("shadow.rs"),
            "pub fn login(u: &str) -> bool { false }\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        std::fs::write(
            tmp.path().join(".git/x.rs"),
            "pub fn login(u: &str) -> bool { false }\n",
        )
        .unwrap();

        let r = resolve(
            &ParsedSubject::load(tmp.path(), &companion),
            "login",
            &[None],
        );
        let Resolution::Resolved { at, .. } = &r else {
            panic!("the companion/.git copies must not create an ambiguity, got {r:?}")
        };
        assert_eq!(at.file, "src/auth.rs");
    }

    // Verifies: REQ025 — an empty observable resolves to nothing (guards a degenerate
    // binding), rather than matching the first function it meets.
    #[test]
    fn empty_observable_resolves_to_nothing() {
        let tmp = subject("pub fn login(u: &str) -> bool { true }\n");
        assert_eq!(resolve_in(&tmp, "   ", 1), Resolution::NotFound);
    }

    // Verifies: REQ025 — a non-Rust file is not parsed for predicates; a cat-1 state
    // predicate is a Rust item, not any text that happens to look like one.
    #[test]
    fn non_rust_files_are_not_searched() {
        let tmp = subject("pub fn login(u: &str) -> bool { true }\n");
        std::fs::write(
            tmp.path().join("README.md"),
            "pub fn login(u: &str) -> bool { false }\n",
        )
        .unwrap();
        assert!(resolve_in(&tmp, "login", 1).is_resolved());
    }

    // Verifies: REQ061 — an item's module path comes from where it was found, so a harness can name
    // it. The standard cargo layout, read off the file path; `src/lib.rs` is the crate root, a
    // `mod.rs` contributes its directory only, and an inline `mod` block adds its own segment.
    #[test]
    fn module_path_follows_the_cargo_layout() {
        for (file, expected) in [
            ("src/lib.rs", Some(vec![])),
            ("src/provision.rs", Some(vec!["provision".to_string()])),
            ("src/a/b.rs", Some(vec!["a".to_string(), "b".to_string()])),
            ("src/a/mod.rs", Some(vec!["a".to_string()])),
        ] {
            assert_eq!(file_module_path(file), expected, "{file}");
        }
        // No path a harness can write reaches these: separate crate targets and binaries.
        for file in [
            "tests/it.rs",
            "benches/b.rs",
            "examples/e.rs",
            "src/main.rs",
            "src/bin/tool.rs",
            "build.rs",
        ] {
            assert_eq!(file_module_path(file), None, "{file}");
        }
    }

    // Verifies: REQ061 — the module a predicate resolves in is recorded, for a top-level item and
    // for one nested in an inline `mod`. Before this, lowering wrote `crate_name::item` for
    // everything, which is right only for a crate whose every item sits in `src/lib.rs`.
    #[test]
    fn resolution_records_the_module_it_found_the_item_in() {
        let tmp = subject(
            "pub fn login(u: &str) -> bool { true }
mod session { pub fn active(id: u32) -> bool { true } }\n",
        );
        let module_of = |name: &str, arity: usize| match resolve_in(&tmp, name, arity) {
            Resolution::Resolved { at, .. } => at.module,
            other => panic!("{name} should resolve, got {other:?}"),
        };
        // `src/auth.rs` → the `auth` module…
        assert_eq!(module_of("login", 1), Some(vec!["auth".to_string()]));
        // …and an inline `mod` inside it is one segment deeper.
        assert_eq!(
            module_of("active", 1),
            Some(vec!["auth".to_string(), "session".to_string()])
        );
    }

    // Verifies: REQ061 — an item in a separate crate target still RESOLVES. Existence and
    // reachability are different questions: the predicate really is declared there, grounding is
    // right to say so, and it is lowering's job to refuse to name it.
    #[test]
    fn an_item_outside_the_crate_resolves_but_has_no_module() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("tests")).unwrap();
        std::fs::write(
            tmp.path().join("tests/helpers.rs"),
            "pub fn ready(u: &str) -> bool { true }\n",
        )
        .unwrap();
        let r = resolve(&parsed(&tmp), "ready", &[None]);
        let Resolution::Resolved { at, .. } = &r else {
            panic!("it is really declared there, so it resolves: {r:?}")
        };
        assert_eq!(at.module, None, "but no harness can name it");
    }

    // Verifies: REQ061 — two enums of one name are an ambiguity, not a pooled variant list. The
    // harness can only name one module, and picking would depend on walk order — the same rule
    // every other duplicate follows.
    #[test]
    fn duplicate_enums_are_ambiguous_never_pooled() {
        let tmp = subject(
            "pub enum Decision { Proceed }
mod other { pub enum Decision { Proceed } }
pub fn decide(u: u32) -> Decision { Decision::Proceed }\n",
        );
        let r = resolve_in(&tmp, "decide::Proceed", 1);
        assert!(matches!(r, Resolution::Ambiguous(_)), "got {r:?}");
    }

    // Verifies: REQ026 — a sort resolves to a real Rust type, so a quantified variable has
    // a domain. struct / enum / type alias all qualify.
    #[test]
    fn resolves_a_sort_to_a_struct_enum_or_alias() {
        for (src, name) in [
            ("pub struct User { id: u32 }\n", "User"),
            ("pub enum User { Admin, Guest }\n", "User"),
            ("pub type User = u32;\n", "User"),
        ] {
            let tmp = subject(src);
            let r = resolve_type(&parsed(&tmp), name);
            let TypeResolution::Resolved(at) = &r else {
                panic!("{name} should resolve from {src:?}, got {r:?}")
            };
            assert_eq!(at.file, "src/auth.rs");
            assert!(r.is_resolved());
        }
    }

    // Verifies: REQ058 — a Rust primitive is a real domain, so it grounds as a sort even though
    // the subject declares nothing of that name. Without this a predicate over a `bool` parameter
    // could never be quantified, whatever the operator wrote.
    #[test]
    fn a_primitive_type_resolves_as_a_sort() {
        let tmp = subject("pub struct User;\n");
        let companion = tmp.path().join("ProvableRequirements");
        for name in ["bool", "u32", "usize", "char", "f64", "i8"] {
            let r = resolve_type(&ParsedSubject::load(tmp.path(), &companion), name);
            assert_eq!(r, TypeResolution::Primitive(name.to_string()), "{name}");
            assert!(r.is_resolved(), "{name} must ground");
        }
        // The read-back says what it resolved to and why there is no location to confirm.
        let msg = resolve_type(&ParsedSubject::load(tmp.path(), &companion), "bool")
            .describe("Flag", "bool");
        assert!(msg.contains("primitive `bool`"), "{msg}");
        assert!(msg.contains("no source location"), "{msg}");
    }

    // Verifies: REQ058 — `str` is unsized and `String` is a declared std type, so neither is
    // admitted: a quantifier over `str` would lower to a harness that cannot compile, which is
    // the failure this slice exists to move earlier rather than one to introduce.
    #[test]
    fn str_and_string_are_not_primitive_sorts() {
        let tmp = subject("pub struct User;\n");
        let companion = tmp.path().join("ProvableRequirements");
        for name in ["str", "String", "Vec", "bools"] {
            assert_eq!(
                resolve_type(&ParsedSubject::load(tmp.path(), &companion), name),
                TypeResolution::NotFound,
                "{name} must not resolve as a primitive"
            );
        }
    }

    // Verifies: REQ058 — a subject that declares its own type named after a primitive keeps it.
    // The declaration has a source location the operator can confirm against and the read-back
    // names it, which is the whole point of grounding; the language is only the fallback.
    #[test]
    fn a_declared_type_wins_over_the_primitive_of_the_same_name() {
        let tmp = subject("pub struct bool { pub set: u8 }\n");
        let r = resolve_type(&parsed(&tmp), "bool");
        let TypeResolution::Resolved(at) = &r else {
            panic!("the subject's own declaration must win, got {r:?}")
        };
        assert_eq!(at.file, "src/auth.rs");
        assert!(r.describe("Flag", "bool").contains("src/auth.rs:1"));
    }

    // Verifies: REQ026 — a sort naming no real type does not resolve, so the requirement
    // parks: nothing can range over a domain that does not exist (R-ground-1).
    #[test]
    fn unknown_sort_does_not_resolve() {
        let tmp = subject("pub struct User;\n");
        let r = resolve_type(&parsed(&tmp), "Session");
        assert_eq!(r, TypeResolution::NotFound);
        assert!(!r.is_resolved());
        assert!(r
            .describe("Session", "Session")
            .contains("cannot range over"));
    }

    // Verifies: REQ026 — two types sharing a name are never silently disambiguated, for the
    // same reason as predicates: the choice would depend on walk order.
    #[test]
    fn duplicate_sorts_are_ambiguous_never_guessed() {
        let tmp = subject("pub struct User;\nmod admin { pub struct User; }\n");
        let r = resolve_type(&parsed(&tmp), "User");
        let TypeResolution::Ambiguous(ats) = &r else {
            panic!("should be ambiguous, got {r:?}")
        };
        assert_eq!(ats.len(), 2);
        assert!(!r.is_resolved());
        // The park now offers a way out, and offers it in the form the operator must type.
        assert!(
            r.describe("U", "User").contains("`auth::admin::User`"),
            "advice to qualify is worth nothing without the real path: {}",
            r.describe("U", "User")
        );
    }

    // Verifies: #138 — a module-qualified sort picks one of two types sharing a name. This is the
    // way out of the `Ambiguous` park above, whose own read-back has always said "qualify it"
    // while the sort side matched a bare ident only — advice nothing could act on.
    #[test]
    fn a_module_qualified_sort_picks_which_type_is_meant() {
        let tmp = subject("mod alpha { pub struct User; }\nmod beta { pub struct User; }\n");
        let subject = parsed(&tmp);

        for written in [
            "alpha::User",
            "auth::alpha::User",
            "crate::auth::alpha::User",
        ] {
            let TypeResolution::Resolved(at) = resolve_type(&subject, written) else {
                panic!(
                    "`{written}` must resolve, got {:?}",
                    resolve_type(&subject, written)
                );
            };
            assert_eq!(
                at.module.as_deref(),
                Some(["auth".to_string(), "alpha".to_string()].as_slice()),
                "`{written}` must pick the `alpha` one"
            );
        }

        // The other one is still reachable, so the qualifier discriminates rather than just
        // accepting the first candidate.
        let TypeResolution::Resolved(at) = resolve_type(&subject, "beta::User") else {
            panic!("the sibling must resolve too")
        };
        assert_eq!(
            at.module.as_deref(),
            Some(["auth".to_string(), "beta".to_string()].as_slice())
        );

        // An unqualified sort is unchanged: nothing written to check, so both still match.
        assert!(matches!(
            resolve_type(&subject, "User"),
            TypeResolution::Ambiguous(_)
        ));
    }

    // Verifies: #138 — a module-qualified sort does not park the predicates that use it. Found by
    // running the real CLI, not by a unit test: `Sess=session::Session` resolved the sort and then
    // parked BOTH predicates, because REQ057 compared the observable `session::Session` against the
    // parameter's written `Session` as strings. Qualifying a sort is the one way out of an
    // ambiguity, so this made the fix for #138 break the thing it exists to enable.
    #[test]
    fn a_qualified_sort_still_matches_the_parameter_it_is_bound_to() {
        let tmp = subject("pub struct Session;\npub fn trusted(s: &Session) -> bool { true }\n");
        let r = resolve_typed(&tmp, "trusted", &[Some("session::Session".to_string())]);
        assert!(
            matches!(r, Resolution::Resolved { .. }),
            "a qualified sort must not park the predicate that ranges over it, got {r:?}"
        );

        // The check still fires on a genuine mismatch — the qualifier is dropped, not the name.
        let wrong = resolve_typed(&tmp, "trusted", &[Some("session::Account".to_string())]);
        assert!(
            matches!(wrong, Resolution::WrongParamType { .. }),
            "dropping the qualifier must not drop the check, got {wrong:?}"
        );
    }

    // Verifies: #138 — a wrong qualifier is reported as a wrong qualifier, not as a missing type.
    // `NotFound` would say "no type `User` in the subject's Rust", which is false and would send
    // the operator hunting for something sitting in their own source.
    #[test]
    fn a_qualifier_that_matches_nothing_is_not_a_missing_type() {
        let tmp = subject("mod alpha { pub struct User; }\n");
        let r = resolve_type(&parsed(&tmp), "crate::beta::User");
        let TypeResolution::QualifierUnmatched { name, candidates } = &r else {
            panic!("should name the qualifier as the fault, got {r:?}")
        };
        assert_eq!(name, "User");
        assert_eq!(candidates.len(), 1);
        assert!(
            !r.is_resolved(),
            "a mis-qualified sort still does not ground"
        );

        let text = r.describe("U", "crate::beta::User");
        assert!(text.contains("the qualifier that is wrong"), "{text}");
        assert!(
            text.contains("`auth::alpha::User`"),
            "it must offer the path the subject really declares: {text}"
        );

        // A name that is genuinely absent is still `NotFound` — the two must not collapse.
        assert_eq!(
            resolve_type(&parsed(&tmp), "crate::beta::Session"),
            TypeResolution::NotFound
        );
    }

    // Verifies: #138 — a candidate whose module the walk could not determine (REQ061) does not
    // answer a qualifier. `None` is a refusal, not "the crate root": matching it would ground a
    // binding against a module path provreq invented, which is the exact over-claim REQ061 exists
    // to prevent.
    #[test]
    fn an_unknown_module_never_satisfies_a_qualifier() {
        assert!(
            !module_matches(None, &["auth"]),
            "an unplaceable item must not answer a qualifier"
        );
        assert!(
            module_matches(None, &[]),
            "but it is unaffected when nothing was written to check"
        );
        assert!(module_matches(
            Some(&["auth".to_string()]),
            &["crate", "auth"]
        ));
        assert!(
            module_matches(Some(&["a".to_string(), "auth".to_string()]), &["auth"]),
            "a partial qualifier matches from the right"
        );
        assert!(
            !module_matches(Some(&["auth".to_string()]), &["mycrate", "auth"]),
            "a qualifier deeper than the declaration names a segment provreq cannot verify — \
             including a crate name, which is indistinguishable from a wrong module"
        );
        assert!(
            !module_matches(Some(&["auth".to_string()]), &["api"]),
            "an overlapping segment that differs is a mismatch"
        );
    }

    // Verifies: REQ026 — a function and a type sharing a name do not cross-resolve. A
    // predicate binds to a function and a sort binds to a type; one resolver must never
    // answer the other's question.
    #[test]
    fn predicates_and_sorts_do_not_cross_resolve() {
        let tmp = subject("pub struct login;\npub fn User() -> bool { true }\n");
        let companion = tmp.path().join("ProvableRequirements");
        // `login` is a struct here, not a fn → the predicate resolver must not find it.
        assert_eq!(
            resolve(&ParsedSubject::load(tmp.path(), &companion), "login", &[]),
            Resolution::NotFound
        );
        // `User` is a fn here, not a type → the sort resolver must not find it.
        assert_eq!(
            resolve_type(&ParsedSubject::load(tmp.path(), &companion), "User"),
            TypeResolution::NotFound
        );
    }

    // Verifies: REQ025/REQ026 — both resolvers share one walk, so they cannot disagree
    // about which files count: the companion tree is skipped for sorts exactly as for
    // predicates.
    #[test]
    fn sort_resolution_skips_the_companion_tree() {
        let tmp = subject("pub struct User;\n");
        let companion = tmp.path().join("ProvableRequirements");
        std::fs::create_dir_all(&companion).unwrap();
        std::fs::write(companion.join("shadow.rs"), "pub struct User;\n").unwrap();
        assert!(resolve_type(&ParsedSubject::load(tmp.path(), &companion), "User").is_resolved());
    }

    // Verifies: REQ025 — the resolved read-back names the limit of what was checked, so a
    // green line never implies a type-check `syn` cannot perform.
    #[test]
    fn resolved_readback_states_the_syntactic_limit() {
        let tmp = subject("pub fn login(user: &str) -> bool { true }\n");
        let text = resolve_in(&tmp, "login", 1).describe("logged_in", "login");
        assert!(text.contains("src/auth.rs:1"), "names the location: {text}");
        assert!(text.contains("syntactic"), "states the limit: {text}");
    }
}
