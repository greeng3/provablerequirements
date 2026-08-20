//! System configuration — per `FORMAT-systemConfigSchema`.

use serde::{Deserialize, Serialize};

use crate::schema::Overflow;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemConfig {
    pub schema_version: u32,
    pub name: String,
    pub projects: Vec<SystemProjectRef>,

    /// User-declared link types augmenting the built-in catalog
    /// (`TRACE-linkExtensibility`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub link_types: Vec<SystemLinkType>,

    /// User-declared scanner languages augmenting the built-in
    /// registry (`TRACE-codeLanguageRegistry`). The contents are
    /// opaque at load time; the scanner validates them lazily.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub languages: Option<serde_json::Value>,

    /// Priority-ordered LLM provider configurations
    /// (`LLM-configSchema`). Opaque at load time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<serde_json::Value>,

    #[serde(flatten)]
    pub overflow: Overflow,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemProjectRef {
    pub slug: String,

    #[serde(flatten)]
    pub overflow: Overflow,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemLinkType {
    pub name: String,
    pub inverse_name: String,
    pub directed: bool,
    pub acyclic: bool,

    #[serde(flatten)]
    pub overflow: Overflow,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_minimal_system_config() {
        let raw = json!({
            "schemaVersion": 1,
            "name": "My ReqForge System",
            "projects": [{ "slug": "sample-project" }]
        });
        let cfg: SystemConfig = serde_json::from_value(raw).unwrap();
        assert_eq!(cfg.name, "My ReqForge System");
        assert_eq!(cfg.projects.len(), 1);
        assert_eq!(cfg.projects[0].slug, "sample-project");
        assert!(cfg.link_types.is_empty());
    }

    #[test]
    fn deserializes_system_with_custom_link_types() {
        let raw = json!({
            "schemaVersion": 1,
            "name": "sys",
            "projects": [],
            "linkTypes": [
                {
                    "name": "mitigates",
                    "inverseName": "mitigated-by",
                    "directed": true,
                    "acyclic": false
                }
            ]
        });
        let cfg: SystemConfig = serde_json::from_value(raw).unwrap();
        assert_eq!(cfg.link_types.len(), 1);
        assert_eq!(cfg.link_types[0].name, "mitigates");
        assert_eq!(cfg.link_types[0].inverse_name, "mitigated-by");
        assert!(cfg.link_types[0].directed);
        assert!(!cfg.link_types[0].acyclic);
    }

    #[test]
    fn preserves_unknown_fields_and_opaque_blocks() {
        let raw = json!({
            "schemaVersion": 1,
            "name": "sys",
            "projects": [],
            "llm": [{ "provider": "openai", "model": "gpt-4o" }],
            "futureFeature": { "enabled": true }
        });
        let cfg: SystemConfig = serde_json::from_value(raw.clone()).unwrap();
        assert!(cfg.llm.is_some());
        let round_tripped = serde_json::to_value(&cfg).unwrap();
        assert_eq!(round_tripped, raw);
    }
}
