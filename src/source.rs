//! The `RequirementsSource` seam: provreq reaches requirement items only through
//! this abstraction, never off a specific tool's files (R-src-1). Doorstop is
//! adapter #1 (see [`crate::doorstop`]) and ReqForge is adapter #2 (see
//! [`crate::reqforge`]), which arrived with phase 1 of the absorb (#296) and
//! made this the two-implementation seam R-src-4 was waiting for. Which one a
//! subject uses is decided in exactly one place, [`crate::adopt::source_for`].
//!
//! Implements: REQ009 (read requirements through a source-agnostic seam)

use anyhow::Result;

/// The A2 triage buckets (README's provable / falsifiable / vague split).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Classification {
    /// Provable now against the code (a code-level verifier can discharge it).
    FormalizableNow,
    /// Only falsifiable — checkable from finite observations of a running system, never proved.
    /// Two engines can do that: a monitor reading a trace the subject wrote (#233), and a browser
    /// driven against a live deployment (#245). Named because the bucket predates both, and the
    /// second one is easy to forget when classifying a requirement about a user interface.
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
    /// Optional per-source prior for triage (ReqForge's `expectsCodeTrace: true`, and only where an
    /// artifact states it explicitly — see [`crate::reqforge`]); `None` for Doorstop, which has no
    /// equivalent. Advisory seed only (R-src-5).
    pub verification_hint: Option<Classification>,
    /// The source's own declaration of whether this requirement is expected to trace to code
    /// (ReqForge's `expectsCodeTrace`; `None` for Doorstop, which has no equivalent). An explicit
    /// `Some(false)` is real information that [`Classification`] deliberately cannot express: it
    /// rules `FormalizableNow` *out* — the source has declared this requirement is not expected to
    /// have a code-level implementation to verify — without choosing between `FalsifiableOnly` and
    /// `StaysProse`. Carried alongside `verification_hint` rather than folded into it so the bucket
    /// set stays a clean partition (R-src-7, #297); the classifiers honour it, and must never place
    /// an item its source ruled out into `FormalizableNow`.
    pub expects_code_trace: Option<bool>,
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

/// A fingerprint of an item's prose, used as the revision token when the source has no native one,
/// or when its native one answers a different question (R-src-3).
///
/// Shared by the adapters rather than owned by one: both Doorstop and ReqForge reach for it, and an
/// adapter borrowing it from a sibling adapter would couple two implementations of this seam that
/// are supposed to know nothing about each other.
///
/// SHA-256, because the token has to outlive the binary that wrote it. It is persisted in
/// `drafts.yml` and in every verdict's `requirement_revision`, and compared on later runs — so a
/// hash that changed with the toolchain would stale every draft and report requirement drift on
/// every stored verdict, with no requirement having moved. That is the failure REQ071 fixed on the
/// code-drift axis, and it sat unfixed on the prose axis until #296. This used `DefaultHasher`,
/// whose own documentation declines to promise stability across Rust releases.
pub fn content_hash(text: &str) -> String {
    use sha2::Digest;
    // Hex by hand rather than through a formatting impl: what this returns is written to disk and
    // compared for the life of the record, so it should not move if the digest crate changes which
    // formatting traits it offers.
    sha2::Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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

    // Verifies: REQ009 / #296 — the revision token is a SHA-256 of the prose, pinned to a digest
    // computed outside this program. It has to survive a provreq upgrade: the token is written into
    // `drafts.yml` and into every verdict's `requirement_revision`, then compared on later runs, so
    // an algorithm that quietly changed would stale every draft and report requirement drift on
    // every stored verdict with no requirement having moved. `DefaultHasher`, which this used
    // before, is explicitly not guaranteed stable across Rust releases.
    #[test]
    fn the_revision_token_is_a_pinned_digest_not_a_build_local_hash() {
        assert_eq!(
            content_hash("the first item"),
            "c7646657f30409387673f0e1c4d90bb0139f91c8dd1f017fc3cd932294d585fc"
        );
        assert_ne!(
            content_hash("the first item"),
            content_hash("the second item")
        );
    }

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
