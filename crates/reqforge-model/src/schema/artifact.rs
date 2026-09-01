//! Artifact metadata — per `FORMAT-artifactMetadataSchema` and
//! `FORMAT-artifactShapeSpecificFields`.
//!
//! The on-disk JSON uses a single flat object whose `shape` discriminator
//! tells the consumer which of the shape-specific fields are meaningful.
//! That's mirrored here with a flat [`Artifact`] struct + a [`ArtifactShape`]
//! enum. Loaders can choose to validate shape-vs-field consistency after
//! deserialization; serde itself accepts any combination at the type level.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::{Link, Overflow, ReviewLogEntry};

/// The three artifact shapes ReqForge supports.
///
/// Implements: ART001
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactShape {
    Content,
    Blob,
    Url,
}

/// The full artifact metadata carried in JSON frontmatter (content)
/// or a `.reqforge.json` sidecar (blob / URL).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    // ---- Required common fields ----
    pub schema_version: u32,
    pub uuid: Uuid,
    pub title: String,
    pub shape: ArtifactShape,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,

    /// Typed outgoing links. Empty vec when the artifact has none.
    #[serde(default)]
    pub links: Vec<Link>,

    /// Review event history. Empty vec when the artifact has none.
    #[serde(default)]
    pub review_log: Vec<ReviewLogEntry>,

    // ---- Optional common fields ----
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expects_code_trace: Option<bool>,

    /// Active lifecycle flag; conceptual default is `true` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,

    /// Derived-from-external-source flag; conceptual default is `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_level: Option<String>,

    /// Importer-preserved opaque payload (`ART-legacyField`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy: Option<serde_json::Value>,

    // ---- Shape-specific fields (all optional at the type level;
    // loaders enforce shape-vs-field consistency). ----
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_path: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<DateTime<Utc>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_status: Option<String>,

    /// Unknown-field overflow (`FORMAT-fieldTolerance`).
    #[serde(flatten)]
    pub overflow: Overflow,
}

impl Artifact {
    /// Effective active flag (defaults to `true` when unset).
    pub fn is_active(&self) -> bool {
        self.active.unwrap_or(true)
    }

    /// Effective derived flag (defaults to `false` when unset).
    pub fn is_derived(&self) -> bool {
        self.derived.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_content_json() -> serde_json::Value {
        json!({
            "schemaVersion": 1,
            "uuid": "0194f6d0-0001-7000-8000-000000000001",
            "title": "Hello World requirement",
            "shape": "content",
            "createdAt": "2026-04-18T00:00:00Z",
            "modifiedAt": "2026-04-18T00:00:00Z",
            "description": "A trivial starter requirement.",
            "links": [],
            "reviewLog": []
        })
    }

    #[test]
    fn deserializes_content_artifact() {
        let artifact: Artifact = serde_json::from_value(sample_content_json()).unwrap();
        assert_eq!(artifact.schema_version, 1);
        assert_eq!(artifact.title, "Hello World requirement");
        assert!(matches!(artifact.shape, ArtifactShape::Content));
        assert!(artifact.is_active());
        assert!(!artifact.is_derived());
        assert!(artifact.blob_path.is_none());
        assert!(artifact.url.is_none());
    }

    #[test]
    fn deserializes_blob_artifact() {
        let raw = json!({
            "schemaVersion": 1,
            "uuid": "0194f6d0-0002-7000-8000-000000000001",
            "title": "Design doc",
            "shape": "blob",
            "createdAt": "2026-04-18T00:00:00Z",
            "modifiedAt": "2026-04-18T00:00:00Z",
            "blobPath": "artifacts/designs/DES-spec.pdf",
            "links": [],
            "reviewLog": []
        });
        let artifact: Artifact = serde_json::from_value(raw).unwrap();
        assert!(matches!(artifact.shape, ArtifactShape::Blob));
        assert_eq!(
            artifact.blob_path.as_deref(),
            Some("artifacts/designs/DES-spec.pdf")
        );
    }

    #[test]
    fn deserializes_url_artifact_with_check_metadata() {
        let raw = json!({
            "schemaVersion": 1,
            "uuid": "0194f6d0-0003-7000-8000-000000000001",
            "title": "RFC 9110",
            "shape": "url",
            "createdAt": "2026-04-18T00:00:00Z",
            "modifiedAt": "2026-04-18T00:00:00Z",
            "url": "https://www.rfc-editor.org/rfc/rfc9110.html",
            "checkedAt": "2026-04-17T12:00:00Z",
            "checkStatus": "ok",
            "links": [],
            "reviewLog": []
        });
        let artifact: Artifact = serde_json::from_value(raw).unwrap();
        assert!(matches!(artifact.shape, ArtifactShape::Url));
        assert_eq!(
            artifact.url.as_deref(),
            Some("https://www.rfc-editor.org/rfc/rfc9110.html")
        );
        assert_eq!(artifact.check_status.as_deref(), Some("ok"));
    }

    #[test]
    fn missing_required_field_fails_cleanly() {
        let mut raw = sample_content_json();
        raw.as_object_mut().unwrap().remove("uuid");
        let err = serde_json::from_value::<Artifact>(raw).unwrap_err();
        assert!(
            err.to_string().contains("uuid"),
            "expected uuid-related error, got: {err}"
        );
    }

    #[test]
    fn unknown_fields_round_trip_via_overflow() {
        let raw = json!({
            "schemaVersion": 1,
            "uuid": "0194f6d0-0001-7000-8000-000000000001",
            "title": "x",
            "shape": "content",
            "createdAt": "2026-04-18T00:00:00Z",
            "modifiedAt": "2026-04-18T00:00:00Z",
            "links": [],
            "reviewLog": [],
            "futureFeature": { "enabled": true, "note": "keep me" }
        });
        let artifact: Artifact = serde_json::from_value(raw.clone()).unwrap();
        assert!(artifact.overflow.contains_key("futureFeature"));
        let round_tripped = serde_json::to_value(&artifact).unwrap();
        assert_eq!(round_tripped, raw);
    }

    #[test]
    fn defaults_links_and_review_log_when_missing() {
        let raw = json!({
            "schemaVersion": 1,
            "uuid": "0194f6d0-0001-7000-8000-000000000001",
            "title": "x",
            "shape": "content",
            "createdAt": "2026-04-18T00:00:00Z",
            "modifiedAt": "2026-04-18T00:00:00Z"
        });
        let artifact: Artifact = serde_json::from_value(raw).unwrap();
        assert!(artifact.links.is_empty());
        assert!(artifact.review_log.is_empty());
    }

    #[test]
    fn is_active_defaults_true_is_derived_defaults_false() {
        let raw = sample_content_json();
        let artifact: Artifact = serde_json::from_value(raw).unwrap();
        assert!(artifact.is_active());
        assert!(!artifact.is_derived());
    }
}
