//! The category-2a adapter: resolve a PRL vocabulary symbol to **a definition in the
//! subject's TLA+ spec** — the shape the design's Adapters list requires for the model world
//! ("2a (model) a direct model variable/action reference"), and the shape a model checker
//! can consume.
//!
//! The observable world here is a **model**, not the subject's own code: category 1 resolves
//! against the subject's Rust ([`crate::rust_adapter`]); category 2a resolves against a TLA+
//! spec the operator wrote to model the system. Both are per-observable-world adapters
//! (R-eng-4); this one owns TLA+, and [`crate::grounding`] owns the category-independent
//! schema and verdict.
//!
//! **One resolver, because TLA+ has one kind of name.** Category 1 keeps predicates
//! (→ functions) and sorts (→ types) apart, because Rust makes them syntactically distinct
//! and a `struct login` must never satisfy the predicate `login`. TLA+ draws no such
//! line: an action `Accept(m) == …`, a state operator `Succeeded(m) == …`, a data set
//! `Message == 1..N`, a `VARIABLE status`, and a `CONSTANT MaxLen` are all just *named
//! definitions*. So a 2a binding resolves by one question — does the spec define this name? —
//! which is both smaller than cat-1's split and more faithful to the language.
//!
//! **Structural extraction, not SANY.** There is no TLA+ parser crate the way `syn` parses
//! Rust, so this reads the definitions a spec declares (`VARIABLES`/`CONSTANTS` declarations
//! and top-level operator definitions) structurally. That limit is real — a name introduced
//! by `LET`/`INSTANCE`, or a multi-line declaration, is not seen — and [`ModelResolution::describe`]
//! states it in the operator's read-back rather than letting a resolved binding imply more
//! than was checked, exactly as the Rust adapter is honest that `syn` sees no types.
//!
//! **Existence, and arity wherever the spec states one.** Existence was once the whole check,
//! on the reasoning that arity belonged to the engine (as instantiability did for cat-1 sorts,
//! REQ026). The first live cat-2a run refuted that: a predicate bound to a name of the wrong
//! arity grounded green, reached TLC, and came back as an `inconclusive` pointing into a
//! generated module provreq had already deleted — a verdict the operator cannot act on, for a
//! mistake sitting in their own binding (#119). A TLA+ operator does have an arity by
//! definition (`Op(a, b) == …`), and the declaration line is already captured in
//! [`SpecMatch::text`], so asking costs no second walk. Return *shape* is still the engine's
//! question and stays deferred. Where a line states no arity, this says nothing rather than
//! guessing — silence keeps a working binding, and a false park does not.
//!
//! Implements: REQ028 (a cat-2a binding resolves to a definition in a TLA+ spec, at the arity
//! the requirement uses it with).

use std::path::Path;
use walkdir::{DirEntry, WalkDir};

/// Where a definition lives in the subject's model: file (relative to the subject root),
/// 1-based line, and that line's own text — so the operator confirms against the real spec
/// rather than a definition this tool reconstructed. Peer of [`crate::rust_adapter::CodeMatch`];
/// kept separate so the two adapters stay independent (a third observable world earns a
/// shared type, not before).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecMatch {
    pub file: String,
    pub line: usize,
    pub text: String,
}

/// What resolving one cat-2a binding against the subject's TLA+ found. Still fewer variants
/// than [`crate::rust_adapter::Resolution`] — return shape is a Rust-type question that does
/// not arise for a bare TLA+ name — but arity is not among the ones that fail to arise, and
/// this carried no variant for it until a live run showed what that cost (#119).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelResolution {
    /// Exactly one definition of that name in the subject's TLA+, taking the number of
    /// arguments the requirement applies it to. The only variant that grounds.
    Resolved(SpecMatch),
    /// No definition of that name anywhere in the subject's TLA+.
    NotFound,
    /// Several definitions share the name. Never guessed between — the operator must
    /// disambiguate, because picking one silently would bind the requirement to whichever
    /// spec was walked first. Decided before arity: until it is known *which* definition this
    /// is, there is no arity to be right or wrong about.
    Ambiguous(Vec<SpecMatch>),
    /// One definition, but it does not take the number of arguments the requirement applies
    /// the symbol to. Refused here rather than left to TLC, which would answer with a location
    /// inside a generated module that no longer exists by the time the operator reads it.
    WrongArity {
        at: SpecMatch,
        /// What the definition takes, read from its own declaration line.
        declared: usize,
        /// What the requirement applies the symbol to.
        expected: usize,
    },
}

impl ModelResolution {
    /// Whether this binding resolved — the single question [`crate::grounding::verdict`]
    /// asks. Only [`ModelResolution::Resolved`] grounds; everything else parks the
    /// requirement (R-ground-1).
    pub fn is_resolved(&self) -> bool {
        matches!(self, ModelResolution::Resolved(_))
    }

    /// The operator-facing read-back for one binding (D13: "here is what your binding
    /// resolves to — is that what you meant?"). A resolved definition names the limit of what
    /// was checked, so a green line never implies more than was done — and since the check now
    /// varies with the line it read, so does the claim: a definition whose arity was confirmed
    /// says so, and one whose line states no arity does not pretend otherwise.
    pub fn describe(&self, symbol: &str, observable: &str) -> String {
        match self {
            ModelResolution::Resolved(at) => {
                // What the read-back claims tracks what was actually done: a line stating no
                // arity is not checked for one, and must not be reported as though it were.
                let checked = match declared_arity(&at.text) {
                    Some(_) => "existence and arity",
                    None => "existence only",
                };
                format!(
                    "{symbol} → `{observable}` resolves to {}:{}  {}\n      ({checked} — a \
                     structural read of the spec, so return shape and names introduced by \
                     LET/INSTANCE are not checked here)",
                    at.file, at.line, at.text
                )
            }
            ModelResolution::WrongArity {
                at,
                declared,
                expected,
            } => format!(
                "{symbol}: `{observable}` is defined at {}:{}  {} — but it takes {}, and the \
                 requirement applies `{symbol}` to {}. TLC would reject the generated spec \
                 instead of checking it, so this is refused here, where the binding is",
                at.file,
                at.line,
                at.text,
                arguments(*declared),
                arguments(*expected)
            ),
            ModelResolution::NotFound => format!(
                "{symbol}: no definition `{observable}` in the subject's TLA+ — the model \
                 does not name it, so nothing observes it"
            ),
            ModelResolution::Ambiguous(ats) => {
                let places = ats
                    .iter()
                    .map(|a| format!("{}:{}", a.file, a.line))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{symbol}: `{observable}` is ambiguous — {} definitions share the name \
                     ({places}); qualify it, because binding to one silently would pick \
                     whichever spec was walked first",
                    ats.len()
                )
            }
        }
    }
}

/// Resolve a PRL symbol to a definition named `observable` in the subject's TLA+, taking the
/// number of arguments the requirement applies it to (REQ028). Read-only over the subject and
/// recomputed live — the model moves under a binding exactly as code and prose do, so a
/// resolution is never stored.
///
/// `expected_arity` is the **requirement's** claim, computed by [`crate::grounding`] from the
/// vocabulary, never read from the subject — the same division of labour as cat-1's expected
/// parameter types: the adapter reads what the operator wrote in the spec, and is told what the
/// requirement asks of it.
pub fn resolve(subject: &SubjectSpecs, observable: &str, expected_arity: usize) -> ModelResolution {
    let name = observable.trim();
    if name.is_empty() {
        return ModelResolution::NotFound;
    }
    let found = find_definitions(subject, name);
    match found.len() {
        0 => ModelResolution::NotFound,
        1 => {
            let at = found.into_iter().next().expect("len checked");
            match declared_arity(&at.text) {
                Some(declared) if declared != expected_arity => ModelResolution::WrongArity {
                    at,
                    declared,
                    expected: expected_arity,
                },
                // Either the arities agree, or the line states none — and a line that states
                // none is not evidence of a mismatch. Parking on a guess would cost the
                // operator a working binding.
                _ => ModelResolution::Resolved(at),
            }
        }
        _ => ModelResolution::Ambiguous(found),
    }
}

/// A count of arguments in the operator's words, so a reason reads as a sentence rather than as
/// a number the reader has to inflect.
fn arguments(n: usize) -> String {
    match n {
        0 => "no arguments".to_string(),
        1 => "1 argument".to_string(),
        n => format!("{n} arguments"),
    }
}

/// How many arguments the definition on `line` takes, or `None` when the line does not state it.
///
/// A declaration (`VARIABLES queue, status`, `CONSTANT MaxLen`) names a value, so it takes none.
/// An operator definition takes what its parameter list holds: none for `Op == …`, two for
/// `Op(a, b) == …`.
///
/// A **function** definition takes none *as an operator*: `Double[x \in Nat] == …` binds
/// `Double` to a function value, which TLA+ applies as `Double[x]` and never as `Double(x)`.
/// Confirmed against real TLC rather than reasoned about — asked to check `Double(n)`, it
/// answers `The operator Double requires 0 arguments.` So provreq, which can only ever emit the
/// `Op(args)` form, is right to refuse a function bound to a predicate that takes arguments.
///
/// `None` where the line cannot be read honestly — an unbalanced parameter list, or an `==` that
/// turns out to sit inside one. A multi-line definition never reaches here at all: neither half
/// of it satisfies [`defines_name`], so it does not resolve in the first place.
fn declared_arity(line: &str) -> Option<usize> {
    let line = strip_comment(line).trim_start();
    if declaration_names(line).is_some() {
        return Some(0);
    }
    let (head, _) = line.split_once("==")?;
    // No operator parameter list: `Op == …`, and `Double[x \in Nat] == …` alike.
    let Some((_, params)) = head.trim().split_once('(') else {
        return Some(0);
    };
    count_params(params.trim().strip_suffix(')')?)
}

/// The number of top-level arguments in a parameter list, or `None` if its brackets do not
/// balance. Depth-aware so a higher-order parameter (`Op(f(_), x) == …`) counts as one argument
/// rather than as its own commas.
fn count_params(params: &str) -> Option<usize> {
    if params.trim().is_empty() {
        return Some(0);
    }
    let mut depth: usize = 0;
    let mut count = 1;
    for c in params.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    (depth == 0).then_some(count)
}

/// Whether an entry is skipped by the walk: the companion tree (whose own files could hold a
/// spurious self-hit), or anything [`crate::subject_tree`] excludes. Shares those rules with the
/// Rust adapter, so the two observable worlds cannot disagree about which of the subject's files
/// count — including *how* they are asked, which is what #294 was: both adapters put files through
/// the directory rule, and neither of the other two walks did.
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

/// One spec file, read once: how it is named back to the operator, its text, and whether it lives
/// outside the subject tree. The last of those is not a detail — an external spec is not covered by
/// the subject's commit, so provenance has to account for it separately (#120).
struct Spec {
    label: String,
    text: String,
    external: bool,
}

/// Every `.tla` file the subject's model is made of, walked and read **once** — the model-side peer
/// of [`crate::rust_adapter::ParsedSubject`], and held by the caller for the same reason (#144): a
/// binding set used to re-walk the whole tree once per model symbol.
///
/// "The subject's model" is the subject tree plus whatever roots the operator configured
/// ([`crate::spec_paths`]).
pub struct SubjectSpecs {
    specs: Vec<Spec>,
}

impl SubjectSpecs {
    /// Walk and read the model's specs once: the subject tree, then each configured root.
    ///
    /// A file reached twice is kept once. A root configured *inside* the subject is a plausible
    /// thing to write, and without this it would make every definition in it resolve twice — a
    /// spurious [`ModelResolution::Ambiguous`] telling the operator to disambiguate between a file
    /// and itself.
    pub fn load(
        subject_root: &Path,
        companion_root: &Path,
        extra: &crate::spec_paths::SpecPaths,
    ) -> Self {
        let mut specs = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut walk = |root: &Path, external: bool| {
            for entry in WalkDir::new(root)
                .into_iter()
                .filter_entry(|e| !is_skipped(e, companion_root))
            {
                let Ok(entry) = entry else { continue };
                if !entry.file_type().is_file()
                    || entry.path().extension().is_none_or(|x| x != "tla")
                {
                    continue;
                }
                let identity = std::fs::canonicalize(entry.path())
                    .unwrap_or_else(|_| entry.path().to_path_buf());
                if !seen.insert(identity) {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(entry.path()) else {
                    continue;
                };
                // An in-tree spec is named relative to the subject, as it always was. An external
                // one is named by its absolute path — which is both what the operator needs in
                // order to open it, and what keeps `subject_root.join(label)` correct, since
                // joining an absolute path yields that path.
                let label = if external {
                    entry.path().display().to_string()
                } else {
                    entry
                        .path()
                        .strip_prefix(subject_root)
                        .unwrap_or(entry.path())
                        .display()
                        .to_string()
                };
                specs.push(Spec {
                    label,
                    text,
                    external,
                });
            }
        };
        walk(subject_root, false);
        for root in extra.roots() {
            walk(root, true);
        }
        Self { specs }
    }

    /// A fingerprint of every spec that lives **outside** the subject tree, or `None` when there
    /// are none.
    ///
    /// This exists because the subject's commit does not cover them. Without it, a verdict proved
    /// against a spec in a sibling repo would go on reading `fresh` while the model it was proved
    /// about moved underneath it — the living loop blind to the very artifact the verdict is about.
    ///
    /// `None` for the in-tree case is deliberate rather than lazy: those specs *are* covered by the
    /// subject commit, and a second axis saying the same thing would flag drift twice. It also
    /// means a subject that configured nothing carries no new axis at all.
    pub fn external_fingerprint(&self) -> Option<String> {
        use std::hash::{Hash, Hasher};
        let mut external: Vec<(&str, &str)> = self
            .specs
            .iter()
            .filter(|s| s.external)
            .map(|s| (s.label.as_str(), s.text.as_str()))
            .collect();
        if external.is_empty() {
            return None;
        }
        // Sorted, because walk order is the filesystem's business and a verdict must not go stale
        // just because a directory was read in a different order.
        external.sort_unstable();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        external.hash(&mut hasher);
        Some(format!("{:016x}", hasher.finish()))
    }
}

/// The current fingerprint of the model's out-of-subject specs, for the drift anchor (#120).
///
/// Returns before walking anything when no extra root is configured — which is every subject that
/// keeps its specs in-tree, so the living-loop surfaces that call this per request pay one manifest
/// read and no directory walk, exactly as they did before this axis existed.
pub fn current_external_fingerprint(subject_root: &Path, companion_root: &Path) -> Option<String> {
    let paths = crate::spec_paths::SpecPaths::load(subject_root, companion_root);
    if paths.is_empty() {
        return None;
    }
    SubjectSpecs::load(subject_root, companion_root, &paths).external_fingerprint()
}

/// Every TLA+ definition named `name` across the model's `.tla` files.
/// Every name the model **declares as a `CONSTANT`**, across all its specs.
///
/// The one question [`crate::tlc::Constants`] needs answered before it writes an assignment: TLC
/// silently ignores a `CONSTANT X = …` for a name the spec does not declare, so an assignment
/// provreq passes through unchecked lands on the verdict as part of a model that never included it
/// (#211). Reading this here keeps the division of labour intact — the adapter reads what the
/// operator wrote in the spec, and the engine decides what to do about it.
///
/// A `CONSTANT` declaration specifically, not any definition: assigning an operator definition is a
/// different thing (TLA+ spells it `Op <- Impl`) and provreq does not write it.
///
/// Existence across the model, the same reading as every other model lookup: a name declared by
/// *some* spec in the subject counts, even when the module being checked does not reach it. A
/// stricter answer would need the `EXTENDS` closure of the checked module, which provreq does not
/// compute, and the loose direction is the safe one — it can miss a mis-assignment, never invent
/// one.
pub fn declared_constants(subject: &SubjectSpecs) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for spec in &subject.specs {
        for raw in spec.text.lines() {
            let line = strip_comment(raw).trim_start();
            let Some(rest) = constant_declaration_names(line) else {
                continue;
            };
            out.extend(rest.split(',').filter_map(identifier).map(str::to_string));
        }
    }
    out
}

/// The declared names after a `CONSTANT(S)` keyword — [`declaration_names`] narrowed to the
/// constant half, since a `VARIABLE` is not something a `.cfg` may assign.
fn constant_declaration_names(line: &str) -> Option<&str> {
    for kw in ["CONSTANTS", "CONSTANT"] {
        if let Some(rest) = line.strip_prefix(kw)
            && rest.starts_with(|c: char| c.is_whitespace())
        {
            return Some(rest);
        }
    }
    None
}

fn find_definitions(subject: &SubjectSpecs, name: &str) -> Vec<SpecMatch> {
    let mut out = Vec::new();
    for spec in &subject.specs {
        collect_definitions(&spec.text, name, &spec.label, &mut out);
    }
    out
}

/// Scan one spec's text for a definition named `name`. Comments are stripped first so a name
/// mentioned in prose is never mistaken for a definition.
fn collect_definitions(text: &str, name: &str, rel: &str, out: &mut Vec<SpecMatch>) {
    for (idx, raw) in text.lines().enumerate() {
        let line = strip_comment(raw);
        if defines_name(line, name) {
            out.push(SpecMatch {
                file: rel.to_string(),
                line: idx + 1,
                text: raw.trim().to_string(),
            });
        }
    }
}

/// Drop a `\*` line comment, so `x == 1  \* not Accept` is read as `x == 1`. Block comments
/// `(* … *)` are a documented gap (see the module docs); a name buried only inside one is not
/// resolved, which errs toward NotFound, never toward a false resolve.
fn strip_comment(line: &str) -> &str {
    match line.find("\\*") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Whether `line` declares or defines `name`: a `VARIABLE(S)`/`CONSTANT(S)` entry, or an
/// operator definition `name == …` / `name(args) == …` / `name[x \in S] == …`.
fn defines_name(line: &str, name: &str) -> bool {
    let line = line.trim_start();
    if let Some(rest) = declaration_names(line) {
        return rest.split(',').any(|tok| identifier(tok) == Some(name));
    }
    operator_name(line) == Some(name)
}

/// The declared names after a `VARIABLE(S)`/`CONSTANT(S)` keyword, as raw comma-separated
/// text; `None` when the line is not such a declaration.
fn declaration_names(line: &str) -> Option<&str> {
    for kw in ["VARIABLES", "VARIABLE", "CONSTANTS", "CONSTANT"] {
        if let Some(rest) = line.strip_prefix(kw) {
            // The keyword must be a whole word, not a prefix of a longer identifier.
            if rest.starts_with(|c: char| c.is_whitespace()) {
                return Some(rest);
            }
        }
    }
    None
}

/// The operator name a `name … ==` definition introduces, or `None` when the line is not an
/// operator definition. Handles the plain, applied (`(args)`), and function (`[x \in S]`)
/// forms; an infix-operator definition (`a \oplus b == …`) is a documented gap.
fn operator_name(line: &str) -> Option<&str> {
    let (head, _) = line.split_once("==")?;
    // Everything left of `==`, minus an argument list or function-domain suffix, must be a
    // single identifier — otherwise it is an expression that merely contains `==`, not a
    // definition.
    let head = head.trim();
    let name_part = head
        .split_once('(')
        .map(|(n, _)| n)
        .or_else(|| head.split_once('[').map(|(n, _)| n))
        .unwrap_or(head)
        .trim();
    identifier(name_part)
}

/// `Some(tok)` when the trimmed token is exactly one TLA+ identifier (a letter followed by
/// letters/digits/underscores), else `None`. This is what keeps `x + y` or `Foo.bar` from
/// reading as a name.
fn identifier(tok: &str) -> Option<&str> {
    let tok = tok.trim();
    let mut chars = tok.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if tok.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Some(tok)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A subject tree with `spec.tla` holding `src`, plus a companion dir the walk skips.
    fn subject(src: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("spec.tla"), src).unwrap();
        tmp
    }

    /// Resolve `observable` as a symbol the requirement applies to `arity` arguments. Every
    /// call states that number, because since #119 it is half of what resolution decides.
    fn resolve_in(tmp: &tempfile::TempDir, observable: &str, arity: usize) -> ModelResolution {
        resolve(
            &SubjectSpecs::load(
                tmp.path(),
                &tmp.path().join("ProvableRequirements"),
                &crate::spec_paths::SpecPaths::default(),
            ),
            observable,
            arity,
        )
    }

    // Verifies: REQ060 / #292 — the peer of the Rust adapter's case, because this walk has the same
    // shape and the same hole: a `._Spec.tla` sidecar carries the extension of the file it shadows,
    // so `extension() != "tla"` admits it, and only the shared rule keeps it out. Written second,
    // after covering one adapter and calling the mechanism pinned — two walks sharing one rule need
    // the rule proven at both, which is the whole reason `subject_tree` exists.
    #[test]
    fn mac_resource_files_never_become_model_specs() {
        let tmp = subject(SPEC);
        std::fs::write(tmp.path().join("._spec.tla"), SPEC).unwrap();
        std::fs::write(tmp.path().join(".DS_Store"), SPEC).unwrap();

        let specs = SubjectSpecs::load(
            tmp.path(),
            &tmp.path().join("ProvableRequirements"),
            &crate::spec_paths::SpecPaths::default(),
        );
        let read: Vec<&str> = specs.specs.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(read, ["spec.tla"], "only the authored spec is the model");

        // The sidecar declares the same operator, so admitting it would be an ambiguity against a
        // file nobody wrote rather than a harmless extra read.
        assert!(resolve_in(&tmp, "Succeeded", 1).is_resolved());
    }

    const SPEC: &str = "\
---- MODULE Msg ----
EXTENDS Naturals
CONSTANT MaxLen
VARIABLES queue, status

Accept(m) == queue' = Append(queue, m)
Succeeded(m) == status[m] = \"Succeeded\"
Message == 1..MaxLen
Init == queue = <<>>
====
";

    /// A subject and a spec directory beside it — the sibling-repo layout #120 is about.
    fn subject_and_external(external_src: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let subject = tmp.path().join("subject");
        let models = tmp.path().join("models");
        std::fs::create_dir_all(&subject).expect("subject");
        std::fs::create_dir_all(&models).expect("models");
        std::fs::write(models.join("ext.tla"), external_src).expect("ext.tla");
        (tmp, models)
    }

    fn load_with(subject: &Path, roots: Vec<std::path::PathBuf>) -> SubjectSpecs {
        SubjectSpecs::load(
            subject,
            &subject.join("ProvableRequirements"),
            &crate::spec_paths::SpecPaths::from_roots(roots),
        )
    }

    // Verifies: REQ028 (#120) — a spec outside the subject tree resolves, which is the whole
    // point: this layout could not ground a category-2a requirement at all before.
    #[test]
    fn a_spec_outside_the_subject_resolves() {
        let (tmp, models) = subject_and_external("Accept(m) == TRUE\n");
        let subject = tmp.path().join("subject");
        let specs = load_with(&subject, vec![models.clone()]);

        let ModelResolution::Resolved(at) = resolve(&specs, "Accept", 1) else {
            panic!("an external definition must resolve");
        };
        // Named by absolute path, so the operator can open it — and so `subject.join(file)` still
        // lands on the real file, since joining an absolute path yields that path.
        assert_eq!(subject.join(&at.file), models.join("ext.tla"));
    }

    // Verifies: REQ028 (#120) — a root configured INSIDE the subject does not make every
    // definition in it resolve twice. Without deduplication the operator would be told to
    // disambiguate a file from itself.
    #[test]
    fn a_root_inside_the_subject_does_not_duplicate_its_specs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let inner = tmp.path().join("models");
        std::fs::create_dir_all(&inner).expect("models");
        std::fs::write(inner.join("m.tla"), "Accept(m) == TRUE\n").expect("m.tla");

        let specs = load_with(tmp.path(), vec![inner]);
        assert!(
            resolve(&specs, "Accept", 1).is_resolved(),
            "a doubly-walked file must still be one definition"
        );
    }

    // Verifies: REQ028 (#120) — the fingerprint covers the EXTERNAL specs only, and moves when one
    // of them does. This is the axis that keeps a verdict proved against an out-of-subject model
    // from reading `fresh` forever: the subject's commit cannot see that file change.
    #[test]
    fn the_fingerprint_covers_external_specs_and_moves_with_them() {
        let (tmp, models) = subject_and_external("Accept(m) == TRUE\n");
        let subject = tmp.path().join("subject");

        let before = load_with(&subject, vec![models.clone()])
            .external_fingerprint()
            .expect("external specs are fingerprinted");
        std::fs::write(models.join("ext.tla"), "Accept(m) == FALSE\n").expect("rewrite");
        let after = load_with(&subject, vec![models.clone()])
            .external_fingerprint()
            .expect("still fingerprinted");

        assert_ne!(before, after, "an external spec moving must be visible");
    }

    // Verifies: REQ028 (#120) — an in-tree subject carries NO fingerprint, so it gains no drift
    // axis at all. The subject's commit already covers those specs, and a second axis would flag
    // the same drift twice.
    #[test]
    fn an_in_tree_subject_has_no_external_fingerprint() {
        let tmp = subject(SPEC);
        assert_eq!(load_with(tmp.path(), vec![]).external_fingerprint(), None);
    }

    // Verifies: REQ028 (#120) — the fingerprint does not depend on walk order. Directory order is
    // the filesystem's business, and a verdict must not go stale because a directory was read in a
    // different sequence.
    #[test]
    fn the_fingerprint_is_independent_of_root_order() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let subject = tmp.path().join("subject");
        std::fs::create_dir_all(&subject).expect("subject");
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        for (dir, src) in [(&a, "One == 1\n"), (&b, "Two == 2\n")] {
            std::fs::create_dir_all(dir).expect("dir");
            std::fs::write(dir.join("s.tla"), src).expect("spec");
        }
        assert_eq!(
            load_with(&subject, vec![a.clone(), b.clone()]).external_fingerprint(),
            load_with(&subject, vec![b, a]).external_fingerprint()
        );
    }

    // Verifies: REQ028 — an operator definition resolves to a real location in the spec,
    // which is what makes a 2a binding groundable against the model.
    #[test]
    fn resolves_an_operator_definition_to_its_location() {
        let tmp = subject(SPEC);
        let ModelResolution::Resolved(at) = resolve_in(&tmp, "Accept", 1) else {
            panic!("Accept should resolve");
        };
        assert_eq!(at.file, "spec.tla");
        assert_eq!(at.line, 6);
        assert!(at.text.contains("Accept(m) =="));
    }

    // Verifies: REQ028 — TLA+ has one kind of name, so a VARIABLE, a CONSTANT, and a data-set
    // operator all resolve through the same resolver. A cat-1-style predicate/sort split
    // would wrongly reject two of these.
    #[test]
    fn a_variable_a_constant_and_a_set_all_resolve() {
        let tmp = subject(SPEC);
        assert!(resolve_in(&tmp, "queue", 0).is_resolved(), "VARIABLE");
        assert!(
            resolve_in(&tmp, "status", 0).is_resolved(),
            "VARIABLE (2nd on the line)"
        );
        assert!(resolve_in(&tmp, "MaxLen", 0).is_resolved(), "CONSTANT");
        assert!(
            resolve_in(&tmp, "Message", 0).is_resolved(),
            "set-defining operator"
        );
    }

    // Verifies: REQ028/REQ029 (#211) — the model's CONSTANT declarations are readable on their
    // own, which is what lets a `.cfg` assignment be held to something. Constants only: a VARIABLE
    // is not assignable, and neither is an operator definition (TLA+ spells that substitution
    // `Op <- Impl`, which provreq does not write).
    #[test]
    fn the_models_declared_constants_are_readable_and_are_constants_only() {
        let tmp = subject(SPEC);
        let specs = SubjectSpecs::load(
            tmp.path(),
            &tmp.path().join("ProvableRequirements"),
            &crate::spec_paths::SpecPaths::default(),
        );
        let declared = declared_constants(&specs);
        assert!(declared.contains("MaxLen"), "{declared:?}");
        assert!(!declared.contains("queue"), "a VARIABLE is not assignable");
        assert!(
            !declared.contains("Message"),
            "a definition is not a CONSTANT"
        );
    }

    // Verifies: REQ029 (#211) — every name on a multi-name `CONSTANTS` line is declared, and a
    // name only mentioned in a comment is not. Both are the reads a refusal would get wrong in the
    // operator's face: a missed name refuses a correct assignment, a commented one accepts a
    // useless one.
    #[test]
    fn declared_constants_reads_whole_lines_and_ignores_comments() {
        let tmp = subject(
            "---- MODULE M ----\nCONSTANTS Drones, MaxAlt\n\\* CONSTANT Ghost\nCONSTANTinople == 1\n====\n",
        );
        let declared = declared_constants(&SubjectSpecs::load(
            tmp.path(),
            &tmp.path().join("ProvableRequirements"),
            &crate::spec_paths::SpecPaths::default(),
        ));
        assert!(
            declared.contains("Drones") && declared.contains("MaxAlt"),
            "{declared:?}"
        );
        assert!(!declared.contains("Ghost"), "a comment declares nothing");
        assert!(
            !declared.contains("CONSTANTinople"),
            "the keyword must be a whole word: {declared:?}"
        );
    }

    // Verifies: REQ028 — a name the spec does not define parks the requirement (R-ground-1),
    // never grounds on a coincidental text match.
    #[test]
    fn an_undefined_name_does_not_resolve() {
        let tmp = subject(SPEC);
        assert_eq!(resolve_in(&tmp, "Rejected", 1), ModelResolution::NotFound);
        assert!(
            resolve_in(&tmp, "Rejected", 1)
                .describe("rejected", "Rejected")
                .contains("does not name it")
        );
    }

    // Verifies: REQ028 — a name only mentioned inside a `\*` comment is not a definition.
    #[test]
    fn a_name_only_in_a_comment_does_not_resolve() {
        let tmp = subject("VARIABLES queue  \\* Accept is handled elsewhere\nInit == queue = 0\n");
        assert_eq!(resolve_in(&tmp, "Accept", 1), ModelResolution::NotFound);
    }

    // Verifies: REQ028 — a keyword that is only a prefix of a longer identifier is not a
    // declaration (`CONSTANTS` must not make `CONSTANThing` resolve, nor swallow a var named
    // `VARIABLEs_note`). Guards the whole-word check.
    #[test]
    fn a_keyword_prefix_is_not_a_declaration() {
        let tmp = subject("CONSTANTing == 1\nVARIABLESuspect == 2\n");
        // These are operator definitions named CONSTANTing / VARIABLESuspect, not decls.
        assert!(resolve_in(&tmp, "CONSTANTing", 0).is_resolved());
        assert_eq!(resolve_in(&tmp, "CONSTANT", 0), ModelResolution::NotFound);
        assert_eq!(resolve_in(&tmp, "ing", 0), ModelResolution::NotFound);
    }

    // Verifies: REQ028 — an expression that merely contains `==` (an equality inside a
    // definition's body) is not mistaken for a definition of some compound name.
    #[test]
    fn an_equality_expression_is_not_a_definition() {
        let tmp = subject("Inv == queue = 0 /\\ status = \"ok\"\n");
        assert!(resolve_in(&tmp, "Inv", 0).is_resolved());
        // `queue = 0 /\ status` is not a name — the body must not resolve as one.
        assert_eq!(resolve_in(&tmp, "queue", 0), ModelResolution::NotFound);
    }

    // Verifies: REQ028 — the function-definition form `Name[x \in S] == …` resolves, and takes
    // no arguments AS AN OPERATOR. `Double` is bound to a function value, applied `Double[x]`
    // and never `Double(x)`; provreq can only emit the `(…)` form. Confirmed against real TLC,
    // which answers `Double(n)` with "The operator Double requires 0 arguments" — so the
    // predicate that takes one is refused, and the set-like use of it grounds.
    #[test]
    fn a_function_definition_takes_no_operator_arguments() {
        let tmp = subject("q == [x \\in 1..3 |-> x * 2]\nDouble[x \\in Nat] == x + x\n");
        assert!(resolve_in(&tmp, "Double", 0).is_resolved());
        assert!(
            matches!(
                resolve_in(&tmp, "Double", 1),
                ModelResolution::WrongArity { declared: 0, .. }
            ),
            "a function is not an operator provreq can apply"
        );
    }

    // Verifies: REQ028 (#119) — THE CASE FROM THE LIVE RUN. A 1-ary predicate bound to a 0-ary
    // VARIABLE is refused at grounding. Before this, it ground green, reached TLC, and returned
    // an inconclusive naming a generated module that had already been deleted.
    #[test]
    fn a_predicate_bound_to_a_variable_that_takes_no_arguments_is_refused() {
        let tmp = subject(SPEC);
        let r = resolve_in(&tmp, "queue", 1);
        assert!(
            matches!(
                r,
                ModelResolution::WrongArity {
                    declared: 0,
                    expected: 1,
                    ..
                }
            ),
            "{r:?}"
        );
        assert!(!r.is_resolved(), "a wrong-arity binding must not ground");
        let said = r.describe("accepted", "queue");
        assert!(said.contains("takes no arguments"), "{said}");
        assert!(said.contains("to 1 argument"), "{said}");
        assert!(said.contains("spec.tla:4"), "the line is named: {said}");
    }

    // Verifies: REQ028 (#119) — the mismatch is caught in the other direction too. An operator
    // that takes an argument, bound to a symbol the requirement applies to none, would lower to
    // a bare name TLC rejects just as surely.
    #[test]
    fn an_operator_applied_to_too_few_arguments_is_refused() {
        let tmp = subject(SPEC);
        assert!(matches!(
            resolve_in(&tmp, "Accept", 0),
            ModelResolution::WrongArity {
                declared: 1,
                expected: 0,
                ..
            }
        ));
    }

    // Verifies: REQ028 (#119) — arity is counted, not merely detected, so a 2-ary operator
    // grounds at 2 and is refused at 1.
    #[test]
    fn a_multi_argument_operator_resolves_only_at_its_own_arity() {
        let tmp = subject("Between(a, b) == a < b\n");
        assert!(resolve_in(&tmp, "Between", 2).is_resolved());
        assert!(matches!(
            resolve_in(&tmp, "Between", 1),
            ModelResolution::WrongArity { declared: 2, .. }
        ));
    }

    // Verifies: REQ028 (#119) — a higher-order parameter is ONE argument, not its own commas.
    // Counting `Op(f(_), x)` as three would refuse a correct binding, which costs the operator
    // more than the check saves them.
    #[test]
    fn a_higher_order_parameter_counts_as_one_argument() {
        let tmp = subject("Apply(f(_), x) == f(x)\n");
        assert!(resolve_in(&tmp, "Apply", 2).is_resolved());
    }

    // Verifies: REQ028 (#119) — ambiguity is decided BEFORE arity. Until it is known which
    // definition the binding means, there is no arity to be right or wrong about, and reporting
    // one would send the operator to fix the wrong thing.
    #[test]
    fn ambiguity_is_reported_before_arity() {
        let tmp = subject("Accept(m) == TRUE\n");
        std::fs::write(tmp.path().join("other.tla"), "Accept == FALSE\n").unwrap();
        assert!(matches!(
            resolve_in(&tmp, "Accept", 7),
            ModelResolution::Ambiguous(_)
        ));
    }

    // Verifies: REQ028 (#119) — the read-back claims only what was checked. A line stating an
    // arity says "existence and arity"; one that does not still says "existence only", so a
    // green line never implies a check that did not happen.
    #[test]
    fn the_read_back_claims_only_what_was_checked() {
        let tmp = subject(SPEC);
        let said = resolve_in(&tmp, "Accept", 1).describe("accepted", "Accept");
        assert!(said.contains("existence and arity"), "{said}");
        assert!(
            !said.contains("arity/shape"),
            "the old blanket caveat: {said}"
        );
    }

    // Verifies: REQ028 (#119) — a line whose parameter list does not close states no arity, so
    // nothing is claimed about it and the binding stands on its other checks. Guessing here
    // would park a binding that may well be correct.
    #[test]
    fn an_unreadable_parameter_list_claims_no_arity() {
        assert_eq!(declared_arity("Op(a, b) == TRUE"), Some(2));
        assert_eq!(declared_arity("Op(a, b == TRUE"), None);
        assert_eq!(declared_arity("Op() == TRUE"), Some(0));
        assert_eq!(declared_arity("VARIABLES queue, status"), Some(0));
        assert_eq!(declared_arity("Message == 1..MaxLen"), Some(0));
    }

    // Verifies: REQ028 — two specs defining the same name are never silently disambiguated;
    // binding to one would depend on walk order, which is not this tool's call.
    #[test]
    fn duplicate_definitions_are_ambiguous_never_guessed() {
        let tmp = subject("Accept(m) == TRUE\n");
        std::fs::write(tmp.path().join("other.tla"), "Accept(m) == FALSE\n").unwrap();
        let ModelResolution::Ambiguous(ats) = resolve_in(&tmp, "Accept", 1) else {
            panic!("two definitions must be ambiguous");
        };
        assert_eq!(ats.len(), 2);
    }

    // Verifies: REQ028 — the walk skips the companion tree and `.git`, the same discipline as
    // the Rust adapter, so a stray spec there cannot create a spurious ambiguity.
    #[test]
    fn the_walk_skips_the_companion_and_git() {
        let tmp = subject("Accept(m) == TRUE\n");
        let companion = tmp.path().join("ProvableRequirements");
        std::fs::create_dir_all(&companion).unwrap();
        std::fs::write(companion.join("shadow.tla"), "Accept(m) == FALSE\n").unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        std::fs::write(tmp.path().join(".git/x.tla"), "Accept(m) == FALSE\n").unwrap();

        let ModelResolution::Resolved(at) = resolve(
            &SubjectSpecs::load(
                tmp.path(),
                &companion,
                &crate::spec_paths::SpecPaths::default(),
            ),
            "Accept",
            1,
        ) else {
            panic!("the companion/.git copies must not create an ambiguity");
        };
        assert_eq!(at.file, "spec.tla");
    }

    // Verifies: REQ028 — a non-`.tla` file is not searched; a model observable is a TLA+
    // definition, not any text that resembles one.
    #[test]
    fn non_tla_files_are_not_searched() {
        let tmp = subject("Accept(m) == TRUE\n");
        std::fs::write(tmp.path().join("README.md"), "Accept(m) == FALSE\n").unwrap();
        assert!(resolve_in(&tmp, "Accept", 1).is_resolved());
    }

    // Verifies: REQ028 — an empty observable resolves to nothing, guarding a degenerate
    // binding rather than matching the first definition it meets.
    #[test]
    fn empty_observable_resolves_to_nothing() {
        let tmp = subject(SPEC);
        assert_eq!(resolve_in(&tmp, "   ", 0), ModelResolution::NotFound);
    }
}
