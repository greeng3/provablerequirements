//! Structured gate errors and warnings. Errors carry a best-effort source line so the
//! generate-then-repair loop can quote precise, actionable feedback back to the LLM.
//! Warnings are the vacuity/triviality "verifies but meaningless" signals — the
//! requirement is well-formed but suspicious, and they ride through to the human (D12)
//! rather than driving repair.
//!
//! Implements: REQ016 (gate part 1 — parse + type/name-check), REQ017 (gate part 2 —
//! vacuity warnings).

use std::fmt;

/// A single reason a candidate PRL was rejected by the gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    /// The grammar did not accept the input at `line`.
    Parse { message: String, line: usize },
    /// A required section (`require`, or the `requirement <name>` header) is absent.
    MissingSection { section: &'static str },
    /// `require { }` parsed but holds no property — a requirement that claims nothing.
    EmptyRequire,
    /// A `category:` token was not one of `1 | 2a | 2b | 3`.
    BadCategory { value: String, line: usize },
    /// The same event/state/sort name was declared twice in `vocabulary`.
    DuplicateDecl { name: String, line: usize },
    /// A predicate used in `require` was never declared in `vocabulary`.
    UndeclaredPredicate { name: String, line: usize },
    /// A predicate was applied with the wrong number of arguments.
    ArityMismatch {
        name: String,
        expected: usize,
        found: usize,
        line: usize,
    },
}

impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GateError::Parse { message, line } => write!(f, "line {line}: {message}"),
            GateError::MissingSection { section } => {
                write!(f, "missing required `{section}`")
            }
            GateError::EmptyRequire => write!(f, "`require` block is empty — it claims nothing"),
            GateError::BadCategory { value, line } => {
                write!(
                    f,
                    "line {line}: category `{value}` is not one of 1, 2a, 2b, 3"
                )
            }
            GateError::DuplicateDecl { name, line } => {
                write!(
                    f,
                    "line {line}: `{name}` is declared more than once in vocabulary"
                )
            }
            GateError::UndeclaredPredicate { name, line } => write!(
                f,
                "line {line}: `{name}` is used in require but not declared in vocabulary"
            ),
            GateError::ArityMismatch {
                name,
                expected,
                found,
                line,
            } => write!(
                f,
                "line {line}: `{name}` takes {expected} argument(s) but was given {found}"
            ),
        }
    }
}

impl std::error::Error for GateError {}

/// A vacuity/triviality signal: the candidate is well-formed and well-typed, but this
/// part of it is trivially true, trivially unsatisfiable, or otherwise likely a
/// mistranslation. Not a rejection — a flag for the human gate (D12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateWarning {
    /// `P leads_to P` — a claim that leads to itself is vacuously true.
    SelfLeadsTo { line: usize },
    /// `P precedes P` — a claim that precedes itself is vacuous.
    SelfPrecedes { line: usize },
    /// A pattern operand of the form `P or not P` — always true, so the pattern is
    /// vacuously satisfied.
    ImmediateTautology { line: usize },
    /// A pattern operand of the form `P and not P` — never true, so the pattern can
    /// never be triggered.
    ImmediateContradiction { line: usize },
    /// `occurs at most 0 times` — means the event never occurs; likely wants `never`.
    OccursAtMostZero { line: usize },
    /// A vocabulary predicate that is never used in `require` — dead vocabulary, often
    /// a sign the translation dropped part of the claim.
    UnusedVocabulary { name: String, line: usize },
}

impl fmt::Display for GateWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GateWarning::SelfLeadsTo { line } => write!(
                f,
                "line {line}: `P leads_to P` leads to itself — vacuously true"
            ),
            GateWarning::SelfPrecedes { line } => write!(
                f,
                "line {line}: `P precedes P` precedes itself — vacuous"
            ),
            GateWarning::ImmediateTautology { line } => write!(
                f,
                "line {line}: an operand `P or not P` is always true — the pattern is vacuously satisfied"
            ),
            GateWarning::ImmediateContradiction { line } => write!(
                f,
                "line {line}: an operand `P and not P` is never true — the pattern can never be triggered"
            ),
            GateWarning::OccursAtMostZero { line } => write!(
                f,
                "line {line}: `occurs at most 0 times` means the event never occurs — use `never` if that is the intent"
            ),
            GateWarning::UnusedVocabulary { name, line } => write!(
                f,
                "line {line}: `{name}` is declared in vocabulary but never used in require"
            ),
        }
    }
}
