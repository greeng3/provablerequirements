//! UUID → artifact-location index, built after Project discovery.
//!
//! Resolves a link's `targetUuid` back to the artifact's project /
//! collection / name triple without rewalking the filesystem.

use std::collections::HashMap;
use std::path::PathBuf;

use uuid::Uuid;

use crate::load::LoadedProject;

/// Where an artifact lives in the System hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactLocation {
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
    pub source_path: PathBuf,
}

/// Reports a UUID that appears in two or more artifacts. The first
/// one wins (keeps the index deterministic); later ones are listed
/// here so the UI can flag the conflict.
#[derive(Debug, Clone)]
pub struct DuplicateUuid {
    pub uuid: Uuid,
    pub first: ArtifactLocation,
    pub duplicate: ArtifactLocation,
}

/// The UUID → location map built from a set of loaded Projects.
#[derive(Debug, Default)]
pub struct UuidIndex {
    inner: HashMap<Uuid, ArtifactLocation>,
}

impl UuidIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up an artifact by UUID.
    pub fn get(&self, uuid: &Uuid) -> Option<&ArtifactLocation> {
        self.inner.get(uuid)
    }

    /// Number of indexed artifacts.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Iterate over `(uuid, location)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&Uuid, &ArtifactLocation)> {
        self.inner.iter()
    }
}

/// Build a UUID index over a slice of loaded Projects.
///
/// The second element of the return tuple lists any UUIDs that
/// appeared more than once; such collisions are a data-integrity
/// issue worth surfacing in the UI.
pub fn build_uuid_index(projects: &[&LoadedProject]) -> (UuidIndex, Vec<DuplicateUuid>) {
    let mut index = UuidIndex::new();
    let mut duplicates = Vec::new();

    for project in projects {
        for collection in &project.collections {
            for artifact in &collection.artifacts {
                let location = ArtifactLocation {
                    project_slug: project.config.slug.clone(),
                    collection_prefix: collection.config.prefix.clone(),
                    artifact_name: artifact.name.clone(),
                    source_path: artifact.source_path.clone(),
                };
                let uuid = artifact.metadata.uuid;
                if let Some(existing) = index.inner.get(&uuid) {
                    duplicates.push(DuplicateUuid {
                        uuid,
                        first: existing.clone(),
                        duplicate: location,
                    });
                } else {
                    index.inner.insert(uuid, location);
                }
            }
        }
    }

    (index, duplicates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::LoadedArtifact;
    use crate::schema::{Artifact, ArtifactShape};
    use chrono::Utc;
    use std::collections::BTreeMap;

    fn make_artifact(uuid: Uuid, name: &str) -> LoadedArtifact {
        LoadedArtifact {
            name: name.to_owned(),
            source_path: PathBuf::from(format!("/fake/{name}.md")),
            metadata: Artifact {
                schema_version: 1,
                uuid,
                title: name.to_owned(),
                shape: ArtifactShape::Content,
                created_at: Utc::now(),
                modified_at: Utc::now(),
                links: Vec::new(),
                review_log: Vec::new(),
                description: None,
                expects_code_trace: None,
                active: None,
                derived: None,
                tags: None,
                outline_level: None,
                legacy: None,
                blob_path: None,
                url: None,
                checked_at: None,
                check_status: None,
                overflow: BTreeMap::new(),
            },
            body: Some(String::new()),
            blob: None,
        }
    }

    fn make_project_with(artifacts: Vec<LoadedArtifact>) -> LoadedProject {
        use crate::load::LoadedCollection;
        use crate::schema::{CollectionConfig, ProjectConfig};
        LoadedProject {
            root: PathBuf::from("/fake/project"),
            config: ProjectConfig {
                schema_version: 1,
                slug: "p".to_owned(),
                name: "P".to_owned(),
                description: None,
                artifacts_path: None,
                scan_paths: None,
                overflow: BTreeMap::new(),
            },
            collections: vec![LoadedCollection {
                dir_name: "requirements".to_owned(),
                dir_path: PathBuf::from("/fake/project/artifacts/requirements"),
                config: CollectionConfig {
                    schema_version: 1,
                    prefix: "REQ".to_owned(),
                    name: "Requirements".to_owned(),
                    description: None,
                    expects_code_trace: None,
                    import_notes: None,
                    overflow: BTreeMap::new(),
                },
                artifacts,
            }],
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn empty_projects_yield_empty_index() {
        let (index, dups) = build_uuid_index(&[]);
        assert!(index.is_empty());
        assert!(dups.is_empty());
    }

    #[test]
    fn indexes_one_artifact_per_project() {
        let u = Uuid::now_v7();
        let project = make_project_with(vec![make_artifact(u, "REQ-a")]);
        let (index, dups) = build_uuid_index(&[&project]);
        assert_eq!(index.len(), 1);
        assert!(dups.is_empty());
        let location = index.get(&u).unwrap();
        assert_eq!(location.project_slug, "p");
        assert_eq!(location.collection_prefix, "REQ");
        assert_eq!(location.artifact_name, "REQ-a");
    }

    #[test]
    fn reports_duplicate_uuids_but_keeps_first_seen() {
        let u = Uuid::now_v7();
        let project_one = make_project_with(vec![make_artifact(u, "REQ-first")]);
        let project_two = make_project_with(vec![make_artifact(u, "REQ-second")]);
        let (index, dups) = build_uuid_index(&[&project_one, &project_two]);
        assert_eq!(index.len(), 1);
        assert_eq!(index.get(&u).unwrap().artifact_name, "REQ-first");
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].duplicate.artifact_name, "REQ-second");
    }

    #[test]
    fn lookup_returns_none_for_unknown_uuid() {
        let project = make_project_with(vec![make_artifact(Uuid::now_v7(), "REQ-a")]);
        let (index, _) = build_uuid_index(&[&project]);
        assert!(index.get(&Uuid::now_v7()).is_none());
    }
}
