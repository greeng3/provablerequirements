//! The `RequirementsSource` seam: provreq reaches requirement items only through
//! this abstraction, never off a specific tool's files (R-src-1). Doorstop is
//! adapter #1 (see [`crate::doorstop`]); reqforge is the real second consumer
//! that will supply adapter #2, so the seam is drawn now and kept single-impl
//! until then (R-src-4).
//!
//! Implements: REQ009 (read requirements through a source-agnostic seam)

use anyhow::Result;

/// The A2 triage buckets (README's provable / falsifiable / vague split).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Classification {
    /// Provable now against the code (a code-level verifier can discharge it).
    FormalizableNow,
    /// Only falsifiable — checkable by a runtime monitor, not provable.
    FalsifiableOnly,
    /// Stays prose — too vague to formalize as written.
    StaysProse,
}

impl Classification {
    /// Parse a kebab-case bucket name (the CLI `--set` surface).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "formalizable-now" => Some(Self::FormalizableNow),
            "falsifiable-only" => Some(Self::FalsifiableOnly),
            "stays-prose" => Some(Self::StaysProse),
            _ => None,
        }
    }

    /// The kebab-case bucket name (round-trips with [`Classification::parse`]).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FormalizableNow => "formalizable-now",
            Self::FalsifiableOnly => "falsifiable-only",
            Self::StaysProse => "stays-prose",
        }
    }
}

/// One requirement item, source-agnostic. Its `text` is prose in every source
/// (R-src-2) — the untrusted natural-language input the formalize gate exists to
/// catch (A1). `id` is an opaque stable string the source owns; `revision` is the
/// source's native change token, or a content-hash of the prose when it has none
/// (R-src-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub text: String,
    pub revision: String,
    pub title: Option<String>,
    /// Optional per-source prior for triage (reqforge `expects_code_trace`);
    /// `None` for Doorstop. Advisory seed only (R-src-5).
    pub verification_hint: Option<Classification>,
}

/// The formalization provenance provreq stamps back onto a source item once a
/// formalization is admitted (D14, R-src-6): the confirmed PRL and who/when/which-tier
/// confirmed it, plus the source revision it was confirmed against so later NL drift is
/// detectable by anyone reading the item — not only via provreq's companion state.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Annotation {
    /// Lifecycle marker — `admitted-but-ungrounded` for now (grounding is D13).
    pub status: String,
    /// The confirmed PRL requirement block.
    pub prl: String,
    /// Review tier at admission (`mandatory` | `optional`).
    pub review: String,
    pub reviewer: String,
    pub reviewed_at_unix: i64,
    /// The source revision token the PRL was confirmed against (drift baseline).
    pub source_revision: String,
}

/// The requirements-source seam (R-src-1). One implementation for now
/// ([`crate::doorstop::DoorstopSource`]); the reqforge adapter is a real,
/// not-speculative second consumer that lands when its format stabilises.
pub trait RequirementsSource {
    /// Every requirement item in the source, sorted by `id`.
    fn items(&self) -> Result<Vec<Item>>;

    /// Write a formalization back-link onto item `id`, rendered in the source's native
    /// way (R-src-6) — for Doorstop, a `provreq:` attribute on the item file. Replaces
    /// any prior annotation. Mutates the subject working tree; the operator commits it.
    fn annotate(&self, id: &str, annotation: &Annotation) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_parse_round_trips() {
        for c in [
            Classification::FormalizableNow,
            Classification::FalsifiableOnly,
            Classification::StaysProse,
        ] {
            assert_eq!(Classification::parse(c.as_str()), Some(c));
        }
        assert_eq!(Classification::parse("nonsense"), None);
    }
}
