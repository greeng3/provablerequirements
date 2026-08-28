//! Browse-by-type computation (Phase 7d). Pure fold over the
//! `World` snapshot — no IO — producing the prefix-keyed panes.

use std::collections::{BTreeMap, HashSet};

use crate::matrix::ReviewStateTag;
use crate::mount::MountState;
use crate::reports::Scope;
use crate::reviews::derive_review_state;
use crate::world::World;

use super::{BrowseArtifact, BrowseError, BrowsePane, BrowseQuery, BrowseResponse, ScopeDto};

/// Groups artifacts by Collection prefix, applies the filter
/// set, and emits one pane per distinct prefix sorted prefix-
/// ascending. Within a pane, artifacts sort case-insensitively
/// by title with a (project, name) tiebreak for stable output.
pub fn build_browse(
    scope: Scope,
    query: &BrowseQuery,
    world: &World,
) -> Result<BrowseResponse, BrowseError> {
    ensure_scope_exists(&scope, world)?;

    let tag_filter: Option<HashSet<String>> =
        csv_field(query.tags.as_deref()).map(|l| l.into_iter().collect());
    let review_filter = parse_review_state_filter(csv_field(query.review_state.as_deref()))?;
    let include_inactive = query.include_inactive.unwrap_or(false);

    // A BTreeMap keys by prefix so iteration yields panes in
    // prefix-ascending order without a separate sort pass.
    let mut by_prefix: BTreeMap<String, PaneBuilder> = BTreeMap::new();

    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        if !project_in_scope(&project.config.slug, &scope) {
            continue;
        }
        for collection in &project.collections {
            if !collection_in_scope(&collection.config.prefix, &scope) {
                continue;
            }
            let entry = by_prefix
                .entry(collection.config.prefix.clone())
                .or_default();
            // Record every Collection name observed for this
            // prefix — the name-variant surfacing fires when
            // the set has ≥ 2 distinct entries.
            entry.name_candidates.insert(collection.config.name.clone());

            for artifact in &collection.artifacts {
                if !include_inactive && !artifact.metadata.is_active() {
                    continue;
                }
                let tags: Vec<String> = artifact.metadata.tags.clone().unwrap_or_default();
                if let Some(wanted) = &tag_filter
                    && !tags.iter().any(|t| wanted.contains(t))
                {
                    continue;
                }
                let review = derive_review_state(&artifact.metadata.review_log);
                let review_tag = ReviewStateTag::from_derived(&review);
                if let Some(wanted) = &review_filter
                    && !wanted.contains(&review_tag)
                {
                    continue;
                }
                entry.artifacts.push(BrowseArtifact {
                    uuid: artifact.metadata.uuid,
                    project_slug: project.config.slug.clone(),
                    collection_prefix: collection.config.prefix.clone(),
                    artifact_name: artifact.name.clone(),
                    title: artifact.metadata.title.clone(),
                    shape: artifact.metadata.shape,
                    active: artifact.metadata.is_active(),
                    review_state: review_tag,
                    tags,
                });
            }
        }
    }

    let mut total_artifacts = 0usize;
    let mut panes: Vec<BrowsePane> = Vec::with_capacity(by_prefix.len());
    for (prefix, mut builder) in by_prefix {
        // Choose the lexicographically-first name as the
        // display label so the pick is stable regardless of
        // which project mounted first.
        let mut names: Vec<String> = builder.name_candidates.into_iter().collect();
        names.sort();
        let display = names.first().cloned().unwrap_or_else(|| prefix.clone());
        let name_variants: Option<Vec<String>> = if names.len() > 1 {
            Some(names.into_iter().skip(1).collect())
        } else {
            None
        };

        builder.artifacts.sort_by(|a, b| {
            let at = a.title.to_lowercase();
            let bt = b.title.to_lowercase();
            at.cmp(&bt)
                .then(a.project_slug.cmp(&b.project_slug))
                .then(a.artifact_name.cmp(&b.artifact_name))
        });

        let total = builder.artifacts.len();
        total_artifacts += total;
        panes.push(BrowsePane {
            prefix,
            name: display,
            name_variants,
            total_artifacts: total,
            artifacts: builder.artifacts,
        });
    }

    Ok(BrowseResponse {
        scope: ScopeDto::from(&scope),
        total_panes: panes.len(),
        total_artifacts,
        panes,
    })
}

#[derive(Default)]
struct PaneBuilder {
    name_candidates: HashSet<String>,
    artifacts: Vec<BrowseArtifact>,
}

fn project_in_scope(slug: &str, scope: &Scope) -> bool {
    match scope {
        Scope::System => true,
        Scope::Project(s) | Scope::Collection { slug: s, .. } => s == slug,
    }
}

fn collection_in_scope(prefix: &str, scope: &Scope) -> bool {
    match scope {
        Scope::System | Scope::Project(_) => true,
        Scope::Collection { prefix: p, .. } => p == prefix,
    }
}

fn ensure_scope_exists(scope: &Scope, world: &World) -> Result<(), BrowseError> {
    match scope {
        Scope::System => Ok(()),
        Scope::Project(slug) => {
            let present = world.mounts.iter().any(|m| match &m.state {
                MountState::Project(p) => p.config.slug == *slug,
                _ => false,
            });
            if present {
                Ok(())
            } else {
                Err(BrowseError::ProjectNotMounted(slug.clone()))
            }
        }
        Scope::Collection { slug, prefix } => {
            let project_present = world.mounts.iter().any(|m| match &m.state {
                MountState::Project(p) => p.config.slug == *slug,
                _ => false,
            });
            if !project_present {
                return Err(BrowseError::ProjectNotMounted(slug.clone()));
            }
            let found = world.mounts.iter().any(|m| {
                matches!(&m.state, MountState::Project(p)
                    if p.config.slug == *slug
                        && p.collections.iter().any(|c| c.config.prefix == *prefix))
            });
            if found {
                Ok(())
            } else {
                Err(BrowseError::CollectionNotFound {
                    slug: slug.clone(),
                    prefix: prefix.clone(),
                })
            }
        }
    }
}

fn csv_field(raw: Option<&str>) -> Option<Vec<String>> {
    let raw = raw?;
    let mut out: Vec<String> = Vec::new();
    for part in raw.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !out.iter().any(|e| e == trimmed) {
            out.push(trimmed.to_owned());
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn parse_review_state_filter(
    raw: Option<Vec<String>>,
) -> Result<Option<HashSet<ReviewStateTag>>, BrowseError> {
    let Some(list) = raw else { return Ok(None) };
    let mut out: HashSet<ReviewStateTag> = HashSet::new();
    let mut unknown: Vec<String> = Vec::new();
    for entry in list {
        match ReviewStateTag::parse(&entry) {
            Some(tag) => {
                out.insert(tag);
            }
            None => unknown.push(entry),
        }
    }
    if !unknown.is_empty() {
        return Err(BrowseError::UnknownReviewStates(unknown.join(", ")));
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::compute::test_support::{make_artifact, make_world};
    use crate::schema::ArtifactShape;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn uuid(seed: u8) -> Uuid {
        let mut b = [0u8; 16];
        b[15] = seed;
        b[6] = 0x70 | seed;
        Uuid::from_bytes(b)
    }

    fn sample_world() -> crate::world::World {
        let req_a = make_artifact(
            "REQ-apple",
            uuid(1),
            "Apple",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let req_b = make_artifact(
            "REQ-banana",
            uuid(2),
            "Banana",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let req_c = make_artifact(
            "REQ-dropped",
            uuid(3),
            "Dropped",
            ArtifactShape::Content,
            vec![],
            Some(false),
        );
        let des_a = make_artifact(
            "DES-alpha",
            uuid(4),
            "Alpha",
            ArtifactShape::Content,
            vec![],
            None,
        );
        make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![
                (
                    "requirements".into(),
                    "REQ".into(),
                    vec![req_a, req_b, req_c],
                ),
                ("designs".into(), "DES".into(), vec![des_a]),
            ],
        )
    }

    #[test]
    fn default_system_scope_groups_by_prefix_and_sorts_by_title() {
        let world = sample_world();
        let resp = build_browse(Scope::System, &BrowseQuery::default(), &world).unwrap();
        assert_eq!(resp.total_panes, 2);
        // Panes are prefix-ascending: DES before REQ.
        assert_eq!(resp.panes[0].prefix, "DES");
        assert_eq!(resp.panes[1].prefix, "REQ");
        // REQ pane has 2 active artifacts (dropped REQ-c is
        // inactive and excluded by default). Sorted by title:
        // Apple before Banana.
        let req_pane = &resp.panes[1];
        assert_eq!(req_pane.total_artifacts, 2);
        assert_eq!(req_pane.artifacts[0].artifact_name, "REQ-apple");
        assert_eq!(req_pane.artifacts[1].artifact_name, "REQ-banana");
        assert_eq!(resp.total_artifacts, 3);
    }

    #[test]
    fn include_inactive_adds_dropped_artifacts_back() {
        let world = sample_world();
        let resp = build_browse(
            Scope::System,
            &BrowseQuery {
                include_inactive: Some(true),
                ..Default::default()
            },
            &world,
        )
        .unwrap();
        // Now REQ pane has 3 artifacts.
        let req = resp.panes.iter().find(|p| p.prefix == "REQ").unwrap();
        assert_eq!(req.total_artifacts, 3);
    }

    #[test]
    fn scope_collection_narrows_to_one_pane() {
        let world = sample_world();
        let resp = build_browse(
            Scope::Collection {
                slug: "sample".into(),
                prefix: "DES".into(),
            },
            &BrowseQuery::default(),
            &world,
        )
        .unwrap();
        assert_eq!(resp.total_panes, 1);
        assert_eq!(resp.panes[0].prefix, "DES");
    }

    #[test]
    fn tag_filter_keeps_only_matching_artifacts() {
        let mut a = make_artifact("REQ-a", uuid(1), "A", ArtifactShape::Content, vec![], None);
        a.metadata.tags = Some(vec!["core".into(), "safety".into()]);
        let mut b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        b.metadata.tags = Some(vec!["docs".into()]);
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b])],
        );
        let resp = build_browse(
            Scope::System,
            &BrowseQuery {
                tags: Some("safety".into()),
                ..Default::default()
            },
            &world,
        )
        .unwrap();
        assert_eq!(resp.total_panes, 1);
        assert_eq!(resp.panes[0].total_artifacts, 1);
        assert_eq!(resp.panes[0].artifacts[0].artifact_name, "REQ-a");
    }

    #[test]
    fn review_state_filter_uses_derived_state() {
        use crate::schema::ReviewLogEntry;
        let approved = ReviewLogEntry {
            outcome: "approved".into(),
            reviewer: "alice".into(),
            timestamp: chrono::Utc::now(),
            explanation: None,
            added_todos: Vec::new(),
            resolved_todos: Vec::new(),
            overflow: Default::default(),
        };
        let mut a = make_artifact("REQ-a", uuid(1), "A", ArtifactShape::Content, vec![], None);
        a.metadata.review_log = vec![approved];
        let b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b])],
        );
        let resp = build_browse(
            Scope::System,
            &BrowseQuery {
                review_state: Some("approved".into()),
                ..Default::default()
            },
            &world,
        )
        .unwrap();
        assert_eq!(resp.panes[0].total_artifacts, 1);
        assert_eq!(resp.panes[0].artifacts[0].artifact_name, "REQ-a");
    }

    #[test]
    fn name_variants_surface_when_projects_disagree() {
        // Two projects, same REQ prefix but different names.
        let a = make_artifact("REQ-a", uuid(1), "A", ArtifactShape::Content, vec![], None);
        let b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);

        use crate::load::{LoadedCollection, LoadedProject};
        use crate::mount::{MountInfo, MountState};
        use crate::schema::{CollectionConfig, ProjectConfig};
        use std::collections::BTreeMap;

        let build_project = |slug: &str,
                             collection_name: &str,
                             artifact: crate::load::LoadedArtifact|
         -> MountInfo {
            let root = PathBuf::from(format!("/tmp/{slug}"));
            let project = LoadedProject {
                root: root.clone(),
                config: ProjectConfig {
                    schema_version: 1,
                    slug: slug.to_owned(),
                    name: slug.to_owned(),
                    description: None,
                    artifacts_path: None,
                    scan_paths: None,
                    overflow: BTreeMap::new(),
                },
                collections: vec![LoadedCollection {
                    dir_name: "requirements".to_owned(),
                    dir_path: root.join("requirements"),
                    config: CollectionConfig {
                        schema_version: 1,
                        prefix: "REQ".to_owned(),
                        name: collection_name.to_owned(),
                        description: None,
                        expects_code_trace: None,
                        import_notes: None,
                        overflow: BTreeMap::new(),
                    },
                    artifacts: vec![artifact],
                }],
                diagnostics: Vec::new(),
            };
            MountInfo {
                path: root,
                state: MountState::Project(project),
            }
        };

        let mounts = vec![
            build_project("alpha", "Requirements", a),
            build_project("beta", "Regulations", b),
        ];
        let loaded: Vec<&LoadedProject> = mounts
            .iter()
            .filter_map(|m| match &m.state {
                MountState::Project(p) => Some(p),
                _ => None,
            })
            .collect();
        let (index, duplicates) = crate::index::build_uuid_index(&loaded);
        let search_index = crate::search::SearchIndex::build(&mounts)
            .map(std::sync::Arc::new)
            .unwrap();
        let world = crate::world::World {
            mounts,
            index,
            duplicates,
            system: crate::system::LoadedSystem::Unnamed,
            missing_project_slugs: Vec::new(),
            link_catalog: crate::links::builtin_catalog().to_vec(),
            search_index,
        };

        let resp = build_browse(Scope::System, &BrowseQuery::default(), &world).unwrap();
        assert_eq!(resp.total_panes, 1);
        let pane = &resp.panes[0];
        // Display label is lexicographically-first —
        // "Regulations" comes before "Requirements".
        assert_eq!(pane.name, "Regulations");
        assert_eq!(
            pane.name_variants.as_deref(),
            Some(&["Requirements".to_owned()][..])
        );
        assert_eq!(pane.total_artifacts, 2);
    }

    #[test]
    fn unknown_review_state_returns_typed_error() {
        let world = sample_world();
        let err = build_browse(
            Scope::System,
            &BrowseQuery {
                review_state: Some("approved,bogus".into()),
                ..Default::default()
            },
            &world,
        );
        assert!(matches!(err, Err(BrowseError::UnknownReviewStates(_))));
    }

    #[test]
    fn unknown_scope_returns_typed_error() {
        let world = sample_world();
        assert!(matches!(
            build_browse(
                Scope::Project("nope".into()),
                &BrowseQuery::default(),
                &world
            ),
            Err(BrowseError::ProjectNotMounted(_))
        ));
        assert!(matches!(
            build_browse(
                Scope::Collection {
                    slug: "sample".into(),
                    prefix: "NOPE".into()
                },
                &BrowseQuery::default(),
                &world
            ),
            Err(BrowseError::CollectionNotFound { .. })
        ));
    }
}
