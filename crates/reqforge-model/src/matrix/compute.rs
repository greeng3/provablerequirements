//! Matrix-link-view computation (Phase 7b).
//!
//! Pure fold over a `World` snapshot — no IO — producing the
//! per-axis capped node sets + the edges of the chosen link type
//! that connect them.

use std::collections::HashSet;

use crate::graph::GraphLinkType;
use crate::mount::MountState;
use crate::reports::Scope;
use crate::reviews::derive_review_state;
use crate::world::World;

use super::{
    MATRIX_AXIS_CAP, MatrixEdge, MatrixError, MatrixNode, MatrixQuery, MatrixResponse,
    ReviewStateTag, ScopeDto,
};

pub fn build_matrix(
    row_scope: Scope,
    column_scope: Scope,
    query: &MatrixQuery,
    world: &World,
) -> Result<MatrixResponse, MatrixError> {
    // Validate both scopes up front so we 404 consistently
    // before doing any work.
    ensure_scope_exists(&row_scope, world)?;
    ensure_scope_exists(&column_scope, world)?;

    // Required link-type param, resolved against the effective
    // catalog. Unknown name → typed 400.
    let Some(ref link_type_name) = query.link_type else {
        return Err(MatrixError::LinkTypeRequired);
    };
    let catalog_entry = world
        .link_catalog
        .iter()
        .find(|lt| lt.name == link_type_name)
        .ok_or_else(|| MatrixError::UnknownLinkType(link_type_name.clone()))?;
    let link_type_meta = GraphLinkType::from(catalog_entry);

    // Per-axis review-state filter. Typo-tolerance would be a
    // footgun here — surface unknowns as a single 400 rather
    // than silently dropping the filter.
    let row_review_filter = parse_review_state_filter(query.row_review_state_list())?;
    let column_review_filter = parse_review_state_filter(query.column_review_state_list())?;

    let row_tag_filter: Option<HashSet<String>> =
        query.row_tag_list().map(|l| l.into_iter().collect());
    let column_tag_filter: Option<HashSet<String>> =
        query.column_tag_list().map(|l| l.into_iter().collect());
    let include_inactive = query.include_inactive();

    // Walk the mount tree once, collecting every in-scope node
    // for each axis. Row and column axes are independent so an
    // artifact may appear on one, both, or neither.
    let mut rows: Vec<MatrixNode> = Vec::new();
    let mut columns: Vec<MatrixNode> = Vec::new();
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        for collection in &project.collections {
            for artifact in &collection.artifacts {
                if !include_inactive && !artifact.metadata.is_active() {
                    continue;
                }
                let derived_state = derive_review_state(&artifact.metadata.review_log);
                let review_tag = ReviewStateTag::from_derived(&derived_state);
                let tags: Vec<String> = artifact.metadata.tags.clone().unwrap_or_default();

                let build_node = || MatrixNode {
                    uuid: artifact.metadata.uuid,
                    project_slug: project.config.slug.clone(),
                    collection_prefix: collection.config.prefix.clone(),
                    artifact_name: artifact.name.clone(),
                    title: artifact.metadata.title.clone(),
                    shape: artifact.metadata.shape,
                    active: artifact.metadata.is_active(),
                    derived: artifact.metadata.is_derived(),
                    tags: tags.clone(),
                    review_state: review_tag,
                };

                if node_matches_axis(
                    &project.config.slug,
                    &collection.config.prefix,
                    &tags,
                    review_tag,
                    &row_scope,
                    &row_tag_filter,
                    &row_review_filter,
                ) {
                    rows.push(build_node());
                }
                if node_matches_axis(
                    &project.config.slug,
                    &collection.config.prefix,
                    &tags,
                    review_tag,
                    &column_scope,
                    &column_tag_filter,
                    &column_review_filter,
                ) {
                    columns.push(build_node());
                }
            }
        }
    }

    // Stable sort so the first-500 sample under the cap is
    // deterministic across runs and the UI's header order
    // matches what operators see in other views.
    rows.sort_by(cmp_by_path);
    columns.sort_by(cmp_by_path);

    let total_rows = rows.len();
    let total_columns = columns.len();
    let rows_truncated = total_rows > MATRIX_AXIS_CAP;
    let columns_truncated = total_columns > MATRIX_AXIS_CAP;

    // If either axis is over the cap, we refuse to draw a
    // partial matrix. Partial matrices mis-represent coverage
    // (cells for hidden artifacts look like "no link" when
    // they might carry one). Clear both axes + edges so the
    // frontend banners without confusion.
    if rows_truncated || columns_truncated {
        return Ok(MatrixResponse {
            row_scope: ScopeDto::from(&row_scope),
            column_scope: ScopeDto::from(&column_scope),
            link_type: link_type_meta,
            total_rows,
            rows_truncated,
            total_columns,
            columns_truncated,
            rows: Vec::new(),
            columns: Vec::new(),
            edges: Vec::new(),
        });
    }

    // Edges: walk each row artifact's outgoing links of the
    // chosen type; keep the ones whose target is in the column
    // axis. Using a HashSet of column UUIDs keeps the inner
    // check O(1).
    let column_uuids: HashSet<uuid::Uuid> = columns.iter().map(|n| n.uuid).collect();
    let row_uuids: HashSet<uuid::Uuid> = rows.iter().map(|n| n.uuid).collect();
    let mut edges: Vec<MatrixEdge> = Vec::new();
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        for collection in &project.collections {
            for artifact in &collection.artifacts {
                if !row_uuids.contains(&artifact.metadata.uuid) {
                    continue;
                }
                for link in &artifact.metadata.links {
                    if link.type_name != *link_type_name {
                        continue;
                    }
                    if !column_uuids.contains(&link.target_uuid) {
                        continue;
                    }
                    edges.push(MatrixEdge {
                        row_uuid: artifact.metadata.uuid,
                        column_uuid: link.target_uuid,
                    });
                }
            }
        }
    }

    edges.sort_by(|a, b| {
        a.row_uuid
            .cmp(&b.row_uuid)
            .then(a.column_uuid.cmp(&b.column_uuid))
    });

    Ok(MatrixResponse {
        row_scope: ScopeDto::from(&row_scope),
        column_scope: ScopeDto::from(&column_scope),
        link_type: link_type_meta,
        total_rows,
        rows_truncated: false,
        total_columns,
        columns_truncated: false,
        rows,
        columns,
        edges,
    })
}

fn cmp_by_path(a: &MatrixNode, b: &MatrixNode) -> std::cmp::Ordering {
    a.project_slug
        .cmp(&b.project_slug)
        .then(a.collection_prefix.cmp(&b.collection_prefix))
        .then(a.artifact_name.cmp(&b.artifact_name))
}

#[allow(clippy::too_many_arguments)]
fn node_matches_axis(
    project_slug: &str,
    collection_prefix: &str,
    tags: &[String],
    review_tag: ReviewStateTag,
    scope: &Scope,
    tag_filter: &Option<HashSet<String>>,
    review_filter: &Option<HashSet<ReviewStateTag>>,
) -> bool {
    if !project_in_scope(project_slug, scope) {
        return false;
    }
    if !collection_in_scope(collection_prefix, scope) {
        return false;
    }
    if let Some(wanted) = tag_filter
        && !tags.iter().any(|t| wanted.contains(t))
    {
        return false;
    }
    if let Some(wanted) = review_filter
        && !wanted.contains(&review_tag)
    {
        return false;
    }
    true
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

fn ensure_scope_exists(scope: &Scope, world: &World) -> Result<(), MatrixError> {
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
                Err(MatrixError::ProjectNotMounted(slug.clone()))
            }
        }
        Scope::Collection { slug, prefix } => {
            let found = world.mounts.iter().any(|m| {
                matches!(&m.state, MountState::Project(p)
                    if p.config.slug == *slug
                        && p.collections.iter().any(|c| c.config.prefix == *prefix))
            });
            if found {
                Ok(())
            } else {
                let project_present = world.mounts.iter().any(|m| match &m.state {
                    MountState::Project(p) => p.config.slug == *slug,
                    _ => false,
                });
                if !project_present {
                    Err(MatrixError::ProjectNotMounted(slug.clone()))
                } else {
                    Err(MatrixError::CollectionNotFound {
                        slug: slug.clone(),
                        prefix: prefix.clone(),
                    })
                }
            }
        }
    }
}

fn parse_review_state_filter(
    raw: Option<Vec<String>>,
) -> Result<Option<HashSet<ReviewStateTag>>, MatrixError> {
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
        return Err(MatrixError::UnknownReviewStates(unknown.join(", ")));
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::compute::test_support::{hint, link, make_artifact, make_world};
    use crate::schema::ArtifactShape;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn uuid(seed: u8) -> Uuid {
        let mut b = [0u8; 16];
        b[15] = seed;
        b[6] = 0x70 | seed;
        Uuid::from_bytes(b)
    }

    fn des_hint(name: &str) -> crate::schema::LinkHint {
        hint("sample", "DES", name)
    }

    fn sample_world_two_collections(
        req_artifacts: Vec<crate::load::LoadedArtifact>,
        des_artifacts: Vec<crate::load::LoadedArtifact>,
    ) -> crate::world::World {
        make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![
                ("requirements".into(), "REQ".into(), req_artifacts),
                ("designs".into(), "DES".into(), des_artifacts),
            ],
        )
    }

    #[test]
    fn default_matrix_lists_rows_and_columns_stably_with_link_edges() {
        let req_a = make_artifact(
            "REQ-a",
            uuid(1),
            "Requirement A",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let req_b = make_artifact(
            "REQ-b",
            uuid(2),
            "Requirement B",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let mut des_a = make_artifact(
            "DES-a",
            uuid(3),
            "Design A",
            ArtifactShape::Content,
            vec![],
            None,
        );
        // DES-a satisfies REQ-a: since the matrix is
        // row-scoped to REQ (row source) and column-scoped to
        // DES (column target), this edge appears only if we
        // treat rows as satisfies-sources. We flip that by
        // making REQ-a satisfy DES-a for the row-DES case — but
        // actually the normal matrix shape is row = source of
        // the link. To keep this test simple, we put the
        // satisfies link on DES-a pointing at REQ-a and test
        // the reverse direction below; here we use the
        // satisfies link on REQ-a pointing at DES-a.
        des_a.metadata.links = vec![];
        let mut req_with_link = make_artifact(
            "REQ-c",
            uuid(4),
            "Requirement C",
            ArtifactShape::Content,
            vec![link(uuid(3), "satisfies", des_hint("DES-a"))],
            None,
        );
        // Silence the unused mut warning.
        req_with_link.metadata.title.push_str("");

        let world = sample_world_two_collections(vec![req_a, req_b, req_with_link], vec![des_a]);

        let query = MatrixQuery {
            row_scope: Some("collection:sample/REQ".into()),
            column_scope: Some("collection:sample/DES".into()),
            link_type: Some("satisfies".into()),
            ..Default::default()
        };
        let resp = build_matrix(
            Scope::Collection {
                slug: "sample".into(),
                prefix: "REQ".into(),
            },
            Scope::Collection {
                slug: "sample".into(),
                prefix: "DES".into(),
            },
            &query,
            &world,
        )
        .unwrap();
        assert_eq!(resp.total_rows, 3);
        assert_eq!(resp.total_columns, 1);
        assert!(!resp.rows_truncated);
        assert!(!resp.columns_truncated);
        // Stable (project, collection, name) order.
        assert_eq!(resp.rows[0].artifact_name, "REQ-a");
        assert_eq!(resp.rows[1].artifact_name, "REQ-b");
        assert_eq!(resp.rows[2].artifact_name, "REQ-c");
        // One satisfies edge: REQ-c → DES-a.
        assert_eq!(resp.edges.len(), 1);
        assert_eq!(resp.edges[0].row_uuid, uuid(4));
        assert_eq!(resp.edges[0].column_uuid, uuid(3));
    }

    #[test]
    fn wrong_link_type_filters_edges_but_keeps_axes() {
        let req = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(uuid(2), "satisfies", des_hint("DES-a"))],
            None,
        );
        let des = make_artifact(
            "DES-a",
            uuid(2),
            "Design A",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let world = sample_world_two_collections(vec![req], vec![des]);
        let query = MatrixQuery {
            row_scope: Some("collection:sample/REQ".into()),
            column_scope: Some("collection:sample/DES".into()),
            link_type: Some("verifies".into()),
            ..Default::default()
        };
        let resp = build_matrix(
            Scope::Collection {
                slug: "sample".into(),
                prefix: "REQ".into(),
            },
            Scope::Collection {
                slug: "sample".into(),
                prefix: "DES".into(),
            },
            &query,
            &world,
        )
        .unwrap();
        // Both axes populated — filter is on the edge's link
        // type only — but the sole satisfies edge is dropped.
        assert_eq!(resp.rows.len(), 1);
        assert_eq!(resp.columns.len(), 1);
        assert_eq!(resp.edges.len(), 0);
    }

    #[test]
    fn per_axis_tag_filter_narrows_that_axis_only() {
        let mut req_a = make_artifact("REQ-a", uuid(1), "A", ArtifactShape::Content, vec![], None);
        req_a.metadata.tags = Some(vec!["core".into()]);
        let mut req_b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        req_b.metadata.tags = Some(vec!["fringe".into()]);
        let mut des_a = make_artifact(
            "DES-a",
            uuid(3),
            "Design A",
            ArtifactShape::Content,
            vec![],
            None,
        );
        des_a.metadata.tags = Some(vec!["core".into()]);
        let world = sample_world_two_collections(vec![req_a, req_b], vec![des_a]);
        let query = MatrixQuery {
            row_scope: Some("collection:sample/REQ".into()),
            column_scope: Some("collection:sample/DES".into()),
            link_type: Some("satisfies".into()),
            row_tags: Some("core".into()),
            ..Default::default()
        };
        let resp = build_matrix(
            Scope::Collection {
                slug: "sample".into(),
                prefix: "REQ".into(),
            },
            Scope::Collection {
                slug: "sample".into(),
                prefix: "DES".into(),
            },
            &query,
            &world,
        )
        .unwrap();
        // Row filter keeps REQ-a only; column axis unaffected.
        assert_eq!(resp.rows.len(), 1);
        assert_eq!(resp.rows[0].artifact_name, "REQ-a");
        assert_eq!(resp.columns.len(), 1);
    }

    #[test]
    fn review_state_filter_uses_derived_state() {
        // REQ-a has an `approved` log entry; REQ-b has no log.
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
        let mut req_a = make_artifact("REQ-a", uuid(1), "A", ArtifactShape::Content, vec![], None);
        req_a.metadata.review_log = vec![approved];
        let req_b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        let des = make_artifact(
            "DES-a",
            uuid(3),
            "Design A",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let world = sample_world_two_collections(vec![req_a, req_b], vec![des]);
        let query = MatrixQuery {
            row_scope: Some("collection:sample/REQ".into()),
            column_scope: Some("collection:sample/DES".into()),
            link_type: Some("satisfies".into()),
            row_review_states: Some("approved".into()),
            ..Default::default()
        };
        let resp = build_matrix(
            Scope::Collection {
                slug: "sample".into(),
                prefix: "REQ".into(),
            },
            Scope::Collection {
                slug: "sample".into(),
                prefix: "DES".into(),
            },
            &query,
            &world,
        )
        .unwrap();
        assert_eq!(resp.rows.len(), 1);
        assert_eq!(resp.rows[0].artifact_name, "REQ-a");
        assert_eq!(resp.rows[0].review_state, ReviewStateTag::Approved);
    }

    #[test]
    fn per_axis_cap_renders_blocking_empty_response() {
        // 501 REQ artifacts in the row axis; column axis below
        // the cap. Response should carry rows_truncated=true
        // with rows + columns + edges all empty.
        let mut req_artifacts = Vec::new();
        for i in 1..=501 {
            let name = format!("REQ-{:04}", i);
            let mut b = [0u8; 16];
            b[14] = (i >> 8) as u8;
            b[15] = (i & 0xff) as u8;
            b[6] = 0x70;
            req_artifacts.push(make_artifact(
                &name,
                Uuid::from_bytes(b),
                &name,
                ArtifactShape::Content,
                vec![],
                None,
            ));
        }
        let des = make_artifact(
            "DES-a",
            uuid(1),
            "Design A",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let world = sample_world_two_collections(req_artifacts, vec![des]);
        let query = MatrixQuery {
            row_scope: Some("collection:sample/REQ".into()),
            column_scope: Some("collection:sample/DES".into()),
            link_type: Some("satisfies".into()),
            ..Default::default()
        };
        let resp = build_matrix(
            Scope::Collection {
                slug: "sample".into(),
                prefix: "REQ".into(),
            },
            Scope::Collection {
                slug: "sample".into(),
                prefix: "DES".into(),
            },
            &query,
            &world,
        )
        .unwrap();
        assert!(resp.rows_truncated);
        assert_eq!(resp.total_rows, 501);
        assert_eq!(resp.total_columns, 1);
        assert!(!resp.columns_truncated);
        assert!(resp.rows.is_empty());
        assert!(resp.columns.is_empty());
        assert!(resp.edges.is_empty());
    }

    #[test]
    fn missing_link_type_returns_typed_error() {
        let world = sample_world_two_collections(vec![], vec![]);
        let query = MatrixQuery {
            row_scope: Some("system".into()),
            column_scope: Some("system".into()),
            ..Default::default()
        };
        assert!(matches!(
            build_matrix(Scope::System, Scope::System, &query, &world),
            Err(MatrixError::LinkTypeRequired)
        ));
    }

    #[test]
    fn unknown_link_type_returns_typed_error() {
        let world = sample_world_two_collections(vec![], vec![]);
        let query = MatrixQuery {
            link_type: Some("not-a-real-type".into()),
            ..Default::default()
        };
        assert!(matches!(
            build_matrix(Scope::System, Scope::System, &query, &world),
            Err(MatrixError::UnknownLinkType(_))
        ));
    }

    #[test]
    fn unknown_review_state_returns_typed_error() {
        let world = sample_world_two_collections(vec![], vec![]);
        let query = MatrixQuery {
            link_type: Some("satisfies".into()),
            row_review_states: Some("approved, bogus".into()),
            ..Default::default()
        };
        assert!(matches!(
            build_matrix(Scope::System, Scope::System, &query, &world),
            Err(MatrixError::UnknownReviewStates(_))
        ));
    }

    #[test]
    fn unknown_scope_returns_typed_error() {
        let world = sample_world_two_collections(vec![], vec![]);
        let query = MatrixQuery {
            link_type: Some("satisfies".into()),
            ..Default::default()
        };
        assert!(matches!(
            build_matrix(Scope::Project("nope".into()), Scope::System, &query, &world),
            Err(MatrixError::ProjectNotMounted(_))
        ));
    }
}
