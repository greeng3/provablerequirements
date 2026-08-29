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
use walkdir::{DirEntry, WalkDir};

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

/// Whether an entry is skipped by the walk: the companion tree (whose `drafts.yml` holds the
/// observables themselves — resolving there would be a spurious self-hit), or anything
/// [`crate::subject_tree`] excludes from every walk of a subject.
///
/// A directory is asked of the directory rule and a file of the file rule, because they are
/// different arguments (#294). Asking the directory rule of everything, as this did, let its hidden
/// clause decide files it was never about.
fn is_skipped(entry: &DirEntry, companion_root: &Path) -> bool {
    if entry.path() == companion_root {
        return true;
    }
    if entry.file_type().is_dir() {
        crate::subject_tree::is_pruned_dir(entry.path(), entry.depth())
    } else {
        crate::subject_tree::is_pruned_file(entry.path())
    }
}

/// How one parameter of a resolved predicate takes its argument. The only thing an engine
/// needs in order to *call* the predicate, and — consistent with this module's
/// syntax-not-types limit — the only thing `syn` can honestly report: whether the parameter
/// is **written** as a reference.
///
/// This says *whether*, never *how deep* and never *how mutable*, and the lowering writes exactly
/// one `&` accordingly. A signature needing anything else is therefore refused at grounding rather
/// than called wrongly — see [`Resolution::UncallableParam`]. A `&self` receiver stays [`ByRef`] and
/// is correct: the call is written on the receiver, which supplies its own reference.
///
/// [`ByRef`]: ParamMode::ByRef
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    ByRef,
    ByValue,
}

/// How a parameter is written, when that is a form the lowering will not call (#201) — `&&Item`,
/// `&mut Item`, `&mut self`. `None` when an ordinary single `&` or a plain value, which is what a
/// call can be generated for.
///
/// A **`mut self`** receiver taken by value is *not* refused: the mutability is the callee's own
/// binding, invisible at the call site, and `u.f()` compiles. Only a reference the caller has to
/// write matters here.
fn uncallable_param(arg: &syn::FnArg) -> Option<String> {
    match arg {
        syn::FnArg::Receiver(r) => {
            (r.reference.is_some() && r.mutability.is_some()).then(|| "&mut self".to_string())
        }
        syn::FnArg::Typed(t) => {
            let (prefix, depth, has_mut) = reference_prefix(&t.ty);
            (depth > 1 || has_mut).then(|| {
                format!(
                    "{prefix}{}",
                    type_ident(&t.ty).unwrap_or_else(|| "…".into())
                )
            })
        }
    }
}

/// The reference layers a type is written with: the text of them (`&`, `&mut `, `&&`), how many,
/// and whether any is mutable. `(String::new(), 0, false)` for a plain type.
fn reference_prefix(ty: &syn::Type) -> (String, usize, bool) {
    let (mut prefix, mut depth, mut has_mut) = (String::new(), 0, false);
    let mut cur = ty;
    while let syn::Type::Reference(r) = cur {
        depth += 1;
        if r.mutability.is_some() {
            has_mut = true;
            prefix.push_str("&mut ");
        } else {
            prefix.push('&');
        }
        cur = &r.elem;
    }
    (prefix, depth, has_mut)
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
    /// A method written `-> bool`. The receiver is the predicate's **first** argument, so
    /// `state ready(u)` against `fn is_ready(&self)` lowers to `u.is_ready()`. Determined by
    /// the signature having a `self` receiver, not by how the operator wrote the binding — a
    /// method found by its bare name is still a method.
    Method {
        name: String,
        /// `Some` when the method comes from a **trait** impl rather than an inherent one (#200),
        /// carrying what the harness needs to call it without importing anything.
        ///
        /// An inherent call (`u.is_ready()`) compiles wherever `u` is in scope. A trait method does
        /// not: `u.is_healthy()` needs the trait in scope, and the harness is generated into a file
        /// that imports nothing of the subject's choosing. So a trait method is called in its
        /// fully-qualified form instead, which needs no import and stays unambiguous when two traits
        /// give one type the same method name.
        via_trait: Option<TraitQualification>,
    },
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

/// Everything the harness needs to write a trait method call in its fully-qualified form —
/// `<crate::status::Status as crate::status::Health>::is_healthy(&s)` (#200).
///
/// Both halves carry the module they are **declared** in, not the one the `impl` block sits in,
/// for the same reason [`PredicateForm::VariantTest`] carries `enum_module`: the harness names each
/// item independently, and the type, the trait and the impl need not share a module. A `None`
/// module is the same refusal it is everywhere else (REQ061) — the item exists, but nothing a
/// harness writes reaches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitQualification {
    pub trait_name: String,
    pub trait_module: Option<Vec<String>>,
    pub type_name: String,
    pub type_module: Option<Vec<String>>,
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
    /// Several declarations answer to the name. Never guessed between — the operator must
    /// disambiguate, because picking one silently would bind the requirement to whichever
    /// file happened to be walked first.
    ///
    /// `kind` says *what* was ambiguous, which is not decoration: the read-back used to call every
    /// candidate a function, so two `struct Entry` declarations were reported as "2 functions share
    /// the name" pointing at struct declarations, and the operator went looking for a second method
    /// that did not exist (#190). The three cases are different questions and only one of them is
    /// about functions.
    Ambiguous {
        kind: AmbiguityKind,
        at: Vec<CodeMatch>,
    },
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
    /// Found, right arity and boolean, but a parameter is written in a form the lowering will not
    /// generate a call for (#201): a multiply-referenced `&&T`, or a `&mut T`.
    ///
    /// The lowering writes exactly one `&` for a reference parameter, because [`ParamMode`] records
    /// only *whether* a parameter is a reference. So `&&T` received `&u` and `&mut T` received `&u`,
    /// and neither compiles. Nothing caught it: [`type_ident`] reads through every reference, so the
    /// parameter cross-check saw the same type name on both sides and agreed.
    ///
    /// Parked here rather than left to the prover. The old reasoning was that a call that fails to
    /// compile is an honest `unknown` — true, and beside the point: the written parameter type was
    /// in front of the adapter at grounding, and the surface exists to move exactly this failure
    /// earlier. An `unknown` carrying a compiler error is the outcome grounding is meant to prevent,
    /// not a satisfactory one.
    UncallableParam {
        /// 1-based position, as the operator counts parameters.
        param: usize,
        /// How the subject writes it — `&&Item`, `&mut Item` — so the reason names the real text.
        written: String,
        at: CodeMatch,
    },
}

/// What was ambiguous about an observable — the half the operator has to disambiguate (#190).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbiguityKind {
    /// Several functions share the name.
    Functions,
    /// Several types share the name. The member may well be unique; it is the receiver that is not.
    Types,
    /// One function and one type share the name, so the observable could be read either way.
    FunctionAndType,
    /// Several enums share the name a function returns, so its variants cannot be pooled.
    Enums,
}

impl AmbiguityKind {
    /// How the candidates read in the ambiguity message. Says what they are, because naming them
    /// wrongly sends the operator hunting for a declaration that is not there.
    fn candidates(self, n: usize) -> String {
        match self {
            AmbiguityKind::Functions => format!("{n} functions share the name"),
            AmbiguityKind::Types => format!(
                "{n} types share the name — the member itself may be unique, but the type it is \
                 reached through is not"
            ),
            AmbiguityKind::FunctionAndType => {
                "the name is both a function and a type, so the binding could be read either way"
                    .to_string()
            }
            AmbiguityKind::Enums => format!(
                "{n} enums share the returned type's name, so its variants cannot be pooled"
            ),
        }
    }
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
            Resolution::Ambiguous { kind, at: ats } => {
                let places = ats
                    .iter()
                    .map(|a| format!("{}:{}", a.file, a.line))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{symbol}: `{observable}` is ambiguous — {} \
                     ({places}); qualify it, because binding to one silently would pick \
                     whichever file was walked first{}",
                    kind.candidates(ats.len()),
                    match kind {
                        AmbiguityKind::Types => offer_paths(ats, &last_two_segments(observable)),
                        _ => String::new(),
                    }
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
            // Not "no *inherent* method" since #200: the search covers trait impls too, and the
            // list below is drawn from both, so saying "inherent" would send an operator looking
            // for a distinction the answer no longer makes.
            Resolution::NoSuchMethod {
                ty,
                method,
                methods,
                at,
            } => format!(
                "{symbol}: `{ty}` at {}:{} has no method `{method}` — {}",
                at.file,
                at.line,
                if methods.is_empty() {
                    "it has no methods at all".to_string()
                } else {
                    format!("its methods are {}", methods.join(", "))
                }
            ),
            Resolution::UncallableParam { param, written, at } => {
                format!(
                    "{symbol}: parameter {param} of the function at {}:{} is written `{written}`, and \
                 the proof would pass it a single `&` — provreq writes one reference for a \
                 reference parameter and will not guess at a deeper one or at a mutable borrow. \
                 Nothing here is wrong with your binding: it is the shape of the signature, and \
                 saying so now is better than a proof that does not compile. A predicate that only \
                 reads its argument can take `&{}`",
                    at.file,
                    at.line,
                    written
                        .trim_start_matches(['&', ' '])
                        .trim_start_matches("mut ")
                )
            }
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
            // The read-back distinguishes the two, because they are two different facts about the
            // subject and the operator is confirming which one they meant (#200).
            PredicateForm::Method {
                name,
                via_trait: None,
            } => {
                format!("an inherent method — checked as `<first argument of {symbol}>.{name}(…)`")
            }
            PredicateForm::Method {
                name,
                via_trait: Some(q),
            } => format!(
                "a method of the trait `{}` — checked as `<{} as {}>::{name}(<first argument of \
                 {symbol}>)`, written out in full so the proof needs no import",
                q.trait_name, q.type_name, q.trait_name
            ),
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
    /// A **type application** — `Wrapper<u32>`, `Wrapper<auth::User>` (#187). Grounds.
    ///
    /// Kept apart from [`TypeResolution::Resolved`] because a harness cannot write this from the
    /// declaration alone: `Wrapper` is where the type is declared, but the type a variable ranges
    /// over is `Wrapper` applied to arguments that are declared somewhere else entirely, and each
    /// of those needs its own path. So each written argument keeps both the text the operator wrote
    /// and its own resolution, and lowering builds the applied path from the pair.
    Applied {
        /// Where the applied type itself is declared.
        at: CodeMatch,
        /// One entry per written argument: the text as written, and what it resolved to. An
        /// argument that resolves to anything but [`ArgResolution`]'s two outcomes makes the whole
        /// application [`TypeResolution::UnusableTypeArguments`] instead, because a domain is only
        /// as real as the types it is built from.
        args: Vec<(String, ArgResolution)>,
    },
    /// The type name is fine, and the arguments written after it are not (#187) — a nested
    /// argument, the wrong number of them, or one that names no type.
    ///
    /// Kept apart from every other refusal because it is the one the operator is closest to
    /// getting right: the type they meant exists and they found it, so a read-back that said "no
    /// such type" would send them to look for something already in front of them. The reason names
    /// which of those it is.
    UnusableTypeArguments {
        /// What is wrong, as a clause the read-back completes.
        reason: String,
    },
}

/// What one written **type argument** of a [`TypeResolution::Applied`] resolved to. Deliberately
/// *not* [`TypeResolution`], for the same reason that is not [`Resolution`]: only the two grounding
/// outcomes can occur here — [`apply`] turns every other one into
/// [`TypeResolution::UnusableTypeArguments`] before an argument is ever kept — and an enum carrying
/// variants a caller can never see misstates the state space.
///
/// It is also what keeps [`TypeResolution`] non-recursive, and that is not incidental (#227).
/// Creusot compiles the whole subject crate and refuses a type that recurs under a type parameter
/// of `Vec` — `Box` does not buy a way out, because it looks through it — so while the arguments
/// held a `TypeResolution`, no category-1 requirement in this repository could reach the deductive
/// route at all, whatever the claim. A type that already had no business being recursive was
/// stopping every proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgResolution {
    /// Exactly one `struct`, `enum`, or `type` alias of that name.
    Resolved(CodeMatch),
    /// One of the language's own primitive types, carrying no [`CodeMatch`] for the reason
    /// [`TypeResolution::Primitive`] carries none.
    Primitive(String),
}

impl ArgResolution {
    /// The argument outcomes a resolution can be narrowed to, and `None` for every outcome that
    /// means the argument did not ground. The caller turns that `None` into the refusal, because
    /// the refusal needs the original resolution to say *which* way it failed.
    fn of(r: &TypeResolution) -> Option<Self> {
        match r {
            TypeResolution::Resolved(at) => Some(ArgResolution::Resolved(at.clone())),
            TypeResolution::Primitive(name) => Some(ArgResolution::Primitive(name.clone())),
            // Includes `Applied`: `split_application` refuses a nested argument before this, so a
            // nested application cannot arrive — and if one ever did, it is not something a harness
            // could path, which is exactly what the refusal says.
            _ => None,
        }
    }
}

impl TypeResolution {
    /// Whether this sort resolved. A quantified claim whose domain names no real type is not
    /// grounded (R-ground-1) — but a primitive is as real a domain as a declared type.
    pub fn is_resolved(&self) -> bool {
        matches!(
            self,
            TypeResolution::Resolved(_)
                | TypeResolution::Primitive(_)
                | TypeResolution::Applied { .. }
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
                    offer_paths(ats, last_segment(observable))
                )
            }
            TypeResolution::QualifierUnmatched { name, candidates } => format!(
                "{sort} (sort): the type `{name}` exists, but not under the path `{observable}` \
                 qualifies it with — so it is the qualifier that is wrong, not the type{}",
                offer_paths(candidates, name)
            ),
            // The arguments are read back too, and each says where *it* was found: the operator is
            // confirming one domain, and `Wrapper<User>` is only the domain they meant if both
            // halves are.
            TypeResolution::Applied { at, args } => format!(
                "{sort} (sort) → `{observable}` resolves to {}:{}  {}, applied to {}",
                at.file,
                at.line,
                at.text,
                args.iter()
                    .map(|(written, r)| match r {
                        ArgResolution::Primitive(name) => format!("`{name}` (the Rust primitive)"),
                        ArgResolution::Resolved(a) =>
                            format!("`{written}` at {}:{}", a.file, a.line),
                    })
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
            TypeResolution::UnusableTypeArguments { reason } => {
                format!("{sort} (sort): `{observable}` {reason}")
            }
        }
    }
}

/// The qualified forms an operator can actually write, taken from where the type is really
/// declared. Advice to "qualify it" is worth nothing without them: the module path is a fact of the
/// subject that provreq walked and the operator would otherwise have to reconstruct by hand.
///
/// `suffix` is what follows the module in the offered form, and it is the caller's to choose
/// because it differs by observable shape: a sort ends at its type name (`pending::Entry`), while a
/// predicate keeps the whole `Type::member` (`pending::Entry::is_clear`). Stripping to the last
/// segment here — which is what this did — offered `pending::is_clear`, which is not a form the
/// adapter accepts, so the way out it advertised did not exist. Caught by running the CLI, not by a
/// test.
///
/// A candidate whose module the walk could not determine (REQ061) is skipped rather than guessed
/// at, so an empty offer means provreq has nothing honest to suggest and says only that.
fn offer_paths(candidates: &[CodeMatch], suffix: &str) -> String {
    let offers: Vec<String> = candidates
        .iter()
        .filter_map(|at| at.module.as_ref())
        .filter(|m| !m.is_empty())
        .map(|m| format!("`{}::{suffix}`", m.join("::")))
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
/// The observable may also **apply type arguments** — `Wrapper<u32>`, `Wrapper<auth::User>` (#187).
/// The PRL sort stays a bare name throughout; it is the *observable* that names the Rust shape, the
/// same side that gained module qualification. So a sort meaning `Wrapper<u32>` costs the
/// requirement language nothing — the requirement says `each w: Wrapped`, and the binding says which
/// type that is, which is exactly the division of labour a binding exists for.
///
/// **One level, and every argument is resolved.** `Wrapper<Vec<u32>>` is refused rather than
/// approximated: `Vec` is not declared by the subject, so provreq has no declaration to confirm a
/// nested argument against, and a domain it cannot confirm is not one it should let a claim range
/// over. The arity is checked against the declaration's own type parameters, so `User<u32>` is
/// refused because `User` takes none — a mistake that would otherwise reach the operator as a
/// harness that does not compile.
///
/// See [`module_matches`] for exactly how much of a written path is checked, and why the rest
/// cannot be.
pub fn resolve_type(subject: &ParsedSubject, observable: &str) -> TypeResolution {
    let (head, args) = match split_application(observable.trim()) {
        Ok(split) => split,
        Err(reason) => return TypeResolution::UnusableTypeArguments { reason },
    };
    let (resolution, declared) = resolve_head(subject, head);
    if args.is_empty() {
        return resolution;
    }
    apply(subject, resolution, declared, &args)
}

/// Resolve the type name itself, ignoring any arguments applied to it — plus **how many type
/// parameters the declaration has**, which only means anything when exactly one declaration matched
/// and is only ever read then.
fn resolve_head(subject: &ParsedSubject, written: &str) -> (TypeResolution, usize) {
    if written.is_empty() {
        return (TypeResolution::NotFound, 0);
    }
    let mut segments: Vec<&str> = written.split("::").map(str::trim).collect();
    // The last segment is the type; everything before it qualifies which one.
    let name = segments.pop().expect("split yields at least one segment");
    if name.is_empty() {
        return (TypeResolution::NotFound, 0);
    }
    let by_name = find_types(subject, name);
    let found: Vec<TypeDecl> = by_name
        .iter()
        .filter(|d| module_matches(d.at.module.as_deref(), &segments))
        .map(|d| TypeDecl {
            at: d.at.clone(),
            type_params: d.type_params,
        })
        .collect();
    // The name is real and only the qualifier missed. Reporting `NotFound` here would deny a type
    // the operator can see in their own source.
    if found.is_empty() && !by_name.is_empty() {
        return (
            TypeResolution::QualifierUnmatched {
                name: name.to_string(),
                candidates: by_name.into_iter().map(|d| d.at).collect(),
            },
            0,
        );
    }
    match found.len() {
        // The subject declares nothing by that name — but the language may. A primitive is only
        // ever the fallback: a subject that declares its own `bool` has a source location the
        // operator can confirm against, and the read-back names it, so the declaration wins and
        // says so rather than being silently overruled by the language.
        0 if is_primitive(name) => (TypeResolution::Primitive(name.to_string()), 0),
        0 => (TypeResolution::NotFound, 0),
        1 => {
            let decl = found.into_iter().next().expect("len checked");
            (TypeResolution::Resolved(decl.at), decl.type_params)
        }
        _ => (
            TypeResolution::Ambiguous(found.into_iter().map(|d| d.at).collect()),
            0,
        ),
    }
}

/// Apply written type arguments to an already-resolved head (#187).
fn apply(
    subject: &ParsedSubject,
    head: TypeResolution,
    declared: usize,
    args: &[&str],
) -> TypeResolution {
    let at = match head {
        TypeResolution::Resolved(at) => at,
        TypeResolution::Primitive(name) => {
            return TypeResolution::UnusableTypeArguments {
                reason: format!(
                    "applies type arguments to `{name}`, one of the language's own primitive \
                     types, which takes none"
                ),
            };
        }
        // Not found, ambiguous, or wrongly qualified: the head's own answer is the one the operator
        // needs, and it already reads correctly. Wrapping it in an argument complaint would bury
        // the fact that the type itself is what could not be found.
        other => return other,
    };
    if declared != args.len() {
        return TypeResolution::UnusableTypeArguments {
            reason: format!(
                "writes {}, but the declaration at {}:{} takes {}",
                count(args.len(), "type argument"),
                at.file,
                at.line,
                count(declared, "type parameter"),
            ),
        };
    }
    let mut resolved = Vec::with_capacity(args.len());
    for written in args {
        // Recursion, bounded to one level by construction: `split_application` refused any argument
        // carrying a `<`, so this call cannot reach `apply` again.
        let r = resolve_type(subject, written);
        let Some(arg) = ArgResolution::of(&r) else {
            return TypeResolution::UnusableTypeArguments {
                reason: argument_problem(written, &r),
            };
        };
        resolved.push(((*written).to_string(), arg));
    }
    TypeResolution::Applied { at, args: resolved }
}

/// `1 type argument`, `2 type arguments`, `no type parameters` — the arity halves of a mismatch
/// read as prose because that is the whole content of the message.
fn count(n: usize, thing: &str) -> String {
    match n {
        0 => format!("no {thing}s"),
        1 => format!("1 {thing}"),
        _ => format!("{n} {thing}s"),
    }
}

/// Why one written type argument is not a type, said in the same terms the sort itself would be —
/// an argument is a domain too, and a wrong one fails for exactly the reasons a wrong sort does.
fn argument_problem(written: &str, r: &TypeResolution) -> String {
    match r {
        TypeResolution::NotFound => format!(
            "applies the type argument `{written}`, and no type of that name is in the subject's \
             Rust, nor is it a primitive"
        ),
        TypeResolution::Ambiguous(ats) => format!(
            "applies the type argument `{written}`, and {} types share that name ({}) — qualify it \
             by module{}",
            ats.len(),
            ats.iter()
                .map(|a| format!("{}:{}", a.file, a.line))
                .collect::<Vec<_>>()
                .join(", "),
            offer_paths(ats, last_segment(written))
        ),
        TypeResolution::QualifierUnmatched { name, candidates } => format!(
            "applies the type argument `{written}`, whose type `{name}` exists but not under the \
             path written for it{}",
            offer_paths(candidates, name)
        ),
        // `split_application` refuses a nested argument before any of this, and a resolved one is
        // not a problem at all.
        _ => format!("applies the type argument `{written}`, which does not name a type"),
    }
}

/// Split a written observable into the type name and the type arguments applied to it —
/// `Wrapper<u32>` → `("Wrapper", ["u32"])`, `auth::User` → `("auth::User", [])` (#187).
///
/// `Err` is for arguments that cannot be read at all, and each case is a different mistake, so each
/// says which it is. A **nested** argument is refused here rather than deeper down because this is
/// where the one-level limit actually lives: everything past this point assumes an argument is a
/// plain type name, and the recursion in [`apply`] is bounded by that assumption holding.
fn split_application(written: &str) -> Result<(&str, Vec<&str>), String> {
    let Some(open) = written.find('<') else {
        return Ok((written, Vec::new()));
    };
    let head = written[..open].trim();
    let Some(inner) = written[open + 1..].trim_end().strip_suffix('>') else {
        return Err(
            "opens a list of type arguments that never closes — a `<` with no matching `>`".into(),
        );
    };
    let inner = inner.trim();
    if inner.contains('<') {
        return Err(format!(
            "applies the type argument `{inner}`, which is itself a type application — provreq \
             reads one level, and the subject declares no such type for a nested argument to be \
             confirmed against"
        ));
    }
    if inner.contains('>') {
        return Err("closes its list of type arguments more than once".into());
    }
    if head.is_empty() {
        return Err(
            "applies type arguments to nothing — there is no type name before the `<`".into(),
        );
    }
    if inner.is_empty() {
        return Err(
            "writes an empty list of type arguments — drop the `<>`, or say what it is applied to"
                .into(),
        );
    }
    let mut args: Vec<&str> = inner.split(',').map(str::trim).collect();
    // A trailing comma is legal Rust and means nothing extra; any *other* empty argument is a
    // slip the operator wants named rather than silently dropped.
    if args.last() == Some(&"") {
        args.pop();
    }
    if args.iter().any(|a| a.is_empty()) {
        return Err("writes an empty type argument between two commas".into());
    }
    Ok((head, args))
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
/// The `Type::member` tail of a predicate observable — `a::b::Entry::is_clear` → `Entry::is_clear`,
/// `Entry::is_clear` → `Entry::is_clear`. What a module qualifier is offered in front of.
fn last_two_segments(written: &str) -> String {
    let segments: Vec<&str> = written.split("::").map(str::trim).collect();
    segments[segments.len().saturating_sub(2)..].join("::")
}

/// The type name at the end of a written path — `session::Session` → `Session`, `Session` →
/// `Session`. The one form both the sort side and the parameter side can always produce.
fn last_segment(written: &str) -> &str {
    written.rsplit("::").next().unwrap_or(written).trim()
}

/// A written type reduced to what two sides can honestly be compared on: the name's last segment,
/// and each type argument's last segment — `wrap::Wrapper<auth::User>` → `("Wrapper", ["User"])`.
///
/// Splitting the arguments off first is not a nicety: `last_segment` alone reads
/// `Wrapper<auth::User>` as `User>`, because the last `::` in the string is inside the argument.
/// A form that cannot be split is compared on its last segment with no arguments, which is what
/// this always did — the sort resolver refuses such an observable separately, and saying it twice
/// would not make it truer.
fn comparable(written: &str) -> (&str, Vec<&str>) {
    match split_application(written) {
        Ok((head, args)) => (
            last_segment(head),
            args.into_iter().map(last_segment).collect(),
        ),
        Err(_) => (last_segment(written), Vec::new()),
    }
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

/// One type declaration found in the subject: where it is, and **how many type parameters it
/// declares** (#187).
///
/// The count is what lets a written type argument be judged rather than ignored. Without it,
/// `User<u32>` and `Wrapper<u32>` are the same shape to this adapter, and the only honest thing
/// left is to drop the argument — which is what it did, and is how a sort meaning `Wrapper<u32>`
/// came to be unwritable. It is kept apart from [`CodeMatch`] because it is a fact about a *type*
/// declaration, and a `CodeMatch` also stands for functions, where it would mean nothing.
struct TypeDecl {
    at: CodeMatch,
    type_params: usize,
}

/// Every `struct`/`enum`/`type` alias named `name`, with the same walk and skip rules as
/// the predicate resolver.
fn find_types(subject: &ParsedSubject, name: &str) -> Vec<TypeDecl> {
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
    out: &mut Vec<TypeDecl>,
) {
    for item in items {
        // Only *type* parameters are counted: a lifetime or a const parameter is not something a
        // sort's written argument can stand for, so including them would refuse a correct
        // `Wrapper<u32>` for a `struct Wrapper<'a, T>` on an arity the operator never wrote.
        let found = match item {
            syn::Item::Struct(s) => Some((&s.ident, s.generics.type_params().count())),
            syn::Item::Enum(e) => Some((&e.ident, e.generics.type_params().count())),
            syn::Item::Type(t) => Some((&t.ident, t.generics.type_params().count())),
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_types(inner, name, rel, text, &inside(module, &m.ident), out);
                }
                None
            }
            _ => None,
        };
        if let Some((ident, type_params)) = found
            && ident == name
        {
            out.push(TypeDecl {
                at: at_ident(ident, rel, text, module),
                type_params,
            });
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
        // `A::B`, optionally preceded by the module `A` is declared in (#189). The last two
        // segments are always the `A::B` this adapter understands; anything before them qualifies
        // which `A` is meant, checked exactly as a sort's qualifier is ([`module_matches`]).
        //
        // A path deeper than `A::B` used to be refused outright, on the grounds that guessing which
        // two segments were meant would bind the requirement to something the operator did not
        // write. There is no guess here: the split is fixed at the right, and the extra segments are
        // verified rather than assumed. What made that comment true was that a written module had no
        // checkable meaning, and #138 gave it one.
        [.., qualifier, member] if !qualifier.is_empty() && !member.is_empty() => {
            resolve_qualified(
                subject,
                &segments[..segments.len() - 2],
                qualifier,
                member,
                params,
            )
        }
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
            // A bare name only ever finds a FREE function ([`find_functions`]), so a receiver here
            // means an inherent method reached without its type — never a trait method, which is
            // why nothing qualifies it.
            let form = if f.has_receiver {
                PredicateForm::Method {
                    name: name.to_string(),
                    via_trait: None,
                }
            } else {
                PredicateForm::Function
            };
            classify(f, params, form)
        }
        _ => Resolution::Ambiguous {
            kind: AmbiguityKind::Functions,
            at: found.into_iter().map(|f| f.at).collect(),
        },
    }
}

/// `A::B` — either a function `A` whose returned enum has a variant `B`, or a type `A` with an
/// inherent method `B`. Which one is decided by what `A` actually is in the subject; a name that
/// is both a function and a type is an ambiguity, never a guess.
fn resolve_qualified(
    subject: &ParsedSubject,
    module: &[&str],
    qualifier: &str,
    member: &str,
    params: &[Option<String>],
) -> Resolution {
    let fns: Vec<FoundFn> = find_functions(subject, qualifier)
        .into_iter()
        .filter(|f| module_matches(f.at.module.as_deref(), module))
        .collect();
    let types: Vec<CodeMatch> = find_types(subject, qualifier)
        .into_iter()
        .map(|d| d.at)
        .filter(|at| module_matches(at.module.as_deref(), module))
        .collect();
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
        // Each of these is a different question, and the read-back must not call them all the same
        // thing (#190): several functions, several types reached through the same name, or one of
        // each — where the observable itself could be read two ways.
        (0, _) => Resolution::Ambiguous {
            kind: AmbiguityKind::Types,
            at: types,
        },
        (_, 0) => Resolution::Ambiguous {
            kind: AmbiguityKind::Functions,
            at: fns.into_iter().map(|f| f.at).collect(),
        },
        _ => Resolution::Ambiguous {
            kind: AmbiguityKind::FunctionAndType,
            at: fns.into_iter().map(|f| f.at).chain(types).collect(),
        },
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
        return Resolution::Ambiguous {
            kind: AmbiguityKind::Enums,
            at: enums.into_iter().map(|e| e.at).collect(),
        };
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
    if let Some(problem) = param_problem(&f, params) {
        return problem;
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
    // Confine the search to the module the chosen type is declared in. `find_methods` matches an
    // `impl` block by its self-type IDENT, which cannot tell `alpha::Entry` from `beta::Entry` —
    // so without this the module qualifier picks the right declaration and is then thrown away:
    // `beta::Entry::is_clear` resolved to alpha's method (#189). A qualifier that is accepted and
    // silently not honoured is worse than one refused, because nothing says the binding moved.
    //
    // Skipped when the type's own module is unknown (REQ061): there is nothing to confine to, and
    // inventing one would be the same over-claim in the other direction.
    let same_module = |f: &FoundFn| match (&ty_at.module, &f.at.module) {
        (Some(want), Some(got)) => want == got,
        _ => true,
    };
    let mut found: Vec<FoundFn> = find_methods(subject, ty, Some(method))
        .into_iter()
        .filter(&same_module)
        .collect();
    match found.len() {
        0 => Resolution::NoSuchMethod {
            ty: ty.to_string(),
            method: method.to_string(),
            methods: find_methods(subject, ty, None)
                .into_iter()
                .filter(&same_module)
                .map(|f| f.name)
                .collect(),
            at: ty_at,
        },
        1 => {
            let f = found.pop().expect("len checked");
            // A trait method is called in its fully-qualified form, which needs the trait's own
            // declaration — not the impl's module, which is where it is *used* (#200). A trait the
            // subject does not declare exactly once cannot be named, and a method that cannot be
            // named cannot be called, so that is a refusal rather than a guess.
            let via_trait = match &f.via_trait {
                None => None,
                Some(trait_name) => {
                    let Some(trait_at) = find_trait(subject, trait_name) else {
                        return Resolution::NoSuchMethod {
                            ty: ty.to_string(),
                            method: method.to_string(),
                            methods: find_methods(subject, ty, None)
                                .into_iter()
                                .filter(&same_module)
                                .map(|f| f.name)
                                .collect(),
                            at: ty_at,
                        };
                    };
                    Some(TraitQualification {
                        trait_name: trait_name.clone(),
                        trait_module: trait_at.module,
                        type_name: ty.to_string(),
                        type_module: ty_at.module.clone(),
                    })
                }
            };
            let form = PredicateForm::Method {
                name: method.to_string(),
                via_trait,
            };
            classify(f, params, form)
        }
        _ => Resolution::Ambiguous {
            kind: AmbiguityKind::Functions,
            at: found.into_iter().map(|f| f.at).collect(),
        },
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
    /// The trait this method came from, when it was found in a trait impl (#200). `None` for a free
    /// function or an inherent method, both of which the harness reaches without naming a trait.
    via_trait: Option<String>,
    /// The first parameter written in a form the lowering will not call, 1-based, with its written
    /// text (#201). `None` when every parameter can be passed.
    uncallable: Option<(usize, String)>,
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
    if let Some(problem) = param_problem(&f, params) {
        return problem;
    }
    Resolution::Resolved {
        at: f.at,
        params: f.params,
        form,
    }
}

/// The first thing wrong with this signature's parameters, if anything: a type that disagrees with
/// the sort the argument ranges over (REQ057), then a shape the lowering cannot pass (#201).
///
/// One function because there are two ways to reach it — an ordinary predicate and a variant test —
/// and they must ask the same questions in the same order. The order is deliberate: a wrong
/// parameter type means the binding names the wrong thing, which the operator can fix, while an
/// uncallable shape is the subject's own and the answer is provreq's limit. Reporting the limit
/// first would bury the fixable mistake underneath it.
fn param_problem(f: &FoundFn, params: &[Option<String>]) -> Option<Resolution> {
    wrong_param_type(f, params).or_else(|| {
        f.uncallable
            .clone()
            .map(|(param, written)| Resolution::UncallableParam {
                param,
                written,
                at: f.at.clone(),
            })
    })
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
///
/// **Type arguments are compared only when both sides carry them** (#187). A sort meaning
/// `Wrapper<u32>` and a parameter written `Wrapper<String>` are a real disagreement, and naming it
/// here is the whole reason a sort may carry arguments at all. But a bare `Wrapper` on either side
/// is not a claim that there are no arguments — it is the absence of one, and every binding written
/// before sorts could carry arguments looks exactly like that. Comparing an absence against a
/// presence would park those, which is the one thing this check may not do.
fn wrong_param_type(f: &FoundFn, params: &[Option<String>]) -> Option<Resolution> {
    f.param_types
        .iter()
        .zip(params)
        .enumerate()
        .find_map(|(i, (found, expected))| {
            let (found, expected) = (found.as_ref()?, expected.as_ref()?);
            let (found_name, found_args) = comparable(found);
            let (expected_name, expected_args) = comparable(expected);
            let differ = found_name != expected_name
                || (!found_args.is_empty()
                    && !expected_args.is_empty()
                    && found_args != expected_args);
            differ.then(|| Resolution::WrongParamType {
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
            .filter_entry(|e| !is_skipped(e, companion_root))
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
                    if let syn::ImplItem::Fn(f) = sub
                        && f.sig.ident == name
                    {
                        out.push(found(&f.sig, rel, text, self_ty.as_deref(), module));
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

/// Methods declared on `ty` — all of them when `name` is `None`, or just the ones so named.
///
/// Both **inherent** (`impl Ty`) and **trait** (`impl Trait for Ty`) impls (#200). Trait impls were
/// skipped, on the reasoning that calling one depends on the trait being in scope and `syn` cannot
/// see scope. True of a method call; not true of the fully-qualified form, which names the trait
/// itself and so needs nothing imported — see [`TraitQualification`]. The cost of skipping them was
/// measured: a predicate bound to a trait method could not ground at all, and a trait is how a large
/// share of real Rust exposes a boolean query.
///
/// A **default** method — a body on the trait with no override in the impl — is still not found,
/// because nothing declares it here. That is a refusal rather than an oversight: its declaration is
/// in the trait, so `at` would point somewhere the operator did not bind.
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
            syn::Item::Impl(i) if self_type_is(&i.self_ty, ty) => {
                // `None` for an inherent impl, the trait's own name for a trait impl — which is
                // what the harness must write to reach the method without an import.
                let via_trait = match &i.trait_ {
                    Some((_, path, _)) => match path.segments.last() {
                        Some(seg) => Some(seg.ident.to_string()),
                        // A trait path with no segments is not a trait this can name, so the impl
                        // is skipped rather than guessed at.
                        None => continue,
                    },
                    None => None,
                };
                for sub in &i.items {
                    if let syn::ImplItem::Fn(f) = sub
                        && name.is_none_or(|n| f.sig.ident == n)
                    {
                        let mut f = found(&f.sig, rel, text, Some(ty), module);
                        f.via_trait = via_trait.clone();
                        out.push(f);
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

/// Where a trait of this name is declared, so a fully-qualified call can name it (#200). Same walk
/// and skip rules as every other lookup; `None` when the subject declares no such trait, which
/// leaves the method uncallable rather than called through a path provreq invented.
fn find_trait(subject: &ParsedSubject, name: &str) -> Option<CodeMatch> {
    let mut out = Vec::new();
    subject.each(|file, rel, text, module| {
        collect_traits(&file.items, name, rel, text, module, &mut out);
    });
    // Exactly one, for the same reason a sort must resolve to exactly one type: choosing between
    // two would bind to whichever file was walked first.
    (out.len() == 1).then(|| out.remove(0))
}

fn collect_traits(
    items: &[syn::Item],
    name: &str,
    rel: &str,
    text: &str,
    module: &Option<Vec<String>>,
    out: &mut Vec<CodeMatch>,
) {
    for item in items {
        match item {
            syn::Item::Trait(t) if t.ident == name => {
                out.push(at_ident(&t.ident, rel, text, module));
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_traits(inner, name, rel, text, &inside(module, &m.ident), out);
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
        // Set by the caller that knows: only `collect_methods` can see the enclosing impl.
        via_trait: None,
        uncallable: sig
            .inputs
            .iter()
            .enumerate()
            .find_map(|(i, arg)| uncallable_param(arg).map(|w| (i + 1, w))),
    }
}

/// The comparable name of one parameter's written type: the last segment of a plain path, so
/// `&mut User` and `crate::auth::User` both read as `User` (the same last-segment convention
/// [`self_type_is`] uses), and the enclosing type for a `self` receiver.
///
/// `None` wherever a written-name comparison would say nothing true: a **generic parameter**
/// (`T` names whatever the caller instantiates — resolving it is type inference, which `syn`
/// does not do), a tuple, a slice, an `impl Trait`.
///
/// Type **arguments** are kept — `Wrapper<u32>` reads as `Wrapper<u32>` (#187) — because the
/// expected side can now carry them too, and a check that dropped them here could not tell
/// `Wrapper<u32>` from `Wrapper<String>`. They are dropped again, back to the bare name, exactly
/// where keeping them would be a lie: an argument that is one of the *function's own* generic
/// parameters (`fn f<T>(w: &Wrapper<T>)`) names whatever the caller instantiates, so comparing it
/// against a real type would be this adapter inventing the answer to a question only inference can
/// settle. [`wrong_param_type`] then compares on the name alone, which is what it always did.
fn param_type_ident(
    arg: &syn::FnArg,
    generics: &[String],
    impl_ty: Option<&str>,
) -> Option<String> {
    match arg {
        syn::FnArg::Receiver(_) => impl_ty.map(str::to_string),
        syn::FnArg::Typed(t) => {
            let name = type_ident(&t.ty)?;
            if generics.contains(&name) {
                return None;
            }
            Some(match type_arguments(&t.ty, generics) {
                Some(args) if !args.is_empty() => format!("{name}<{}>", args.join(", ")),
                _ => name,
            })
        }
    }
}

/// The written type arguments of a type, each reduced to its comparable name — `&Wrapper<u32>` →
/// `["u32"]`, `User` → `[]` (#187).
///
/// `None` means "do not compare arguments at all": one of them is the enclosing function's own
/// generic parameter, or a shape with no single name (a tuple, a slice, a nested application).
/// Lifetimes are skipped rather than refused — nothing on the sort side can name one, and the
/// declaration's arity is counted the same way, so an argument list that differs only by a lifetime
/// is not a disagreement about types.
fn type_arguments(ty: &syn::Type, generics: &[String]) -> Option<Vec<String>> {
    match ty {
        syn::Type::Reference(r) => type_arguments(&r.elem, generics),
        syn::Type::Path(p) if p.qself.is_none() => {
            let args = match &p.path.segments.last()?.arguments {
                syn::PathArguments::None => return Some(Vec::new()),
                syn::PathArguments::AngleBracketed(a) => &a.args,
                syn::PathArguments::Parenthesized(_) => return None,
            };
            let mut out = Vec::new();
            for arg in args {
                match arg {
                    syn::GenericArgument::Lifetime(_) => continue,
                    syn::GenericArgument::Type(t) => {
                        let name = type_ident(t)?;
                        if generics.contains(&name) {
                            return None;
                        }
                        out.push(name);
                    }
                    _ => return None,
                }
            }
            Some(out)
        }
        _ => None,
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

/// What the subject declares that a binding could name (REQ072, #259): bool-returning
/// functions as predicate candidates, declared types as sort candidates. Names only, sorted,
/// deduplicated — the triage prompt needs what exists, not where it lives; resolution
/// (REQ025/REQ026) stays the sole authority on whether a specific binding holds.
pub struct Inventory {
    pub predicates: Vec<String>,
    pub sorts: Vec<String>,
}

/// Enumerate the parsed subject's observable candidates (REQ072). The predicate test is the
/// same syntactic `-> bool` this adapter grounds by ([`return_type`]) — a `Result<bool>` is
/// not a predicate here for the same reason it does not ground as one.
///
/// Implements: REQ072
pub fn inventory(subject: &ParsedSubject) -> Inventory {
    let mut predicates = std::collections::BTreeSet::new();
    let mut sorts = std::collections::BTreeSet::new();
    subject.each(|ast, _, _, _| collect_inventory(&ast.items, &mut predicates, &mut sorts));
    Inventory {
        predicates: predicates.into_iter().collect(),
        sorts: sorts.into_iter().collect(),
    }
}

/// The inventory walk, descending into inline modules and impl blocks exactly as
/// [`collect_fns`] does — the two must agree on where a declaration can live.
fn collect_inventory(
    items: &[syn::Item],
    predicates: &mut std::collections::BTreeSet<String>,
    sorts: &mut std::collections::BTreeSet<String>,
) {
    for item in items {
        match item {
            syn::Item::Fn(f) => {
                if return_type(&f.sig) == "bool" {
                    predicates.insert(f.sig.ident.to_string());
                }
            }
            syn::Item::Struct(s) => {
                sorts.insert(s.ident.to_string());
            }
            syn::Item::Enum(e) => {
                sorts.insert(e.ident.to_string());
            }
            syn::Item::Type(t) => {
                sorts.insert(t.ident.to_string());
            }
            syn::Item::Impl(imp) => {
                for impl_item in &imp.items {
                    if let syn::ImplItem::Fn(f) = impl_item
                        && return_type(&f.sig) == "bool"
                    {
                        predicates.insert(f.sig.ident.to_string());
                    }
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_inventory(inner, predicates, sorts);
                }
            }
            _ => {}
        }
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
/// The type of the `impl` block enclosing the function whose ident is on `line`, or `None` when it
/// is a free function (or not found).
///
/// A method's source alone does not say what `self` is — `pub fn is_clear(&self) -> bool` names no
/// type — so a mirror that must turn `&self` into an ordinary first parameter needs this. It is a
/// fact of the subject the walk already passes over; asking the model to supply it instead is what
/// produced `s: &Ent`, the requirement's own sort symbol, which is not a Rust type at all (#191).
pub fn impl_type_at(text: &str, line: usize) -> Option<String> {
    fn walk(items: &[syn::Item], line: usize) -> Option<String> {
        for item in items {
            match item {
                syn::Item::Mod(m) => {
                    if let Some((_, inner)) = &m.content
                        && let Some(found) = walk(inner, line)
                    {
                        return Some(found);
                    }
                }
                syn::Item::Impl(i) => {
                    let holds_it = i.items.iter().any(|sub| {
                        matches!(sub, syn::ImplItem::Fn(f)
                            if f.sig.ident.span().start().line == line)
                    });
                    if holds_it {
                        return type_ident(&i.self_ty);
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(&syn::parse_file(text).ok()?.items, line)
}

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
                if let Some((_, inner)) = &m.content
                    && let Some(span) = find_fn_span(inner, line)
                {
                    return Some(span);
                }
            }
            syn::Item::Impl(i) => {
                for sub in &i.items {
                    if let syn::ImplItem::Fn(f) = sub
                        && let Some(span) = end_line(&f.sig, &f.block)
                    {
                        return Some(span);
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

    // Verifies: REQ060 / #294 — a hidden file may be the author's own, so the walk reads it. Until
    // the file rule existed this was excluded by `is_pruned_dir`'s hidden rule, applied to an entry
    // it was never an argument about — the over-reach that made one shared rule mean two things.
    #[test]
    fn a_hidden_source_file_is_still_the_subjects_own() {
        let tmp = subject("pub fn is_ready() -> bool { true }\n");
        std::fs::write(
            tmp.path().join("src/.generated.rs"),
            "pub fn is_hidden_but_authored() -> bool { true }\n",
        )
        .unwrap();

        let parsed = parsed(&tmp);
        let walked: Vec<&str> = parsed.files.iter().map(|f| f.rel.as_str()).collect();
        assert!(
            walked.contains(&"src/.generated.rs"),
            "a dotted name is not evidence of being unauthored, got {walked:?}"
        );
    }

    // Verifies: REQ060 / #292 — an operating system's resource files are not the subject's source,
    // and the walk excludes them by rule rather than by luck. The sidecar here holds *valid Rust*
    // on purpose: a `._`-prefixed AppleDouble copy carries the extension of the file it shadows, so
    // this module's `extension() != "rs"` filter would admit it, and nothing downstream would reject
    // it on content either. Only [`crate::subject_tree::is_pruned_dir`] — which `filter_entry`
    // applies to file entries as well as directories — keeps it out. Were it to get in, it would be
    // a second declaration of a name the operator wrote once, parking a correct binding as
    // `Ambiguous` against a file nobody wrote: the `.claude-home/` failure, one file at a time.
    #[test]
    fn mac_resource_files_never_become_binding_candidates() {
        let tmp = subject("pub fn is_ready() -> bool { true }\n");
        let src = tmp.path().join("src");
        std::fs::write(
            src.join("._auth.rs"),
            "pub fn is_ready() -> bool { false }\n",
        )
        .unwrap();
        std::fs::write(
            src.join(".DS_Store"),
            "pub fn is_ready() -> bool { false }\n",
        )
        .unwrap();

        let parsed = parsed(&tmp);
        let walked: Vec<&str> = parsed.files.iter().map(|f| f.rel.as_str()).collect();
        assert_eq!(
            walked,
            ["src/auth.rs"],
            "only the authored file is the subject's"
        );

        let Resolution::Resolved { at, .. } = resolve(&parsed, "is_ready", &[]) else {
            panic!("the sidecar must not make the real function ambiguous")
        };
        assert_eq!(at.file, "src/auth.rs");
    }

    // Verifies: REQ072 / #259 — the inventory names what an adapter could bind: bool-returning
    // functions (free, methods, and inside inline modules) as predicates, declared types as
    // sorts — and nothing else. Sorted and deduplicated, so the prompt is deterministic.
    #[test]
    fn inventory_names_predicates_and_sorts() {
        let tmp = subject(
            "pub struct User { ok: bool }\n\
             pub enum Mode { A }\n\
             pub type Alias = u8;\n\
             pub fn is_ready(u: &User) -> bool { u.ok }\n\
             pub fn count() -> u32 { 0 }\n\
             pub fn fallible() -> Result<bool, ()> { Ok(true) }\n\
             impl User { pub fn is_clear(&self) -> bool { self.ok } }\n\
             mod inner { pub fn is_deep() -> bool { true } }\n",
        );
        let parsed = ParsedSubject::load(tmp.path(), &tmp.path().join("no-companion"));
        let inv = inventory(&parsed);
        assert_eq!(inv.predicates, ["is_clear", "is_deep", "is_ready"]);
        assert_eq!(inv.sorts, ["Alias", "Mode", "User"]);
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
            Resolution::Ambiguous { .. }
        ));

        let r = resolve_in(&tmp, "Engine::is_ready", 1);
        let Resolution::Resolved { form, at, .. } = &r else {
            panic!("should resolve, got {r:?}")
        };
        assert_eq!(
            form,
            &PredicateForm::Method {
                name: "is_ready".into(),
                via_trait: None
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
                name: "ready".into(),
                via_trait: None
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

    // Verifies: #200 — a TRAIT method resolves, carrying what a harness needs to call it without
    // importing the trait.
    //
    // This test previously asserted the opposite, as `a_trait_method_is_not_an_inherent_method`:
    // that a trait impl must NOT resolve, because calling one depends on the trait being in scope
    // and `syn` cannot see scope. That reasoning was sound for a *method call* and was never true of
    // the fully-qualified form, which names the trait and so needs nothing imported. The belief was
    // right when written and quietly stopped being right — the fifth time in this codebase a test
    // has held a limitation in place rather than catching one. Measured: a predicate bound to a
    // trait method could not ground at all, which is how a very large share of real Rust exposes a
    // boolean query.
    #[test]
    fn a_trait_method_resolves_and_carries_the_trait_it_came_from() {
        let tmp = subject(
            "pub struct S;\npub trait Ready { fn ready(&self) -> bool; }\n\
             impl Ready for S { fn ready(&self) -> bool { true } }\n",
        );
        let r = resolve_in(&tmp, "S::ready", 1);
        let Resolution::Resolved { form, .. } = &r else {
            panic!("a trait method resolves, got {r:?}")
        };
        let PredicateForm::Method {
            name,
            via_trait: Some(q),
        } = form
        else {
            panic!("it is a method qualified by its trait, got {form:?}")
        };
        assert_eq!(name, "ready");
        assert_eq!(q.trait_name, "Ready");
        assert_eq!(q.type_name, "S");
        // Each half carries where it is DECLARED, which is what the harness names it by.
        assert_eq!(
            q.trait_module.as_deref(),
            Some(["auth".to_string()].as_ref())
        );
        assert_eq!(
            q.type_module.as_deref(),
            Some(["auth".to_string()].as_ref())
        );

        // The read-back says which kind it is, so the operator confirms the right fact.
        let msg = r.describe("ready", "S::ready");
        assert!(msg.contains("trait `Ready`"), "{msg}");
        assert!(msg.contains("no import"), "{msg}");
    }

    // Verifies: #200 — an INHERENT method is still unqualified, so it keeps lowering to the plain
    // method call it always did. The trait work must not change the form that already worked.
    #[test]
    fn an_inherent_method_carries_no_trait() {
        let tmp = subject("pub struct S;\nimpl S { pub fn ready(&self) -> bool { true } }\n");
        let r = resolve_in(&tmp, "S::ready", 1);
        let Resolution::Resolved { form, .. } = &r else {
            panic!("should resolve, got {r:?}")
        };
        assert!(
            matches!(
                form,
                PredicateForm::Method {
                    via_trait: None,
                    ..
                }
            ),
            "an inherent method names no trait: {form:?}"
        );
    }

    // Verifies: #200 — a trait the subject does not declare cannot be named, so the method cannot
    // be called, so it does not resolve. An `impl SomeExternalTrait for S` names a trait whose
    // declaration is in another crate; writing a path to it would be a path provreq invented.
    #[test]
    fn a_method_of_a_trait_the_subject_does_not_declare_does_not_resolve() {
        let tmp = subject(
            "pub struct S;\nimpl std::fmt::Debug for S { fn ready(&self) -> bool { true } }\n",
        );
        let r = resolve_in(&tmp, "S::ready", 1);
        assert!(!r.is_resolved(), "an unnameable trait cannot ground: {r:?}");
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
        let Resolution::Ambiguous { at: ats, .. } = &r else {
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
        assert!(matches!(r, Resolution::Ambiguous { .. }), "got {r:?}");
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
        assert!(
            r.describe("Session", "Session")
                .contains("cannot range over")
        );
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

    // Verifies: #189 — a predicate may carry a module qualifier on its receiver type, so an
    // ambiguous type name is not a dead end for predicates the way it stopped being one for sorts.
    // Found by the fifth end-to-end pass: on a subject declaring `Entry` twice, the sort grounded
    // via `pending::Entry` and the predicate could not be named at all, so the requirement parked.
    // The adapter refused any observable deeper than `A::B`, on the grounds that guessing which two
    // segments were meant would bind something the operator did not write — true until #138 gave a
    // written module a checkable meaning.
    #[test]
    fn a_predicate_receiver_can_be_qualified_by_module() {
        let tmp = subject(
            "mod alpha { pub struct Entry; impl Entry { pub fn is_clear(&self) -> bool { true } } }\n\
             mod beta { pub struct Entry; }\n",
        );

        let r = resolve_typed(&tmp, "alpha::Entry::is_clear", &[Some("Entry".to_string())]);
        assert!(
            matches!(r, Resolution::Resolved { .. }),
            "a qualified receiver must resolve, got {r:?}"
        );

        // The qualifier discriminates: `beta::Entry` has no such method.
        let wrong = resolve_typed(&tmp, "beta::Entry::is_clear", &[Some("Entry".to_string())]);
        assert!(
            !matches!(wrong, Resolution::Resolved { .. }),
            "the qualifier must pick a type, not merely be tolerated: {wrong:?}"
        );

        // And the unqualified form is still the ambiguity it always was.
        assert!(matches!(
            resolve_typed(&tmp, "Entry::is_clear", &[Some("Entry".to_string())]),
            Resolution::Ambiguous { .. }
        ));
    }

    // Verifies: #190 — an ambiguity says WHICH half was ambiguous, and offers forms that exist.
    // Measured: two `struct Entry` declarations were reported as "2 functions share the name"
    // pointing at struct declarations, with exactly one `is_clear` in the subject — sending the
    // operator to look for a second method that was not there. The offer was wrong too, in a way
    // only a live run showed: it stripped to the last segment and proposed `pending::is_clear`,
    // which the adapter does not accept, so the advertised way out did not exist.
    #[test]
    fn an_ambiguity_names_what_was_ambiguous_and_offers_a_form_that_works() {
        let tmp = subject(
            "mod alpha { pub struct Entry; impl Entry { pub fn is_clear(&self) -> bool { true } } }\n\
             mod beta { pub struct Entry; }\n",
        );
        let r = resolve_typed(&tmp, "Entry::is_clear", &[Some("Entry".to_string())]);
        let Resolution::Ambiguous { kind, .. } = &r else {
            panic!("should be ambiguous, got {r:?}")
        };
        assert_eq!(
            *kind,
            AmbiguityKind::Types,
            "the METHOD is unique; the type is not"
        );

        let text = r.describe("clear", "Entry::is_clear");
        assert!(text.contains("types share the name"), "{text}");
        assert!(
            !text.contains("functions share the name"),
            "a struct must not be reported as a function: {text}"
        );
        // The offered forms must be ones the adapter would actually accept.
        for offered in [
            "`auth::alpha::Entry::is_clear`",
            "`auth::beta::Entry::is_clear`",
        ] {
            assert!(text.contains(offered), "must offer {offered}: {text}");
        }
    }

    // Verifies: #190 — several functions of one name still report as functions. The kind must
    // discriminate rather than relabel every ambiguity as the newest case.
    #[test]
    fn several_functions_of_one_name_are_still_reported_as_functions() {
        let tmp =
            subject("pub fn login() -> bool { true }\nmod m { pub fn login() -> bool { true } }\n");
        let r = resolve_in(&tmp, "login", 0);
        let Resolution::Ambiguous { kind, .. } = &r else {
            panic!("should be ambiguous, got {r:?}")
        };
        assert_eq!(*kind, AmbiguityKind::Functions);
        assert!(
            r.describe("l", "login")
                .contains("functions share the name")
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
            TypeResolution::Ambiguous { .. }
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

    // Verifies: #201 — a parameter the lowering cannot pass is refused at GROUNDING, naming the
    // written type, instead of grounding green and lowering to a harness that will not compile.
    //
    // Measured with the harness captured from a real run: `fn deep(item: &&Item)` grounded, and the
    // harness emitted `nested::deep(&i)` — one `&` for a `&&Item`. Three checks agreed it was fine,
    // because none of them tracks reference depth: `param_mode` flattens any depth to `ByRef`, the
    // lowering writes exactly one `&`, and `type_ident` reads through references so REQ057 compared
    // `Item` against `Item`.
    #[test]
    fn a_parameter_the_lowering_cannot_pass_is_refused_at_grounding() {
        for (src, written) in [
            ("pub fn deep(item: &&Item) -> bool { item.ok }", "&&Item"),
            (
                "pub fn deep(item: &mut Item) -> bool { item.ok }",
                "&mut Item",
            ),
            ("pub fn deep(item: &&&Item) -> bool { item.ok }", "&&&Item"),
        ] {
            let tmp = subject(&format!("pub struct Item {{ pub ok: bool }}\n{src}\n"));
            let r = resolve_typed(&tmp, "deep", &[want("Item")]);
            let Resolution::UncallableParam {
                param,
                written: got,
                ..
            } = &r
            else {
                panic!("`{written}` must not ground, got {r:?}")
            };
            assert_eq!(param, &1, "1-based, as an operator counts");
            assert_eq!(got, written, "the reason names the real text");
            assert!(!r.is_resolved());

            let msg = r.describe("deep", "deep");
            assert!(msg.contains(written), "names how it is written: {msg}");
            assert!(
                msg.contains("not") && msg.contains("your binding"),
                "says the binding is not at fault: {msg}"
            );
        }
    }

    // Verifies: #201 — the refusal is narrow. A single `&`, a plain value, and a `&self` receiver
    // are all forms the lowering DOES write correctly, and refusing them would take away reach that
    // works today. `&self` especially: the call is written on the receiver, which supplies its own
    // reference, so it never sees the single-`&` rule at all.
    #[test]
    fn an_ordinary_reference_or_receiver_is_not_refused() {
        let by_ref = subject("pub struct I;\npub fn f(i: &I) -> bool { true }\n");
        assert!(resolve_typed(&by_ref, "f", &[want("I")]).is_resolved());

        let by_value = subject("pub struct I;\npub fn f(i: I) -> bool { true }\n");
        assert!(resolve_typed(&by_value, "f", &[want("I")]).is_resolved());

        let receiver = subject("pub struct I;\nimpl I { pub fn f(&self) -> bool { true } }\n");
        assert!(resolve_in(&receiver, "I::f", 1).is_resolved(), "&self");

        // `mut self` BY VALUE is the callee's own binding, invisible at the call site — `u.f()`
        // compiles, so refusing it would cost reach for nothing.
        let mut_self = subject("pub struct I;\nimpl I { pub fn f(mut self) -> bool { true } }\n");
        assert!(resolve_in(&mut_self, "I::f", 1).is_resolved(), "mut self");
    }

    // Verifies: #201 — a `&mut self` receiver is refused too. The harness declares its value
    // without `mut`, so `u.f()` cannot borrow it mutably; this is the same defect as a `&mut T`
    // parameter, reached through the receiver instead.
    #[test]
    fn a_mutable_receiver_is_refused_like_a_mutable_parameter() {
        let tmp = subject("pub struct I;\nimpl I { pub fn f(&mut self) -> bool { true } }\n");
        let r = resolve_in(&tmp, "I::f", 1);
        let Resolution::UncallableParam { written, .. } = &r else {
            panic!("a mutable receiver cannot be called from the harness, got {r:?}")
        };
        assert_eq!(written, "&mut self");
    }

    // Verifies: #201 — a binding error the operator can FIX is reported ahead of provreq's own
    // limit. Both are true of this signature; the wrong parameter type is the one they can act on,
    // and burying it under a shape limitation would send them to the wrong place.
    #[test]
    fn a_fixable_binding_error_is_reported_before_provreqs_own_limit() {
        let tmp = subject(
            "pub struct Item;\npub struct Other;\npub fn f(x: &mut Other) -> bool { true }\n",
        );
        let r = resolve_typed(&tmp, "f", &[want("Item")]);
        assert!(
            matches!(r, Resolution::WrongParamType { .. }),
            "the fixable mistake comes first: {r:?}"
        );
    }

    // --- type applications (#187) -------------------------------------------

    /// A subject with a generic type, a two-parameter one, and a plain one to apply as an argument.
    const GENERIC_SUBJECT: &str = "\
pub struct User;
pub struct Wrapper<T> { pub inner: T }
pub struct Pair<A, B> { pub a: A, pub b: B }
pub struct Held<'a, T> { pub held: &'a T }
";

    // Verifies: REQ069 — a sort's observable may apply type arguments, so a domain meaning
    // `Wrapper<u32>` can be written at all. The PRL sort stays a bare name; it is the binding that
    // says which type it is, which is the whole division of labour a binding exists for.
    #[test]
    fn a_sort_observable_may_apply_a_type_argument() {
        let tmp = subject(GENERIC_SUBJECT);
        let r = resolve_type(&parsed(&tmp), "Wrapper<u32>");
        let TypeResolution::Applied { at, args } = &r else {
            panic!("should resolve as an application, got {r:?}")
        };
        assert_eq!(at.line, 2);
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].0, "u32");
        assert!(matches!(&args[0].1, ArgResolution::Primitive(n) if n == "u32"));
        assert!(r.is_resolved(), "an application is a real domain");
    }

    // Verifies: REQ069 — an argument is a domain too, so it resolves the same way a sort does:
    // against the subject's own declarations, and through a module qualifier when one is written.
    #[test]
    fn a_type_argument_resolves_against_the_subject_like_any_sort() {
        let tmp = subject(GENERIC_SUBJECT);
        let r = resolve_type(&parsed(&tmp), "Wrapper<auth::User>");
        let TypeResolution::Applied { args, .. } = &r else {
            panic!("should resolve, got {r:?}")
        };
        let ArgResolution::Resolved(at) = &args[0].1 else {
            panic!("the argument names a declared type, got {:?}", args[0].1)
        };
        assert_eq!((at.file.as_str(), at.line), ("src/auth.rs", 1));

        // Two arguments, in the order written.
        let two = resolve_type(&parsed(&tmp), "Pair<User, u32>");
        let TypeResolution::Applied { args, .. } = &two else {
            panic!("should resolve, got {two:?}")
        };
        assert_eq!(
            args.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>(),
            vec!["User", "u32"]
        );
    }

    // Verifies: REQ069 — the arity is checked against the declaration's own type parameters, so a
    // sort that applies arguments to a type taking none is refused *here*, naming the declaration,
    // rather than reaching the operator later as a harness that does not compile.
    #[test]
    fn type_arguments_are_checked_against_the_declarations_arity() {
        let tmp = subject(GENERIC_SUBJECT);
        let none = resolve_type(&parsed(&tmp), "User<u32>");
        let TypeResolution::UnusableTypeArguments { reason } = &none else {
            panic!("`User` takes no arguments, got {none:?}")
        };
        assert!(reason.contains("no type parameters"), "{reason}");
        assert!(reason.contains("src/auth.rs:1"), "names it: {reason}");
        assert!(!none.is_resolved());

        let short = resolve_type(&parsed(&tmp), "Pair<u32>");
        let TypeResolution::UnusableTypeArguments { reason } = &short else {
            panic!("`Pair` takes two, got {short:?}")
        };
        assert!(reason.contains("1 type argument"), "{reason}");
        assert!(reason.contains("2 type parameters"), "{reason}");

        // A lifetime is not something a sort could name, so it is not counted: `Held<'a, T>` takes
        // one *type* argument, and demanding two would refuse a correct binding on an arity the
        // operator never wrote.
        assert!(
            resolve_type(&parsed(&tmp), "Held<u32>").is_resolved(),
            "a lifetime parameter is not a type argument"
        );
    }

    // Verifies: REQ069 — one level, and the refusal says so. `Vec` is not declared by the subject,
    // so provreq has no declaration to confirm a nested argument against, and a domain it cannot
    // confirm is not one it should let a claim range over.
    #[test]
    fn a_nested_type_argument_is_refused_by_name() {
        let tmp = subject(GENERIC_SUBJECT);
        let r = resolve_type(&parsed(&tmp), "Wrapper<Vec<u32>>");
        let TypeResolution::UnusableTypeArguments { reason } = &r else {
            panic!("a nested argument is refused, got {r:?}")
        };
        assert!(reason.contains("Vec<u32>"), "names the argument: {reason}");
        assert!(reason.contains("one level"), "says why: {reason}");
    }

    // Verifies: REQ069 — an argument that names no type refuses the whole application, and the
    // read-back is the one the operator needs: which argument, and what is wrong with it.
    #[test]
    fn an_argument_that_names_no_type_refuses_the_application() {
        let tmp = subject(GENERIC_SUBJECT);
        let r = resolve_type(&parsed(&tmp), "Wrapper<Nope>");
        let TypeResolution::UnusableTypeArguments { reason } = &r else {
            panic!("an unresolvable argument is refused, got {r:?}")
        };
        assert!(reason.contains("Nope"), "names the argument: {reason}");

        // A malformed list is its own mistake, not "no such type".
        for (written, expect) in [
            ("Wrapper<u32", "never closes"),
            ("Wrapper<>", "empty list"),
            ("u32<bool>", "primitive"),
        ] {
            let r = resolve_type(&parsed(&tmp), written);
            let TypeResolution::UnusableTypeArguments { reason } = &r else {
                panic!("`{written}` is refused, got {r:?}")
            };
            assert!(reason.contains(expect), "`{written}` says why: {reason}");
        }
    }

    // Verifies: REQ069 — the head's own answer wins when the *type* is what could not be found. An
    // argument complaint on top would bury the fact that there is no such type to apply anything to.
    #[test]
    fn a_missing_head_type_reads_as_missing_not_as_bad_arguments() {
        let tmp = subject(GENERIC_SUBJECT);
        assert_eq!(
            resolve_type(&parsed(&tmp), "Nothing<u32>"),
            TypeResolution::NotFound
        );
    }

    // Verifies: REQ069 — the read-back confirms *both* halves of the domain. The operator is
    // confirming one type, and `Wrapper<User>` is only what they meant if the argument is too.
    #[test]
    fn the_readback_of_an_application_names_every_part() {
        let tmp = subject(GENERIC_SUBJECT);
        let text = resolve_type(&parsed(&tmp), "Wrapper<User>").describe("W", "Wrapper<User>");
        assert!(text.contains("src/auth.rs:2"), "the applied type: {text}");
        assert!(text.contains("src/auth.rs:1"), "the argument: {text}");
    }

    // Verifies: REQ057 + REQ069 — the parameter cross-check now discriminates on type arguments,
    // which is the half of #187 that made `Wrapper<u32>` and `Wrapper<String>` one name to it.
    #[test]
    fn the_parameter_check_discriminates_on_type_arguments() {
        let tmp = subject(
            "pub struct Wrapper<T> { pub inner: T }\n\
             pub fn holds(w: &Wrapper<String>) -> bool { true }\n",
        );
        let r = resolve_typed(&tmp, "holds", &[want("Wrapper<u32>")]);
        let Resolution::WrongParamType {
            expected, found, ..
        } = &r
        else {
            panic!("a different argument is a different type, got {r:?}")
        };
        assert_eq!(expected, "Wrapper<u32>");
        assert_eq!(found, "Wrapper<String>");

        // The same argument agrees.
        assert!(resolve_typed(&tmp, "holds", &[want("Wrapper<String>")]).is_resolved());
        // And a qualifier on either side is not a disagreement: both are compared on last
        // segments, the same limit every check on this surface works under.
        assert!(
            resolve_typed(&tmp, "holds", &[want("wrap::Wrapper<std::string::String>")])
                .is_resolved()
        );
    }

    // Verifies: REQ057 — the check may not turn a working binding into a park. A bare `Wrapper` on
    // either side is the *absence* of an argument, not a claim that there is none, and every
    // binding written before sorts could carry arguments looks exactly like that.
    #[test]
    fn a_side_that_writes_no_type_arguments_is_compared_on_the_name_alone() {
        let tmp = subject(
            "pub struct Wrapper<T> { pub inner: T }\n\
             pub fn holds(w: &Wrapper<String>) -> bool { true }\n",
        );
        assert!(
            resolve_typed(&tmp, "holds", &[want("Wrapper")]).is_resolved(),
            "a bare sort still grounds against a generic parameter"
        );

        // The subject's own side: an argument that is the function's generic parameter names
        // whatever the caller instantiates, so comparing it against a real type would be inventing
        // the answer to a question only inference can settle.
        let generic = subject(
            "pub struct Wrapper<T> { pub inner: T }\n\
             pub fn holds<T>(w: &Wrapper<T>) -> bool { true }\n",
        );
        assert!(
            resolve_typed(&generic, "holds", &[want("Wrapper<u32>")]).is_resolved(),
            "`Wrapper<T>` is compared on `Wrapper` alone"
        );
    }
}
