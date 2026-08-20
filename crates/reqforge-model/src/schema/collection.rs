//! Collection configuration — per `FORMAT-collectionConfigSchema`.
//!
//! Found at the Collection-directory root as `.collection.json`.

use serde::{Deserialize, Serialize};

use crate::schema::Overflow;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionConfig {
    pub schema_version: u32,
    pub prefix: String,
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Per `TRACE-codeCoverageExpectation`, conceptual default is
    /// `true` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expects_code_trace: Option<bool>,

    /// Opaque importer payload (for example, `{"doorstopParent":
    /// "REQ"}` from a doorstop import).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_notes: Option<serde_json::Value>,

    #[serde(flatten)]
    pub overflow: Overflow,
}

impl CollectionConfig {
    /// Effective `expectsCodeTrace` (conceptual default: `true`).
    pub fn effective_expects_code_trace(&self) -> bool {
        self.expects_code_trace.unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_minimal_collection_config() {
        let raw = json!({
            "schemaVersion": 1,
            "prefix": "REQ",
            "name": "Requirements"
        });
        let cfg: CollectionConfig = serde_json::from_value(raw).unwrap();
        assert_eq!(cfg.prefix, "REQ");
        assert!(cfg.effective_expects_code_trace());
    }

    #[test]
    fn respects_expects_code_trace_override() {
        let raw = json!({
            "schemaVersion": 1,
            "prefix": "REQ",
            "name": "Requirements",
            "expectsCodeTrace": false
        });
        let cfg: CollectionConfig = serde_json::from_value(raw).unwrap();
        assert!(!cfg.effective_expects_code_trace());
    }

    #[test]
    fn preserves_import_notes_and_unknown_fields() {
        let raw = json!({
            "schemaVersion": 1,
            "prefix": "REQ",
            "name": "Requirements",
            "importNotes": { "doorstopParent": "SRS" },
            "experimentalFlag": true
        });
        let cfg: CollectionConfig = serde_json::from_value(raw.clone()).unwrap();
        assert!(cfg.import_notes.is_some());
        let round_tripped = serde_json::to_value(&cfg).unwrap();
        assert_eq!(round_tripped, raw);
    }
}
