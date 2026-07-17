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
}

/// Whether a directory is pruned from the walk: the VCS metadata dir, or the companion
/// tree (whose `drafts.yml` holds the observables themselves — resolving there would be a
/// spurious self-hit).
fn is_skipped_dir(path: &Path, companion_root: &Path) -> bool {
    if path == companion_root {
        return true;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == ".git")
        .unwrap_or(false)
}

/// What resolving one cat-1 binding against the subject's Rust found. Every non-resolved
/// variant is a *distinct operator action* — a typo, a name collision, a wrong predicate,
/// or a non-boolean — so they stay distinct rather than collapsing to "not found".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Exactly one function of that name, with matching arity, syntactically returning
    /// `bool`. The only variant that grounds.
    Resolved(CodeMatch),
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
    /// stand for a state predicate.
    NotBoolean { returns: String, at: CodeMatch },
}

impl Resolution {
    /// Whether this binding resolved — the single question [`crate::grounding::verdict`]
    /// asks. Only [`Resolution::Resolved`] grounds; everything else parks the requirement
    /// (R-ground-1: a no-resolve never fakes a verdict).
    pub fn is_resolved(&self) -> bool {
        matches!(self, Resolution::Resolved(_))
    }

    /// The operator-facing read-back for one binding (D13: "here is what your binding
    /// resolves to — is that what you meant?"). A resolved predicate names the limit of
    /// what was actually checked, so a green line never implies a type-check that `syn`
    /// cannot perform.
    pub fn describe(&self, symbol: &str, observable: &str) -> String {
        match self {
            Resolution::Resolved(at) => format!(
                "{symbol} → `{observable}` resolves to {}:{}  {}\n      (syntactic check \
                 only — `syn` sees no types, so a `bool` alias or `Result<bool>` would \
                 pass here)",
                at.file, at.line, at.text
            ),
            Resolution::NotFound => format!(
                "{symbol}: no function `{observable}` in the subject's Rust — nothing to \
                 check it through"
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
                 state predicate must be a boolean over program state",
                at.file, at.line
            ),
        }
    }
}

/// Resolve `observable` (a function name) against the subject's Rust, requiring `arity`
/// parameters to match the PRL predicate's declared arity. Read-only over the subject and
/// recomputed live — code moves under a binding exactly as prose moves under a draft, so a
/// resolution is never stored.
pub fn resolve(
    subject_root: &Path,
    companion_root: &Path,
    observable: &str,
    arity: usize,
) -> Resolution {
    let name = observable.trim();
    if name.is_empty() {
        return Resolution::NotFound;
    }
    let found = find_functions(subject_root, companion_root, name);
    match found.len() {
        0 => Resolution::NotFound,
        1 => classify(found.into_iter().next().expect("len checked"), arity),
        _ => Resolution::Ambiguous(found.into_iter().map(|f| f.at).collect()),
    }
}

/// One function declaration found in the subject, with the facts the check needs.
struct FoundFn {
    at: CodeMatch,
    arity: usize,
    returns: String,
}

/// Decide whether a single found function can stand for the predicate. Arity is checked
/// before the return type so the message names the more fundamental mismatch first.
fn classify(f: FoundFn, arity: usize) -> Resolution {
    if f.arity != arity {
        return Resolution::WrongArity {
            expected: arity,
            found: f.arity,
            at: f.at,
        };
    }
    if f.returns != "bool" {
        return Resolution::NotBoolean {
            returns: f.returns,
            at: f.at,
        };
    }
    Resolution::Resolved(f.at)
}

/// Every function named `name` in the subject's `.rs` files, including inside inline
/// `mod` blocks and `impl` blocks. Unparseable files are skipped rather than failing the
/// run — a subject may legitimately contain a Rust file this parser cannot read (a newer
/// edition, a generated fixture), and one bad file must not blind the whole resolution.
fn find_functions(subject_root: &Path, companion_root: &Path, name: &str) -> Vec<FoundFn> {
    let mut out = Vec::new();
    for entry in WalkDir::new(subject_root)
        .into_iter()
        .filter_entry(|e| !is_skipped_dir(e.path(), companion_root))
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() || entry.path().extension().is_some_and(|x| x != "rs") {
            continue;
        }
        if entry.path().extension().is_none() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(file) = syn::parse_file(&text) else {
            continue;
        };
        let rel = entry
            .path()
            .strip_prefix(subject_root)
            .unwrap_or(entry.path())
            .display()
            .to_string();
        collect_fns(&file.items, name, &rel, &text, &mut out);
    }
    out
}

/// Walk items for functions named `name`, descending into inline modules and impl blocks
/// so a predicate declared inside one is still found.
fn collect_fns(items: &[syn::Item], name: &str, rel: &str, text: &str, out: &mut Vec<FoundFn>) {
    for item in items {
        match item {
            syn::Item::Fn(f) if f.sig.ident == name => out.push(found(&f.sig, rel, text)),
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_fns(inner, name, rel, text, out);
                }
            }
            syn::Item::Impl(i) => {
                for sub in &i.items {
                    if let syn::ImplItem::Fn(f) = sub {
                        if f.sig.ident == name {
                            out.push(found(&f.sig, rel, text));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Build the record for one matched signature: where it is, how many parameters it takes,
/// and how its return type is written.
fn found(sig: &syn::Signature, rel: &str, text: &str) -> FoundFn {
    let line = sig.ident.span().start().line;
    FoundFn {
        at: CodeMatch {
            file: rel.to_string(),
            line,
            text: source_line(text, line),
        },
        arity: sig.inputs.len(),
        returns: return_type(sig),
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

    fn resolve_in(tmp: &tempfile::TempDir, observable: &str, arity: usize) -> Resolution {
        resolve(
            tmp.path(),
            &tmp.path().join("ProvableRequirements"),
            observable,
            arity,
        )
    }

    // Verifies: REQ025 — a predicate resolves to a real function at a real location, which
    // is what a substring match could never establish.
    #[test]
    fn resolves_a_bool_function_to_its_source_location() {
        let tmp = subject("pub fn login(user: &str) -> bool { !user.is_empty() }\n");
        let r = resolve_in(&tmp, "login", 1);
        let Resolution::Resolved(at) = r else {
            panic!("should resolve, got {r:?}")
        };
        assert_eq!(at.file, "src/auth.rs");
        assert_eq!(at.line, 1);
        assert!(at.text.contains("fn login"));
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

        let r = resolve(tmp.path(), &companion, "login", 1);
        let Resolution::Resolved(at) = &r else {
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
