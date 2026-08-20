//! Link payload and hint types — per `FORMAT-linkPayloadSchema`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::Overflow;

/// A single outgoing link from one artifact to another.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    /// The target artifact's UUID.
    pub target_uuid: Uuid,

    /// Link-type name — must match a built-in type
    /// (`TRACE-linkCatalog`) or a System-declared type
    /// (`TRACE-linkExtensibility`). Validated at authoring time; at
    /// load time we accept any string so unknown types surface as
    /// diagnostic errors rather than data-loss parse failures.
    #[serde(rename = "type")]
    pub type_name: String,

    /// Best-effort human-readable pointer to the target.
    pub hint: LinkHint,

    /// Unknown-field overflow (`FORMAT-fieldTolerance`).
    #[serde(flatten)]
    pub overflow: Overflow,
}

/// The three hint fields required on every link entry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkHint {
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,

    #[serde(flatten)]
    pub overflow: Overflow,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_minimal_link() {
        let raw = json!({
            "targetUuid": "0194f6d0-0001-7000-8000-000000000002",
            "type": "derives-from",
            "hint": {
                "projectSlug": "sample-project",
                "collectionPrefix": "REQ",
                "artifactName": "root"
            }
        });
        let link: Link = serde_json::from_value(raw).unwrap();
        assert_eq!(link.type_name, "derives-from");
        assert_eq!(link.hint.project_slug, "sample-project");
        assert_eq!(link.hint.collection_prefix, "REQ");
        assert_eq!(link.hint.artifact_name, "root");
        assert!(link.overflow.is_empty());
    }

    #[test]
    fn preserves_unknown_fields_in_overflow() {
        let raw = json!({
            "targetUuid": "0194f6d0-0001-7000-8000-000000000002",
            "type": "satisfies",
            "hint": {
                "projectSlug": "p",
                "collectionPrefix": "REQ",
                "artifactName": "a",
                "futureField": "keep-me"
            },
            "experimental": { "someKey": 1 }
        });
        let link: Link = serde_json::from_value(raw.clone()).unwrap();
        let round_tripped = serde_json::to_value(&link).unwrap();
        assert_eq!(round_tripped, raw);
    }
}
