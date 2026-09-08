//! Graph-canvas computation (Phase 7a).
//!
//! Pure fold over a `World` snapshot — no IO — producing the
//! capped nodes + edges that back the React-Flow canvas.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::mount::MountState;
use crate::reports::Scope;
use crate::world::World;

use super::{
    GRAPH_NODE_CAP, GraphEdge, GraphError, GraphLinkType, GraphNode, GraphQuery, GraphResponse,
    ScopeDto,
};

pub fn build_graph(
    scope: Scope,
    query: &GraphQuery,
    world: &World,
) -> Result<GraphResponse, GraphError> {
    // Validate scope up front so we 404 consistently before
    // doing any work.
    ensure_scope_exists(&scope, world)?;

    let link_type_filter = query.link_type_list();
    let tag_filter: Option<HashSet<String>> =
        query.tag_list().map(|list| list.into_iter().collect());
    let include_inactive = query.include_inactive();

    // Effective catalog lookup for the per-edge acyclic/directed
    // metadata the UI needs for tooltips + layout hints.
    let catalog: HashMap<&str, &crate::links::LinkType> =
        world.link_catalog.iter().map(|t| (t.name, t)).collect();

    // Pass 1: collect every in-scope node (stable ordering).
    let mut in_scope_nodes: Vec<GraphNode> = Vec::new();
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
                in_scope_nodes.push(GraphNode {
                    uuid: artifact.metadata.uuid,
                    project_slug: project.config.slug.clone(),
                    collection_prefix: collection.config.prefix.clone(),
                    artifact_name: artifact.name.clone(),
                    title: artifact.metadata.title.clone(),
                    shape: artifact.metadata.shape,
                    active: artifact.metadata.is_active(),
                    derived: artifact.metadata.is_derived(),
                    tags,
                });
            }
        }
    }

    // Stable ordering so the cap's "first 500" sample is
    // deterministic across runs.
    in_scope_nodes.sort_by(|a, b| {
        a.project_slug
            .cmp(&b.project_slug)
            .then(a.collection_prefix.cmp(&b.collection_prefix))
            .then(a.artifact_name.cmp(&b.artifact_name))
    });

    let total_nodes = in_scope_nodes.len();
    let truncated = total_nodes > GRAPH_NODE_CAP;
    if truncated {
        in_scope_nodes.truncate(GRAPH_NODE_CAP);
    }

    let visible_uuids: HashSet<Uuid> = in_scope_nodes.iter().map(|n| n.uuid).collect();

    // Pass 2: collect edges whose both endpoints survived the
    // scope + cap trim and whose link type passes the filter.
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut referenced: HashSet<&'static str> = HashSet::new();
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        for collection in &project.collections {
            for artifact in &collection.artifacts {
                if !visible_uuids.contains(&artifact.metadata.uuid) {
                    continue;
                }
                for link in &artifact.metadata.links {
                    if !visible_uuids.contains(&link.target_uuid) {
                        continue;
                    }
                    if let Some(filter) = &link_type_filter
                        && !filter.contains(&link.type_name)
                    {
                        continue;
                    }
                    let catalog_entry = catalog.get(link.type_name.as_str());
                    let (acyclic, directed) = catalog_entry
                        .map(|lt| (lt.acyclic, lt.directed))
                        .unwrap_or((false, true));
                    if let Some(lt) = catalog_entry {
                        referenced.insert(lt.name);
                    }
                    edges.push(GraphEdge {
                        source_uuid: artifact.metadata.uuid,
                        target_uuid: link.target_uuid,
                        link_type: link.type_name.clone(),
                        acyclic,
                        directed,
                    });
                }
            }
        }
    }

    edges.sort_by(|a, b| {
        a.source_uuid
            .cmp(&b.source_uuid)
            .then(a.target_uuid.cmp(&b.target_uuid))
            .then(a.link_type.cmp(&b.link_type))
    });

    let hint_all_edges_acyclic = !edges.is_empty() && edges.iter().all(|e| e.acyclic);

    let mut referenced_link_types: Vec<GraphLinkType> = world
        .link_catalog
        .iter()
        .filter(|lt| referenced.contains(lt.name))
        .map(GraphLinkType::from)
        .collect();
    referenced_link_types.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(GraphResponse {
        scope: ScopeDto::from(&scope),
        total_nodes,
        truncated,
        hint_all_edges_acyclic,
        referenced_link_types,
        nodes: in_scope_nodes,
        edges,
    })
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

/// Typed 404 surface for unknown scope values. Mirrors the
/// Phase 6a reports `in_scope_artifacts` contract so handlers
/// map both kinds of "not mounted" into the same 404.
fn ensure_scope_exists(scope: &Scope, world: &World) -> Result<(), GraphError> {
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
                Err(GraphError::ProjectNotMounted(slug.clone()))
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
                // Distinguish missing-project vs missing-collection
                // so the handler's error message names the right
                // thing.
                let project_present = world.mounts.iter().any(|m| match &m.state {
                    MountState::Project(p) => p.config.slug == *slug,
                    _ => false,
                });
                if !project_present {
                    Err(GraphError::ProjectNotMounted(slug.clone()))
                } else {
                    Err(GraphError::CollectionNotFound {
                        slug: slug.clone(),
                        prefix: prefix.clone(),
                    })
                }
            }
        }
    }
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

    #[test]
    fn default_query_returns_all_in_scope_nodes_sorted_stably() {
        let a = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        let b = make_artifact("REQ-a", uuid(1), "A", ArtifactShape::Content, vec![], None);
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b])],
        );
        let resp = build_graph(Scope::System, &GraphQuery::default(), &world).unwrap();
        assert_eq!(resp.total_nodes, 2);
        assert!(!resp.truncated);
        // Sorted by (project, collection, name): REQ-a before REQ-b.
        assert_eq!(resp.nodes[0].artifact_name, "REQ-a");
        assert_eq!(resp.nodes[1].artifact_name, "REQ-b");
    }

    #[test]
    fn inactive_excluded_by_default_and_included_on_flag() {
        let active = make_artifact("REQ-a", uuid(1), "A", ArtifactShape::Content, vec![], None);
        let inactive = make_artifact(
            "REQ-old",
            uuid(2),
            "Old",
            ArtifactShape::Content,
            vec![],
            Some(false),
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![active, inactive])],
        );
        let off = build_graph(Scope::System, &GraphQuery::default(), &world).unwrap();
        assert_eq!(off.total_nodes, 1);
        let on = build_graph(
            Scope::System,
            &GraphQuery {
                include_inactive: Some(true),
                ..Default::default()
            },
            &world,
        )
        .unwrap();
        assert_eq!(on.total_nodes, 2);
    }

    #[test]
    fn link_type_filter_prunes_edges_but_leaves_nodes_alone() {
        // Two link types, only derives-from requested. Edge of
        // other type must be dropped but nodes survive.
        let h = |name: &str| hint("sample", "REQ", name);
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![
                link(uuid(2), "derives-from", h("REQ-b")),
                link(uuid(3), "related-to", h("REQ-c")),
            ],
            None,
        );
        let b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        let c = make_artifact("REQ-c", uuid(3), "C", ArtifactShape::Content, vec![], None);
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b, c])],
        );
        let resp = build_graph(
            Scope::System,
            &GraphQuery {
                link_types: Some("derives-from".into()),
                ..Default::default()
            },
            &world,
        )
        .unwrap();
        assert_eq!(resp.total_nodes, 3);
        assert_eq!(resp.edges.len(), 1);
        assert_eq!(resp.edges[0].link_type, "derives-from");
        // Hint is true because the only edge's link type is acyclic.
        assert!(resp.hint_all_edges_acyclic);
    }

    #[test]
    fn tag_filter_keeps_only_artifacts_with_a_matching_tag() {
        let mut a = make_artifact("REQ-a", uuid(1), "A", ArtifactShape::Content, vec![], None);
        a.metadata.tags = Some(vec!["safety".into(), "core".into()]);
        let mut b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        b.metadata.tags = Some(vec!["docs".into()]);
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b])],
        );
        let resp = build_graph(
            Scope::System,
            &GraphQuery {
                tags: Some("safety".into()),
                ..Default::default()
            },
            &world,
        )
        .unwrap();
        assert_eq!(resp.total_nodes, 1);
        assert_eq!(resp.nodes[0].artifact_name, "REQ-a");
    }

    #[test]
    fn cap_trims_and_edges_only_cross_surviving_nodes() {
        let mut artifacts = Vec::new();
        // 502 nodes — first 500 are isolated, last 2 edge to
        // each other.
        for i in 1..=500 {
            let name = format!("REQ-{:04}", i);
            artifacts.push(make_artifact(
                &name,
                uuid(i as u8),
                &name,
                ArtifactShape::Content,
                vec![],
                None,
            ));
        }
        let h = |name: &str| hint("sample", "REQ", name);
        let mut a501 = make_artifact(
            "REQ-0501",
            Uuid::parse_str("00000000-0000-7000-8000-000000000501").unwrap(),
            "501",
            ArtifactShape::Content,
            vec![],
            None,
        );
        // Link to a node that survives the cap (REQ-0001 sorts
        // first alphabetically).
        a501.metadata.links = vec![link(uuid(1), "related-to", h("REQ-0001"))];
        let a502 = make_artifact(
            "REQ-0502",
            Uuid::parse_str("00000000-0000-7000-8000-000000000502").unwrap(),
            "502",
            ArtifactShape::Content,
            vec![],
            None,
        );
        artifacts.push(a501);
        artifacts.push(a502);

        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), artifacts)],
        );
        let resp = build_graph(Scope::System, &GraphQuery::default(), &world).unwrap();
        assert_eq!(resp.total_nodes, 502);
        assert!(resp.truncated);
        assert_eq!(resp.nodes.len(), GRAPH_NODE_CAP);
        // The edge REQ-0501 → REQ-0001 should have been dropped
        // because REQ-0501 itself is past the cap.
        assert!(!resp.edges.iter().any(|e| e.link_type == "related-to"));
    }

    #[test]
    fn hint_is_false_when_any_edge_is_non_acyclic() {
        let h = |name: &str| hint("sample", "REQ", name);
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![
                link(uuid(2), "derives-from", h("REQ-b")),
                link(uuid(3), "satisfies", h("REQ-c")),
            ],
            None,
        );
        let b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        let c = make_artifact("REQ-c", uuid(3), "C", ArtifactShape::Content, vec![], None);
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b, c])],
        );
        let resp = build_graph(Scope::System, &GraphQuery::default(), &world).unwrap();
        assert!(!resp.hint_all_edges_acyclic);
    }

    #[test]
    fn unknown_scope_returns_typed_error() {
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![])],
        );
        assert!(matches!(
            build_graph(
                Scope::Project("nope".into()),
                &GraphQuery::default(),
                &world
            ),
            Err(GraphError::ProjectNotMounted(_))
        ));
        assert!(matches!(
            build_graph(
                Scope::Collection {
                    slug: "sample".into(),
                    prefix: "DES".into()
                },
                &GraphQuery::default(),
                &world
            ),
            Err(GraphError::CollectionNotFound { .. })
        ));
    }

    #[test]
    fn referenced_link_types_carry_catalog_metadata() {
        let h = |name: &str| hint("sample", "REQ", name);
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(uuid(2), "derives-from", h("REQ-b"))],
            None,
        );
        let b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b])],
        );
        let resp = build_graph(Scope::System, &GraphQuery::default(), &world).unwrap();
        assert_eq!(resp.referenced_link_types.len(), 1);
        let lt = &resp.referenced_link_types[0];
        assert_eq!(lt.name, "derives-from");
        assert!(lt.directed);
        assert!(lt.acyclic);
    }
}
