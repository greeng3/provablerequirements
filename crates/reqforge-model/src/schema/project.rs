//! Project configuration — per `FORMAT-projectConfigSchema`.
//!
//! Found at the Project-repo root as `reqforge.json`.

use serde::{Deserialize, Serialize};

use crate::schema::Overflow;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    pub schema_version: u32,
    pub slug: String,
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Repository-relative override for the Collections root
    /// (defaults to `artifacts` when unset; see
    /// `FORMAT-collectionsRootPath`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts_path: Option<String>,

    /// Source paths for the code-trace scanner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_paths: Option<Vec<String>>,

    #[serde(flatten)]
    pub overflow: Overflow,
}

impl ProjectConfig {
    /// The effective Collections-root path, applying the default.
    pub fn effective_artifacts_path(&self) -> &str {
        self.artifacts_path.as_deref().unwrap_or("artifacts")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_minimal_project_config() {
        let raw = json!({
            "schemaVersion": 1,
            "slug": "sample-project",
            "name": "Sample Project"
        });
        let cfg: ProjectConfig = serde_json::from_value(raw).unwrap();
        assert_eq!(cfg.slug, "sample-project");
        assert_eq!(cfg.effective_artifacts_path(), "artifacts");
    }

    #[test]
    fn respects_artifacts_path_override() {
        let raw = json!({
            "schemaVersion": 1,
            "slug": "p",
            "name": "P",
            "artifactsPath": "docs/reqs"
        });
        let cfg: ProjectConfig = serde_json::from_value(raw).unwrap();
        assert_eq!(cfg.effective_artifacts_path(), "docs/reqs");
    }

    #[test]
    fn preserves_unknown_fields() {
        let raw = json!({
            "schemaVersion": 1,
            "slug": "p",
            "name": "P",
            "experimental": { "deepLink": true }
        });
        let cfg: ProjectConfig = serde_json::from_value(raw.clone()).unwrap();
        let round_tripped = serde_json::to_value(&cfg).unwrap();
        assert_eq!(round_tripped, raw);
    }

    #[test]
    fn missing_slug_fails() {
        let raw = json!({ "schemaVersion": 1, "name": "P" });
        let err = serde_json::from_value::<ProjectConfig>(raw).unwrap_err();
        assert!(err.to_string().contains("slug"));
    }
}
