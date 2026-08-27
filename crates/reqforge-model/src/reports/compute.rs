//! Individual report computations (Phase 6a).
//!
//! Each compute function takes the [`Scope`], the `includeInactive`
//! flag, and an immutable `World` snapshot, and returns the matching
//! DTO variant body. Nothing in here hits disk — reports are a pure
//! fold over the already-loaded World.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::load::LoadedArtifact;
use crate::mount::MountState;
use crate::world::World;

use super::{
    ConflictPair, ConflictsReport, CoverageChildEntry, CoverageMatrixReport, CoverageParentEntry,
    CycleEntry, CycleNode, CyclesReport, FilesystemOrphansReport, ImpactAnalysisReport,
    ImpactedArtifact, LinkOrphanEntry, LinkOrphansReport, OrphanBinary, OrphanSidecar, ReportError,
    ReportQuery, ReviewStatusByCollection, ReviewStatusByProject, ReviewStatusByShape,
    ReviewStatusCounts, ReviewStatusReport, Scope, ScopeDto, UnresolvedLinkEntry,
    UnresolvedLinksReport,
};

/// REPORT-coverageMatrix default — "a parent is covered when at
/// least one design/implementation claims to fulfil it and at
/// least one verification claims to confirm it".
pub const DEFAULT_COVERING_LINK_TYPES: &[&str] = &["satisfies", "verifies"];

/// Cap on the BFS frontier for impact analysis. A pathological
/// graph shouldn't take the report down; depth-first explosion
/// on derives-from chains can be deep but the repo population is
/// nowhere near this bound.
pub const MAX_IMPACTED_ARTIFACTS: usize = 5_000;

/// Cap on cycles reported per link type. Protects the UI and the
/// handler from a pathological graph with thousands of entangled
/// cycles — operators fix the first batch, the report refreshes,
/// the next batch surfaces.
pub const MAX_CYCLES_PER_LINK_TYPE: usize = 100;

/// Conflict-pair cap. Same rationale as above; a hand-authored
/// repo with hundreds of conflict pairs is a red flag worth
/// surfacing via the cap rather than flooding the UI.
pub const MAX_CONFLICT_PAIRS: usize = 500;

/// Output of [`in_scope_artifacts`]: one `(project_slug,
/// collection_prefix, &LoadedArtifact)` tuple per in-scope
/// artifact after the `include_inactive` filter.
struct ScopedArtifact<'a> {
    project_slug: &'a str,
    collection_prefix: &'a str,
    artifact: &'a LoadedArtifact,
}

/// Filter the world's loaded artifacts down to the set selected by
/// `scope`, further filtered by the `include_inactive` toggle.
/// Validates the scope exists on the way in; an unknown project or
/// collection surfaces as a typed error so the handler can return a
/// 404 rather than a silently-empty report.
fn in_scope_artifacts<'a>(
    world: &'a World,
    scope: &Scope,
    include_inactive: bool,
) -> Result<Vec<ScopedArtifact<'a>>, ReportError> {
    let mut out = Vec::new();
    let mut project_seen = false;
    let mut collection_seen = false;

    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        let project_match = match scope {
            Scope::System => true,
            Scope::Project(slug) | Scope::Collection { slug, .. } => project.config.slug == *slug,
        };
        if !project_match {
            continue;
        }
        if matches!(scope, Scope::Project(_) | Scope::Collection { .. }) {
            project_seen = true;
        }
        for collection in &project.collections {
            let collection_match = match scope {
                Scope::System | Scope::Project(_) => true,
                Scope::Collection { prefix, .. } => collection.config.prefix == *prefix,
            };
            if !collection_match {
                continue;
            }
            if matches!(scope, Scope::Collection { .. }) {
                collection_seen = true;
            }
            for artifact in &collection.artifacts {
                if !include_inactive && !artifact.metadata.is_active() {
                    continue;
                }
                out.push(ScopedArtifact {
                    project_slug: &project.config.slug,
                    collection_prefix: &collection.config.prefix,
                    artifact,
                });
            }
        }
    }

    match scope {
        Scope::System => {}
        Scope::Project(slug) => {
            if !project_seen {
                return Err(ReportError::ProjectNotMounted(slug.clone()));
            }
        }
        Scope::Collection { slug, prefix } => {
            if !project_seen {
                return Err(ReportError::ProjectNotMounted(slug.clone()));
            }
            if !collection_seen {
                return Err(ReportError::CollectionNotFound {
                    slug: slug.clone(),
                    prefix: prefix.clone(),
                });
            }
        }
    }

    Ok(out)
}

pub fn unresolved_links(
    scope: &Scope,
    include_inactive: bool,
    world: &World,
) -> Result<UnresolvedLinksReport, ReportError> {
    let scoped = in_scope_artifacts(world, scope, include_inactive)?;
    let mounted_slugs: HashSet<&str> = world
        .mounts
        .iter()
        .filter_map(|m| match &m.state {
            MountState::Project(p) => Some(p.config.slug.as_str()),
            _ => None,
        })
        .collect();

    let mut entries = Vec::new();
    for s in &scoped {
        for link in &s.artifact.metadata.links {
            if world.index.get(&link.target_uuid).is_some() {
                continue;
            }
            let reason: &'static str = if mounted_slugs.contains(link.hint.project_slug.as_str()) {
                "target-missing"
            } else {
                "mount-missing"
            };
            entries.push(UnresolvedLinkEntry {
                source_uuid: s.artifact.metadata.uuid,
                source_project_slug: s.project_slug.to_owned(),
                source_collection_prefix: s.collection_prefix.to_owned(),
                source_artifact_name: s.artifact.name.clone(),
                source_title: s.artifact.metadata.title.clone(),
                source_shape: s.artifact.metadata.shape,
                link_type: link.type_name.clone(),
                target_uuid: link.target_uuid,
                target_hint_project_slug: link.hint.project_slug.clone(),
                target_hint_collection_prefix: link.hint.collection_prefix.clone(),
                target_hint_artifact_name: link.hint.artifact_name.clone(),
                reason,
            });
        }
    }

    // Deterministic order for test stability + UI consistency.
    entries.sort_by(|a, b| {
        a.source_project_slug
            .cmp(&b.source_project_slug)
            .then(a.source_collection_prefix.cmp(&b.source_collection_prefix))
            .then(a.source_artifact_name.cmp(&b.source_artifact_name))
            .then(a.target_uuid.cmp(&b.target_uuid))
    });

    Ok(UnresolvedLinksReport {
        scope: ScopeDto::from(scope),
        total_unresolved: entries.len(),
        entries,
    })
}

pub fn link_orphans(
    scope: &Scope,
    include_inactive: bool,
    world: &World,
) -> Result<LinkOrphansReport, ReportError> {
    // Incoming-edge count is computed across the *entire* world,
    // not just the scoped slice — an artifact in scope REF might
    // be referenced by an artifact in scope REQ, which would keep
    // it from being an orphan. Scope filtering only decides which
    // artifacts we check, not which links we count.
    let mut incoming: HashMap<Uuid, usize> = HashMap::new();
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        for collection in &project.collections {
            for artifact in &collection.artifacts {
                for link in &artifact.metadata.links {
                    *incoming.entry(link.target_uuid).or_insert(0) += 1;
                }
            }
        }
    }

    let scoped = in_scope_artifacts(world, scope, include_inactive)?;
    let mut entries = Vec::new();
    for s in &scoped {
        let uuid = s.artifact.metadata.uuid;
        let out_count = s.artifact.metadata.links.len();
        let in_count = incoming.get(&uuid).copied().unwrap_or(0);
        if out_count == 0 && in_count == 0 {
            entries.push(LinkOrphanEntry {
                uuid,
                project_slug: s.project_slug.to_owned(),
                collection_prefix: s.collection_prefix.to_owned(),
                artifact_name: s.artifact.name.clone(),
                title: s.artifact.metadata.title.clone(),
                shape: s.artifact.metadata.shape,
                active: s.artifact.metadata.is_active(),
                derived: s.artifact.metadata.is_derived(),
            });
        }
    }

    entries.sort_by(|a, b| {
        a.project_slug
            .cmp(&b.project_slug)
            .then(a.collection_prefix.cmp(&b.collection_prefix))
            .then(a.artifact_name.cmp(&b.artifact_name))
    });

    Ok(LinkOrphansReport {
        scope: ScopeDto::from(scope),
        total_orphans: entries.len(),
        entries,
    })
}

/// Per-artifact snapshot used by the graph-walk reports (cycles,
/// conflicts). Built once at the top of each report from the
/// world's mounts so the DFS and pair-walk don't re-scan the
/// Project structure on every node lookup.
struct NodeInfo {
    project_slug: String,
    collection_prefix: String,
    artifact_name: String,
    title: String,
    shape: crate::schema::ArtifactShape,
    active: bool,
}

fn build_node_index(world: &World) -> std::collections::HashMap<Uuid, NodeInfo> {
    let mut nodes = std::collections::HashMap::new();
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        for collection in &project.collections {
            for artifact in &collection.artifacts {
                nodes.insert(
                    artifact.metadata.uuid,
                    NodeInfo {
                        project_slug: project.config.slug.clone(),
                        collection_prefix: collection.config.prefix.clone(),
                        artifact_name: artifact.name.clone(),
                        title: artifact.metadata.title.clone(),
                        shape: artifact.metadata.shape,
                        active: artifact.metadata.is_active(),
                    },
                );
            }
        }
    }
    nodes
}

fn node_to_dto(uuid: Uuid, info: &NodeInfo) -> CycleNode {
    CycleNode {
        uuid,
        project_slug: info.project_slug.clone(),
        collection_prefix: info.collection_prefix.clone(),
        artifact_name: info.artifact_name.clone(),
        title: info.title.clone(),
        shape: info.shape,
        active: info.active,
    }
}

/// Does `uuid` fall inside `scope`? System matches everything;
/// Project matches every artifact under the slug; Collection
/// matches only (slug, prefix).
fn node_in_scope(info: &NodeInfo, scope: &Scope) -> bool {
    match scope {
        Scope::System => true,
        Scope::Project(slug) => info.project_slug == *slug,
        Scope::Collection { slug, prefix } => {
            info.project_slug == *slug && info.collection_prefix == *prefix
        }
    }
}

/// Cycles report — one DFS per acyclic link type in the effective
/// catalog. Each cycle is trimmed to the loop portion (the path
/// from the repeating node back to itself). Scope filtering
/// reports a cycle iff at least one of its nodes is in scope;
/// inactive artifacts are excluded from the graph when
/// `include_inactive` is false.
pub fn cycles(
    scope: &Scope,
    include_inactive: bool,
    world: &World,
) -> Result<CyclesReport, ReportError> {
    // Validate scope matches a real mount / collection before we
    // build the graph — consistent 404 semantics with the other
    // reports.
    let _ = in_scope_artifacts(world, scope, include_inactive)?;
    let nodes = build_node_index(world);

    let acyclic_types: Vec<&crate::links::LinkType> = world
        .link_catalog
        .iter()
        .filter(|t| t.acyclic && t.directed)
        .collect();

    let mut cycles: Vec<CycleEntry> = Vec::new();
    let mut truncated = false;
    for lt in &acyclic_types {
        // Build the adjacency restricted to this link type and the
        // current include_inactive filter.
        let mut adj: std::collections::HashMap<Uuid, Vec<Uuid>> = std::collections::HashMap::new();
        for mount in &world.mounts {
            let MountState::Project(project) = &mount.state else {
                continue;
            };
            for collection in &project.collections {
                for artifact in &collection.artifacts {
                    let src_info = match nodes.get(&artifact.metadata.uuid) {
                        Some(n) => n,
                        None => continue,
                    };
                    if !include_inactive && !src_info.active {
                        continue;
                    }
                    for link in &artifact.metadata.links {
                        if link.type_name != lt.name {
                            continue;
                        }
                        let Some(tgt_info) = nodes.get(&link.target_uuid) else {
                            continue; // unresolved — tracked by the other report
                        };
                        if !include_inactive && !tgt_info.active {
                            continue;
                        }
                        adj.entry(artifact.metadata.uuid)
                            .or_default()
                            .push(link.target_uuid);
                    }
                }
            }
        }

        // Iterative three-colour DFS. On a GRAY hit we extract the
        // loop slice (from the repeating UUID back to itself) as a
        // canonical cycle, dedupe by a rotation-invariant key, and
        // stop early once we've collected MAX_CYCLES_PER_LINK_TYPE
        // cycles for this link type.
        let mut color: std::collections::HashMap<Uuid, u8> = std::collections::HashMap::new();
        let mut seen_keys: std::collections::HashSet<Vec<Uuid>> = std::collections::HashSet::new();
        let mut type_cycles = 0usize;

        let sources: Vec<Uuid> = adj.keys().copied().collect();
        'type_loop: for root in sources {
            if matches!(color.get(&root), Some(&2)) {
                continue;
            }
            let mut stack: Vec<(Uuid, usize)> = vec![(root, 0)];
            let mut path: Vec<Uuid> = vec![root];
            color.insert(root, 1); // GRAY

            while let Some(&(node, idx)) = stack.last() {
                let children = adj.get(&node).map(|v| v.as_slice()).unwrap_or(&[]);
                if idx < children.len() {
                    let next = children[idx];
                    stack.last_mut().unwrap().1 = idx + 1;
                    match color.get(&next).copied() {
                        Some(1) => {
                            let cycle_start = path.iter().position(|n| *n == next).unwrap_or(0);
                            let raw: Vec<Uuid> = path[cycle_start..].to_vec();
                            let key = canonical_cycle_key(&raw);
                            let in_scope = raw.iter().any(|u| {
                                nodes
                                    .get(u)
                                    .map(|info| node_in_scope(info, scope))
                                    .unwrap_or(false)
                            });
                            if seen_keys.insert(key.clone()) && in_scope {
                                // Report the canonicalised rotation —
                                // stable regardless of which DFS root
                                // happened to discover the loop.
                                cycles.push(CycleEntry {
                                    link_type: lt.name.to_string(),
                                    nodes: key
                                        .iter()
                                        .filter_map(|u| nodes.get(u).map(|n| node_to_dto(*u, n)))
                                        .collect(),
                                });
                                type_cycles += 1;
                                if type_cycles >= MAX_CYCLES_PER_LINK_TYPE {
                                    truncated = true;
                                    break 'type_loop;
                                }
                            }
                        }
                        Some(2) => {}
                        _ => {
                            color.insert(next, 1);
                            path.push(next);
                            stack.push((next, 0));
                        }
                    }
                } else {
                    color.insert(node, 2);
                    path.pop();
                    stack.pop();
                }
            }
        }
    }

    // Stable ordering: by link type then by first node identity.
    cycles.sort_by(|a, b| {
        a.link_type.cmp(&b.link_type).then_with(|| {
            let a_key = a
                .nodes
                .first()
                .map(|n| {
                    format!(
                        "{}/{}/{}",
                        n.project_slug, n.collection_prefix, n.artifact_name
                    )
                })
                .unwrap_or_default();
            let b_key = b
                .nodes
                .first()
                .map(|n| {
                    format!(
                        "{}/{}/{}",
                        n.project_slug, n.collection_prefix, n.artifact_name
                    )
                })
                .unwrap_or_default();
            a_key.cmp(&b_key)
        })
    });

    Ok(CyclesReport {
        scope: ScopeDto::from(scope),
        link_types_checked: acyclic_types.iter().map(|t| t.name.to_string()).collect(),
        total_cycles: cycles.len(),
        truncated,
        cycles,
    })
}

/// Rotate a cycle slice so its smallest UUID is first. Two
/// traversals of the same loop can surface different starting
/// points; rotating to a canonical origin lets the hash-set
/// dedupe them cleanly.
fn canonical_cycle_key(raw: &[Uuid]) -> Vec<Uuid> {
    if raw.is_empty() {
        return Vec::new();
    }
    let min_idx = raw
        .iter()
        .enumerate()
        .min_by_key(|(_, u)| *u)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let mut out = Vec::with_capacity(raw.len());
    out.extend(raw.iter().cycle().skip(min_idx).take(raw.len()).copied());
    out
}

/// Conflicts report — pairs of artifacts related by the
/// `conflicts-with` link type, deduplicated by UUID-sorted
/// endpoints. A bidirectional-flag is set when both sides of the
/// pair declare the link, so operators can spot half-complete
/// pairings.
pub fn conflicts(
    scope: &Scope,
    include_inactive: bool,
    world: &World,
) -> Result<ConflictsReport, ReportError> {
    let _ = in_scope_artifacts(world, scope, include_inactive)?;
    let nodes = build_node_index(world);

    // Collect directed conflicts-with edges first so we can flag
    // the bidirectional ones post-hoc.
    let mut directed: std::collections::HashSet<(Uuid, Uuid)> = std::collections::HashSet::new();
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        for collection in &project.collections {
            for artifact in &collection.artifacts {
                let src_info = match nodes.get(&artifact.metadata.uuid) {
                    Some(n) => n,
                    None => continue,
                };
                if !include_inactive && !src_info.active {
                    continue;
                }
                for link in &artifact.metadata.links {
                    if link.type_name != "conflicts-with" {
                        continue;
                    }
                    let Some(tgt_info) = nodes.get(&link.target_uuid) else {
                        continue;
                    };
                    if !include_inactive && !tgt_info.active {
                        continue;
                    }
                    if artifact.metadata.uuid == link.target_uuid {
                        continue; // self-conflict is meaningless
                    }
                    directed.insert((artifact.metadata.uuid, link.target_uuid));
                }
            }
        }
    }

    let mut pairs: Vec<ConflictPair> = Vec::new();
    let mut seen: std::collections::HashSet<(Uuid, Uuid)> = std::collections::HashSet::new();
    for &(a, b) in &directed {
        let key = if a < b { (a, b) } else { (b, a) };
        if !seen.insert(key) {
            continue;
        }
        let (first, second) = (key.0, key.1);
        let Some(first_info) = nodes.get(&first) else {
            continue;
        };
        let Some(second_info) = nodes.get(&second) else {
            continue;
        };
        // Scope filter: keep the pair if either endpoint is in scope.
        if !node_in_scope(first_info, scope) && !node_in_scope(second_info, scope) {
            continue;
        }
        let bidirectional =
            directed.contains(&(first, second)) && directed.contains(&(second, first));
        pairs.push(ConflictPair {
            first: node_to_dto(first, first_info),
            second: node_to_dto(second, second_info),
            bidirectional,
        });
        if pairs.len() >= MAX_CONFLICT_PAIRS {
            break;
        }
    }

    pairs.sort_by(|a, b| {
        format!(
            "{}/{}/{}",
            a.first.project_slug, a.first.collection_prefix, a.first.artifact_name
        )
        .cmp(&format!(
            "{}/{}/{}",
            b.first.project_slug, b.first.collection_prefix, b.first.artifact_name
        ))
        .then_with(|| {
            format!(
                "{}/{}/{}",
                a.second.project_slug, a.second.collection_prefix, a.second.artifact_name
            )
            .cmp(&format!(
                "{}/{}/{}",
                b.second.project_slug, b.second.collection_prefix, b.second.artifact_name
            ))
        })
    });

    Ok(ConflictsReport {
        scope: ScopeDto::from(scope),
        total_pairs: pairs.len(),
        pairs,
    })
}

/// Coverage-matrix report — per in-scope parent, list the
/// artifacts that cover it via any of the configured covering
/// link types (`satisfies` + `verifies` by default). Parents
/// with zero covering children are flagged as gaps. Covering
/// edges are counted across the entire world, not just the
/// scoped slice, so a child in another collection still counts.
pub fn coverage_matrix(
    scope: &Scope,
    include_inactive: bool,
    query: &ReportQuery,
    world: &World,
) -> Result<CoverageMatrixReport, ReportError> {
    let scoped = in_scope_artifacts(world, scope, include_inactive)?;
    let nodes = build_node_index(world);

    // Resolve the covering link types against the effective
    // catalog. Unknown names get echoed back separately so the UI
    // can flag them without dropping the report.
    let catalog: HashSet<&str> = world.link_catalog.iter().map(|t| t.name).collect();
    let requested: Vec<String> = match query.covering_link_type_list() {
        Some(list) => list,
        None => DEFAULT_COVERING_LINK_TYPES
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    };
    let mut effective = Vec::new();
    let mut unknown = Vec::new();
    for name in requested {
        if catalog.contains(name.as_str()) {
            effective.push(name);
        } else {
            unknown.push(name);
        }
    }
    let effective_set: HashSet<&str> = effective.iter().map(|s| s.as_str()).collect();

    // Index covering incoming edges per target uuid → Vec<(source, link_type)>.
    let mut covering_incoming: HashMap<Uuid, Vec<(Uuid, String)>> = HashMap::new();
    if !effective_set.is_empty() {
        for mount in &world.mounts {
            let MountState::Project(project) = &mount.state else {
                continue;
            };
            for collection in &project.collections {
                for artifact in &collection.artifacts {
                    let src_info = match nodes.get(&artifact.metadata.uuid) {
                        Some(n) => n,
                        None => continue,
                    };
                    if !include_inactive && !src_info.active {
                        continue;
                    }
                    for link in &artifact.metadata.links {
                        if !effective_set.contains(link.type_name.as_str()) {
                            continue;
                        }
                        // Incoming-edge consumers check the target's
                        // own active flag at read time; we collect
                        // the edge regardless and filter output there.
                        covering_incoming
                            .entry(link.target_uuid)
                            .or_default()
                            .push((artifact.metadata.uuid, link.type_name.clone()));
                    }
                }
            }
        }
    }

    // Phase 9b: pre-compute the code-side evidence index once
    // per request. Keys are the `projectSlug/collectionPrefix/
    // artifactName` form the scanner emits so per-parent
    // lookup is O(1). Only tags whose verb (lowercased)
    // matches a configured covering link type count.
    let code_evidence = build_code_evidence_index(world, &effective_set);

    let mut parents = Vec::with_capacity(scoped.len());
    for s in &scoped {
        let parent_uuid = s.artifact.metadata.uuid;
        let parent_info = match nodes.get(&parent_uuid) {
            Some(info) => info,
            None => continue,
        };
        let mut children: Vec<CoverageChildEntry> = Vec::new();
        if let Some(edges) = covering_incoming.get(&parent_uuid) {
            for (src_uuid, link_type) in edges {
                let Some(child_info) = nodes.get(src_uuid) else {
                    continue;
                };
                if !include_inactive && !child_info.active {
                    continue;
                }
                children.push(CoverageChildEntry {
                    child: node_to_dto(*src_uuid, child_info),
                    link_type: link_type.clone(),
                });
            }
        }
        children.sort_by(|a, b| {
            a.link_type
                .cmp(&b.link_type)
                .then(a.child.project_slug.cmp(&b.child.project_slug))
                .then(a.child.collection_prefix.cmp(&b.child.collection_prefix))
                .then(a.child.artifact_name.cmp(&b.child.artifact_name))
        });
        let evidence_key = format!(
            "{}/{}/{}",
            parent_info.project_slug, parent_info.collection_prefix, parent_info.artifact_name
        );
        let covering_code_evidence: Vec<crate::reports::CoverageCodeEntry> = code_evidence
            .get(&evidence_key)
            .cloned()
            .unwrap_or_default();
        // Gap flag drops to false when either artifact-side
        // children or code-side evidence covers the parent
        // per the configured link-type set.
        let has_gap = children.is_empty() && covering_code_evidence.is_empty();
        parents.push(CoverageParentEntry {
            parent: node_to_dto(parent_uuid, parent_info),
            has_gap,
            covering_children: children,
            covering_code_evidence,
        });
    }
    parents.sort_by(|a, b| {
        a.parent
            .project_slug
            .cmp(&b.parent.project_slug)
            .then(a.parent.collection_prefix.cmp(&b.parent.collection_prefix))
            .then(a.parent.artifact_name.cmp(&b.parent.artifact_name))
    });
    let gap_count = parents.iter().filter(|p| p.has_gap).count();

    Ok(CoverageMatrixReport {
        scope: ScopeDto::from(scope),
        covering_link_types: effective,
        unknown_requested_types: unknown,
        total_parents: parents.len(),
        gap_count,
        parents,
    })
}

/// Impact-analysis report — BFS from a seed artifact along
/// traceability links. `direction = "dependents"` walks incoming
/// edges (who transitively points AT the seed); `"dependencies"`
/// walks outgoing (who the seed transitively points AT).
pub fn impact_analysis(
    scope: &Scope,
    include_inactive: bool,
    query: &ReportQuery,
    world: &World,
) -> Result<ImpactAnalysisReport, ReportError> {
    // Validate scope first so we 404 consistently before we look
    // at the seed. Output gets scope-filtered below.
    let _ = in_scope_artifacts(world, scope, include_inactive)?;
    let nodes = build_node_index(world);

    let direction_raw = query.direction.as_deref().unwrap_or("dependents");
    let walk_incoming = match direction_raw {
        "dependents" => true,
        "dependencies" => false,
        other => return Err(ReportError::InvalidDirection(other.to_owned())),
    };

    // Missing or unparseable seed → friendly missing-seed payload
    // rather than a 4xx. The UI guides the operator to pick one.
    let seed_uuid = match query.seed.as_deref().map(Uuid::parse_str) {
        Some(Ok(u)) => u,
        _ => {
            return Ok(ImpactAnalysisReport {
                scope: ScopeDto::from(scope),
                seed: None,
                direction: direction_raw.to_owned(),
                total_impacted: 0,
                impacted: Vec::new(),
                missing_seed_reason: Some(
                    "Pick a seed artifact to see its transitive impact.".to_owned(),
                ),
            });
        }
    };
    let Some(seed_info) = nodes.get(&seed_uuid) else {
        return Ok(ImpactAnalysisReport {
            scope: ScopeDto::from(scope),
            seed: None,
            direction: direction_raw.to_owned(),
            total_impacted: 0,
            impacted: Vec::new(),
            missing_seed_reason: Some(format!("Seed UUID {seed_uuid} is not currently mounted.")),
        });
    };

    // Build the directed edge set (honouring include_inactive on
    // both endpoints). Dependents walk uses the reversed
    // adjacency — from target → sources — which is more
    // conveniently computed directly here.
    let mut forward: HashMap<Uuid, Vec<(Uuid, String)>> = HashMap::new();
    let mut reverse: HashMap<Uuid, Vec<(Uuid, String)>> = HashMap::new();
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        for collection in &project.collections {
            for artifact in &collection.artifacts {
                let src_info = match nodes.get(&artifact.metadata.uuid) {
                    Some(n) => n,
                    None => continue,
                };
                if !include_inactive && !src_info.active {
                    continue;
                }
                for link in &artifact.metadata.links {
                    let Some(tgt_info) = nodes.get(&link.target_uuid) else {
                        continue;
                    };
                    if !include_inactive && !tgt_info.active {
                        continue;
                    }
                    forward
                        .entry(artifact.metadata.uuid)
                        .or_default()
                        .push((link.target_uuid, link.type_name.clone()));
                    reverse
                        .entry(link.target_uuid)
                        .or_default()
                        .push((artifact.metadata.uuid, link.type_name.clone()));
                }
            }
        }
    }
    let adj = if walk_incoming { &reverse } else { &forward };

    // BFS. Track depth per visited uuid, plus the set of link
    // types that arrived at it (deduped) so the UI can tell the
    // operator WHICH traceability trail runs through this node.
    let mut depths: HashMap<Uuid, usize> = HashMap::new();
    let mut link_types: HashMap<Uuid, std::collections::BTreeSet<String>> = HashMap::new();
    let mut queue: std::collections::VecDeque<Uuid> = std::collections::VecDeque::new();
    depths.insert(seed_uuid, 0);
    queue.push_back(seed_uuid);
    while let Some(current) = queue.pop_front() {
        let d = depths[&current];
        let Some(neighbours) = adj.get(&current) else {
            continue;
        };
        for (next, link_type) in neighbours {
            link_types
                .entry(*next)
                .or_default()
                .insert(link_type.clone());
            if depths.contains_key(next) {
                continue;
            }
            depths.insert(*next, d + 1);
            queue.push_back(*next);
            if depths.len() >= MAX_IMPACTED_ARTIFACTS {
                break;
            }
        }
        if depths.len() >= MAX_IMPACTED_ARTIFACTS {
            break;
        }
    }

    let mut impacted: Vec<ImpactedArtifact> = Vec::new();
    for (uuid, depth) in &depths {
        if *uuid == seed_uuid {
            continue; // exclude the seed itself
        }
        let Some(info) = nodes.get(uuid) else {
            continue;
        };
        if !node_in_scope(info, scope) {
            continue;
        }
        let lts: Vec<String> = link_types
            .get(uuid)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        impacted.push(ImpactedArtifact {
            node: node_to_dto(*uuid, info),
            depth: *depth,
            link_types: lts,
        });
    }
    impacted.sort_by(|a, b| {
        a.depth.cmp(&b.depth).then_with(|| {
            format!(
                "{}/{}/{}",
                a.node.project_slug, a.node.collection_prefix, a.node.artifact_name
            )
            .cmp(&format!(
                "{}/{}/{}",
                b.node.project_slug, b.node.collection_prefix, b.node.artifact_name
            ))
        })
    });

    Ok(ImpactAnalysisReport {
        scope: ScopeDto::from(scope),
        seed: Some(node_to_dto(seed_uuid, seed_info)),
        direction: direction_raw.to_owned(),
        total_impacted: impacted.len(),
        impacted,
        missing_seed_reason: None,
    })
}

/// Review-status report — aggregate approved / rejected /
/// re-requested / never-reviewed counts over the scoped artifact
/// set, plus per-project, per-collection, and per-shape
/// breakdowns. The derivation reuses
/// [`crate::reviews::state::derive_review_state`] so the counts
/// match whatever the review-queue sees.
pub fn review_status(
    scope: &Scope,
    include_inactive: bool,
    world: &World,
) -> Result<ReviewStatusReport, ReportError> {
    use crate::reviews::state::{ReviewState, derive_review_state};

    let scoped = in_scope_artifacts(world, scope, include_inactive)?;

    let mut totals = ReviewStatusCounts::default();
    let mut by_project: HashMap<String, ReviewStatusCounts> = HashMap::new();
    let mut by_collection: HashMap<(String, String), ReviewStatusCounts> = HashMap::new();
    let mut by_shape = ReviewStatusByShape::default();

    fn bump(counts: &mut ReviewStatusCounts, state: ReviewState) {
        match state {
            ReviewState::Approved => counts.approved += 1,
            ReviewState::Rejected => counts.rejected += 1,
            ReviewState::ReRequested => counts.re_requested += 1,
            ReviewState::NeverReviewed => counts.never_reviewed += 1,
        }
    }

    for s in &scoped {
        let state = derive_review_state(&s.artifact.metadata.review_log).state;
        bump(&mut totals, state);
        bump(
            by_project.entry(s.project_slug.to_owned()).or_default(),
            state,
        );
        bump(
            by_collection
                .entry((s.project_slug.to_owned(), s.collection_prefix.to_owned()))
                .or_default(),
            state,
        );
        match s.artifact.metadata.shape {
            crate::schema::ArtifactShape::Content => bump(&mut by_shape.content, state),
            crate::schema::ArtifactShape::Blob => bump(&mut by_shape.blob, state),
            crate::schema::ArtifactShape::Url => bump(&mut by_shape.url, state),
        }
    }

    let mut by_project_vec: Vec<ReviewStatusByProject> = by_project
        .into_iter()
        .map(|(project_slug, counts)| ReviewStatusByProject {
            project_slug,
            counts,
        })
        .collect();
    by_project_vec.sort_by(|a, b| a.project_slug.cmp(&b.project_slug));

    let mut by_collection_vec: Vec<ReviewStatusByCollection> = by_collection
        .into_iter()
        .map(
            |((project_slug, collection_prefix), counts)| ReviewStatusByCollection {
                project_slug,
                collection_prefix,
                counts,
            },
        )
        .collect();
    by_collection_vec.sort_by(|a, b| {
        a.project_slug
            .cmp(&b.project_slug)
            .then(a.collection_prefix.cmp(&b.collection_prefix))
    });

    Ok(ReviewStatusReport {
        scope: ScopeDto::from(scope),
        totals,
        by_project: by_project_vec,
        by_collection: by_collection_vec,
        by_shape,
    })
}

/// Filesystem-orphans report — walks the on-disk blob-holding
/// collections and surfaces pairing mismatches per REPORT-orphans.
/// Not cached; the walk runs on every report view (cheap stat-
/// only work, sub-100 ms on realistic repos).
pub fn filesystem_orphans(
    scope: &Scope,
    world: &World,
) -> Result<FilesystemOrphansReport, ReportError> {
    use crate::load::blob_allowlist::{is_allowed_blob_extension, media_type_for_extension};
    use crate::schema::sidecar::{SIDECAR_SUFFIX, blob_path_for_sidecar, sidecar_path_for_blob};

    // The walker doesn't touch the world's in-memory artifact
    // index — it lists the on-disk filesystem directly so
    // operator edits made outside ReqForge (a CLI `rm`, a git
    // checkout that drops a file) surface even before the next
    // discovery refresh.
    let mut missing_sidecar = Vec::new();
    let mut missing_binary = Vec::new();

    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        // Scope gate at the project level.
        match scope {
            Scope::System => {}
            Scope::Project(slug) | Scope::Collection { slug, .. } => {
                if project.config.slug != *slug {
                    continue;
                }
            }
        }
        for collection in &project.collections {
            if let Scope::Collection { prefix, .. } = scope
                && collection.config.prefix != *prefix
            {
                continue;
            }
            let dir = &collection.dir_path;
            let entries = match std::fs::read_dir(dir) {
                Ok(it) => it,
                Err(_) => continue,
            };
            let mut present_files: Vec<std::path::PathBuf> = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    present_files.push(path);
                }
            }

            for path in &present_files {
                let filename = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_owned();
                if filename.ends_with(SIDECAR_SUFFIX) || filename.starts_with('.') {
                    continue;
                }
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !is_allowed_blob_extension(ext) {
                    continue;
                }
                let sidecar = sidecar_path_for_blob(path);
                if sidecar.exists() {
                    continue;
                }
                let byte_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                let rel = path
                    .strip_prefix(&project.root)
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| filename.clone());
                missing_sidecar.push(OrphanBinary {
                    project_slug: project.config.slug.clone(),
                    collection_prefix: collection.config.prefix.clone(),
                    filename,
                    binary_relative_path: rel,
                    byte_size,
                    media_type: media_type_for_extension(ext),
                });
            }

            for path in &present_files {
                let filename = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_owned();
                if !filename.ends_with(SIDECAR_SUFFIX) || filename.starts_with('.') {
                    continue;
                }
                // Parse the sidecar to extract blobPath (URL-shape
                // sidecars have no peer file so they're not orphans).
                let text = match std::fs::read_to_string(path) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let Ok(meta) = serde_json::from_str::<crate::schema::Artifact>(&text) else {
                    continue;
                };
                if meta.shape != crate::schema::ArtifactShape::Blob {
                    continue;
                }
                let declared = match meta.blob_path.as_deref() {
                    Some(d) => d.to_owned(),
                    None => {
                        // Fall back to sibling resolution so a
                        // hand-authored sidecar without blob_path
                        // still flags sensibly.
                        match blob_path_for_sidecar(path) {
                            Some(p) => p
                                .strip_prefix(&project.root)
                                .map(|p| p.to_string_lossy().replace('\\', "/"))
                                .unwrap_or_else(|_| p.to_string_lossy().into_owned()),
                            None => continue,
                        }
                    }
                };
                let resolved = project.root.join(declared.replace('\\', "/"));
                if resolved.exists() {
                    continue;
                }
                missing_binary.push(OrphanSidecar {
                    project_slug: project.config.slug.clone(),
                    collection_prefix: collection.config.prefix.clone(),
                    sidecar_filename: filename,
                    declared_blob_path: declared,
                });
            }
        }
    }

    missing_sidecar.sort_by(|a, b| {
        a.project_slug
            .cmp(&b.project_slug)
            .then(a.collection_prefix.cmp(&b.collection_prefix))
            .then(a.filename.cmp(&b.filename))
    });
    missing_binary.sort_by(|a, b| {
        a.project_slug
            .cmp(&b.project_slug)
            .then(a.collection_prefix.cmp(&b.collection_prefix))
            .then(a.sidecar_filename.cmp(&b.sidecar_filename))
    });

    Ok(FilesystemOrphansReport {
        scope: ScopeDto::from(scope),
        missing_sidecar,
        missing_binary,
    })
}

// ---- Phase 9b: code-traceability report + coverage-matrix
// helper ----

/// Run the scanner over every mounted project and return a
/// `projectSlug/collectionPrefix/artifactName` → Vec of
/// `CoverageCodeEntry` index, filtered to tags whose verb
/// (lowercased) matches the effective covering link-type set.
fn build_code_evidence_index(
    world: &World,
    effective_set: &std::collections::HashSet<&str>,
) -> std::collections::HashMap<String, Vec<crate::reports::CoverageCodeEntry>> {
    let mut out: std::collections::HashMap<String, Vec<crate::reports::CoverageCodeEntry>> =
        std::collections::HashMap::new();
    if effective_set.is_empty() {
        return out;
    }
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        let scan = crate::scan::run_scan(project, world);
        for (key, tags) in scan.tags_by_artifact {
            for tag in tags {
                let lower = tag.verb.to_ascii_lowercase();
                if !effective_set.contains(lower.as_str()) {
                    continue;
                }
                out.entry(key.clone())
                    .or_default()
                    .push(crate::reports::CoverageCodeEntry {
                        file: tag.file,
                        line: tag.line,
                        verb: tag.verb,
                    });
            }
        }
    }
    // Stable sort per key so the UI sees deterministic
    // output across runs.
    for entries in out.values_mut() {
        entries.sort_by(|a, b| {
            a.file
                .cmp(&b.file)
                .then(a.line.cmp(&b.line))
                .then(a.verb.cmp(&b.verb))
        });
    }
    out
}

/// Build the code-traceability report per
/// REPORT-codeTraceability. For each in-scope artifact,
/// collects the scanner's tag locations grouped by canonical
/// verb; flags gaps (expects-code-trace AND no tags);
/// separately surfaces orphan tags (tags whose `(prefix,
/// name)` pair didn't resolve).
pub fn code_traceability(
    scope: &Scope,
    include_inactive: bool,
    world: &World,
) -> Result<crate::reports::CodeTraceabilityReport, ReportError> {
    let scoped = in_scope_artifacts(world, scope, include_inactive)?;
    let nodes = build_node_index(world);

    // Aggregate tags + orphans across every mounted project.
    // The cross-project resolution is already handled inside
    // `run_scan`; we just pool its output here.
    let mut tags_by_target: std::collections::HashMap<String, Vec<crate::scan::ScanTag>> =
        std::collections::HashMap::new();
    let mut orphan_tags: Vec<crate::scan::OrphanTag> = Vec::new();
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        let scan = crate::scan::run_scan(project, world);
        for (k, mut v) in scan.tags_by_artifact {
            tags_by_target.entry(k).or_default().append(&mut v);
        }
        orphan_tags.extend(scan.orphan_tags);
    }

    let mut entries: Vec<crate::reports::CodeTraceabilityEntry> = Vec::with_capacity(scoped.len());
    for s in &scoped {
        let uuid = s.artifact.metadata.uuid;
        let info = match nodes.get(&uuid) {
            Some(n) => n,
            None => continue,
        };
        let key = format!(
            "{}/{}/{}",
            info.project_slug, info.collection_prefix, info.artifact_name
        );
        let mut locations_by_verb: std::collections::BTreeMap<
            String,
            Vec<crate::reports::CodeTraceabilityLocation>,
        > = std::collections::BTreeMap::new();
        if let Some(tags) = tags_by_target.get(&key) {
            for tag in tags {
                locations_by_verb.entry(tag.verb.clone()).or_default().push(
                    crate::reports::CodeTraceabilityLocation {
                        file: tag.file.clone(),
                        line: tag.line,
                    },
                );
            }
        }
        for group in locations_by_verb.values_mut() {
            group.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
        }
        let expects_code_trace = effective_expects_code_trace(s, world);
        let has_gap = expects_code_trace && locations_by_verb.is_empty();
        entries.push(crate::reports::CodeTraceabilityEntry {
            artifact: node_to_dto(uuid, info),
            expects_code_trace,
            has_gap,
            locations_by_verb,
        });
    }
    entries.sort_by(|a, b| {
        a.artifact
            .project_slug
            .cmp(&b.artifact.project_slug)
            .then(
                a.artifact
                    .collection_prefix
                    .cmp(&b.artifact.collection_prefix),
            )
            .then(a.artifact.artifact_name.cmp(&b.artifact.artifact_name))
    });
    // Deduplicate orphans in case two projects share the scan
    // tree (typically they don't, but the pool is from
    // run_scan output which already rooted them at the
    // project — a single-project repo emits each orphan once).
    // Additionally, stable-sort so the report is
    // deterministic.
    orphan_tags.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.verb.cmp(&b.verb))
            .then(a.raw_id.cmp(&b.raw_id))
    });
    orphan_tags.dedup_by(|a, b| {
        a.file == b.file && a.line == b.line && a.verb == b.verb && a.raw_id == b.raw_id
    });

    let uncovered_count = entries.iter().filter(|e| e.has_gap).count();
    let orphan_tag_count = orphan_tags.len();
    let total_artifacts = entries.len();
    Ok(crate::reports::CodeTraceabilityReport {
        scope: ScopeDto::from(scope),
        total_artifacts,
        uncovered_count,
        orphan_tag_count,
        entries,
        orphan_tags: orphan_tags
            .into_iter()
            .map(|o| crate::reports::CodeTraceabilityOrphan {
                file: o.file,
                line: o.line,
                verb: o.verb,
                raw_id: o.raw_id,
            })
            .collect(),
    })
}

/// Resolve the effective `expectsCodeTrace` per the Phase 4
/// precedence: artifact-level override wins, otherwise the
/// Collection's `effective_expects_code_trace` (which defaults
/// to `true` when absent). Walks the world to find the
/// containing Collection since the `ScopedArtifact` helper
/// doesn't carry its config.
fn effective_expects_code_trace(scoped: &ScopedArtifact<'_>, world: &World) -> bool {
    if let Some(flag) = scoped.artifact.metadata.expects_code_trace {
        return flag;
    }
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        if project.config.slug != scoped.project_slug {
            continue;
        }
        for collection in &project.collections {
            if collection.config.prefix == scoped.collection_prefix {
                return collection.config.effective_expects_code_trace();
            }
        }
    }
    true
}

#[cfg(test)]
pub(crate) mod test_support {
    //! Fixture builders for the unit tests that sit alongside the
    //! compute code. Integration tests rebuild the World via the
    //! real discovery pipeline.

    use crate::links::builtin_catalog;
    use crate::load::LoadedArtifact;
    use crate::mount::{MountInfo, MountState};
    use crate::schema::{Artifact, ArtifactShape, CollectionConfig, Link, LinkHint, ProjectConfig};
    use crate::system::LoadedSystem;
    use crate::world::World;
    use chrono::Utc;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use uuid::Uuid;

    pub fn make_artifact(
        name: &str,
        uuid: Uuid,
        title: &str,
        shape: ArtifactShape,
        links: Vec<Link>,
        active: Option<bool>,
    ) -> LoadedArtifact {
        LoadedArtifact {
            name: name.to_owned(),
            source_path: PathBuf::from(format!("/tmp/{}", name)),
            metadata: Artifact {
                schema_version: 1,
                uuid,
                title: title.to_owned(),
                shape,
                created_at: Utc::now(),
                modified_at: Utc::now(),
                links,
                review_log: Vec::new(),
                description: None,
                expects_code_trace: None,
                active,
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

    pub fn link(target: Uuid, type_name: &str, hint: LinkHint) -> Link {
        Link {
            target_uuid: target,
            type_name: type_name.to_owned(),
            hint,
            overflow: BTreeMap::new(),
        }
    }

    pub fn hint(slug: &str, prefix: &str, name: &str) -> LinkHint {
        LinkHint {
            project_slug: slug.to_owned(),
            collection_prefix: prefix.to_owned(),
            artifact_name: name.to_owned(),
            overflow: BTreeMap::new(),
        }
    }

    pub fn make_world(
        project_slug: &str,
        project_root: PathBuf,
        collections: Vec<(String, String, Vec<LoadedArtifact>)>,
    ) -> World {
        let project = crate::load::LoadedProject {
            root: project_root.clone(),
            config: ProjectConfig {
                schema_version: 1,
                slug: project_slug.to_owned(),
                name: project_slug.to_owned(),
                description: None,
                artifacts_path: None,
                scan_paths: None,
                overflow: BTreeMap::new(),
            },
            collections: collections
                .into_iter()
                .map(|(dir, prefix, artifacts)| crate::load::LoadedCollection {
                    dir_name: dir.clone(),
                    dir_path: project_root.join(&dir),
                    config: CollectionConfig {
                        schema_version: 1,
                        prefix: prefix.clone(),
                        name: prefix.clone(),
                        description: None,
                        expects_code_trace: None,
                        import_notes: None,
                        overflow: BTreeMap::new(),
                    },
                    artifacts,
                })
                .collect(),
            diagnostics: Vec::new(),
        };
        let mount = MountInfo {
            path: project_root,
            state: MountState::Project(project),
        };
        let mounts = vec![mount];
        let loaded: Vec<&_> = mounts
            .iter()
            .filter_map(|m| match &m.state {
                MountState::Project(p) => Some(p),
                _ => None,
            })
            .collect();
        let (index, duplicates) = crate::index::build_uuid_index(&loaded);
        let search_index = crate::search::SearchIndex::build(&mounts)
            .map(std::sync::Arc::new)
            .expect("test search index build should succeed");
        World {
            mounts,
            index,
            duplicates,
            system: LoadedSystem::Unnamed,
            missing_project_slugs: Vec::new(),
            link_catalog: builtin_catalog().to_vec(),
            search_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::schema::ArtifactShape;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn uuid(seed: u8) -> Uuid {
        let mut bytes = [0u8; 16];
        bytes[15] = seed;
        // Flip the top nibble of byte 6 to match UUID v4-ish shape
        // so the tostring round-trip is legal — actual version
        // doesn't matter for tests.
        bytes[6] = 0x70 | seed;
        Uuid::from_bytes(bytes)
    }

    #[test]
    fn unresolved_links_flags_missing_target_with_hint() {
        let target = uuid(99); // NOT added to any artifact
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(
                target,
                "derives-from",
                hint("sample", "REQ", "REQ-ghost"),
            )],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a])],
        );
        let report = unresolved_links(&Scope::System, false, &world).unwrap();
        assert_eq!(report.total_unresolved, 1);
        assert_eq!(report.entries[0].reason, "target-missing");
        assert_eq!(
            report.entries[0].target_hint_artifact_name.as_str(),
            "REQ-ghost"
        );
    }

    #[test]
    fn unresolved_links_classifies_mount_missing_when_project_not_loaded() {
        let target = uuid(99);
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(
                target,
                "derives-from",
                hint("other-project", "DES", "DES-xyz"),
            )],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a])],
        );
        let report = unresolved_links(&Scope::System, false, &world).unwrap();
        assert_eq!(report.entries[0].reason, "mount-missing");
    }

    #[test]
    fn unresolved_links_excludes_inactive_sources_by_default() {
        let target = uuid(99);
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(
                target,
                "derives-from",
                hint("sample", "REQ", "REQ-ghost"),
            )],
            Some(false), // inactive
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a])],
        );
        let report = unresolved_links(&Scope::System, false, &world).unwrap();
        assert_eq!(report.total_unresolved, 0);
    }

    #[test]
    fn unresolved_links_includes_inactive_when_flag_set() {
        let target = uuid(99);
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(
                target,
                "derives-from",
                hint("sample", "REQ", "REQ-ghost"),
            )],
            Some(false),
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a])],
        );
        let report = unresolved_links(&Scope::System, true, &world).unwrap();
        assert_eq!(report.total_unresolved, 1);
    }

    #[test]
    fn link_orphans_reports_artifacts_with_zero_in_and_out_edges() {
        let orphan = make_artifact(
            "REQ-solo",
            uuid(1),
            "Solo",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let linked = make_artifact(
            "REQ-linked",
            uuid(2),
            "Linked",
            ArtifactShape::Content,
            vec![link(
                uuid(3),
                "derives-from",
                hint("sample", "REQ", "REQ-target"),
            )],
            None,
        );
        let target = make_artifact(
            "REQ-target",
            uuid(3),
            "Target",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![(
                "requirements".into(),
                "REQ".into(),
                vec![orphan, linked, target],
            )],
        );
        let report = link_orphans(&Scope::System, false, &world).unwrap();
        assert_eq!(report.total_orphans, 1);
        assert_eq!(report.entries[0].artifact_name, "REQ-solo");
    }

    #[test]
    fn link_orphans_scope_filter_still_counts_cross_scope_incoming_edges() {
        // REQ-target would be an orphan looking at only DES, but REQ
        // has an edge pointing at it; scope-collection:DES must NOT
        // report REQ-target because we count incoming globally.
        let des_artifact = make_artifact(
            "DES-foo",
            uuid(1),
            "Foo",
            ArtifactShape::Content,
            vec![link(
                uuid(2),
                "derives-from",
                hint("sample", "REQ", "REQ-bar"),
            )],
            None,
        );
        let req_target = make_artifact(
            "REQ-bar",
            uuid(2),
            "Bar",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![
                ("designs".into(), "DES".into(), vec![des_artifact]),
                ("requirements".into(), "REQ".into(), vec![req_target]),
            ],
        );
        let req_scope = Scope::Collection {
            slug: "sample".into(),
            prefix: "REQ".into(),
        };
        let report = link_orphans(&req_scope, false, &world).unwrap();
        assert_eq!(report.total_orphans, 0);
    }

    #[test]
    fn scope_unknown_project_returns_typed_error() {
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![])],
        );
        let result = unresolved_links(&Scope::Project("missing".into()), false, &world);
        assert!(matches!(result, Err(ReportError::ProjectNotMounted(_))));
    }

    // -------- Cycles --------

    #[test]
    fn cycles_finds_a_derives_from_three_cycle() {
        // A -> B -> C -> A on `derives-from` (acyclic-declared).
        let h = |name: &str| hint("sample", "REQ", name);
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(uuid(2), "derives-from", h("REQ-b"))],
            None,
        );
        let b = make_artifact(
            "REQ-b",
            uuid(2),
            "B",
            ArtifactShape::Content,
            vec![link(uuid(3), "derives-from", h("REQ-c"))],
            None,
        );
        let c = make_artifact(
            "REQ-c",
            uuid(3),
            "C",
            ArtifactShape::Content,
            vec![link(uuid(1), "derives-from", h("REQ-a"))],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b, c])],
        );
        let report = cycles(&Scope::System, false, &world).unwrap();
        assert_eq!(report.total_cycles, 1);
        assert_eq!(report.cycles[0].link_type, "derives-from");
        assert_eq!(report.cycles[0].nodes.len(), 3);
        // The cycle should be canonicalised so the smallest UUID
        // leads — uuid(1) is the smallest by the test's seed rule.
        assert_eq!(report.cycles[0].nodes[0].uuid, uuid(1));
    }

    #[test]
    fn cycles_ignores_non_acyclic_link_types() {
        // `satisfies` is directed but NOT acyclic — the same
        // A->B->A shape must not produce a cycle entry.
        let h = |name: &str| hint("sample", "REQ", name);
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(uuid(2), "satisfies", h("REQ-b"))],
            None,
        );
        let b = make_artifact(
            "REQ-b",
            uuid(2),
            "B",
            ArtifactShape::Content,
            vec![link(uuid(1), "satisfies", h("REQ-a"))],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b])],
        );
        let report = cycles(&Scope::System, false, &world).unwrap();
        assert_eq!(report.total_cycles, 0);
        assert!(
            report
                .link_types_checked
                .iter()
                .any(|t| t == "derives-from")
        );
        assert!(!report.link_types_checked.iter().any(|t| t == "satisfies"));
    }

    #[test]
    fn cycles_dedupes_the_same_loop_regardless_of_start_node() {
        // A->B->C->A. DFS starting from any of A/B/C must dedupe
        // down to exactly one cycle entry.
        let h = |name: &str| hint("sample", "REQ", name);
        let mk = |name: &str, u, target, target_name: &'static str| {
            make_artifact(
                name,
                u,
                name,
                ArtifactShape::Content,
                vec![link(target, "derives-from", h(target_name))],
                None,
            )
        };
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![(
                "requirements".into(),
                "REQ".into(),
                vec![
                    mk("REQ-a", uuid(1), uuid(2), "REQ-b"),
                    mk("REQ-b", uuid(2), uuid(3), "REQ-c"),
                    mk("REQ-c", uuid(3), uuid(1), "REQ-a"),
                ],
            )],
        );
        let report = cycles(&Scope::System, false, &world).unwrap();
        assert_eq!(report.total_cycles, 1);
    }

    #[test]
    fn cycles_excludes_inactive_endpoints_when_flag_off() {
        let h = |name: &str| hint("sample", "REQ", name);
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(uuid(2), "derives-from", h("REQ-b"))],
            None,
        );
        let b = make_artifact(
            "REQ-b",
            uuid(2),
            "B",
            ArtifactShape::Content,
            vec![link(uuid(1), "derives-from", h("REQ-a"))],
            Some(false),
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b])],
        );
        let off = cycles(&Scope::System, false, &world).unwrap();
        assert_eq!(off.total_cycles, 0);
        let on = cycles(&Scope::System, true, &world).unwrap();
        assert_eq!(on.total_cycles, 1);
    }

    // -------- Conflicts --------

    #[test]
    fn conflicts_deduplicates_pairs_and_flags_bidirectional_edges() {
        let h = |name: &str| hint("sample", "REQ", name);
        // REQ-a and REQ-b each declare conflicts-with the other.
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(uuid(2), "conflicts-with", h("REQ-b"))],
            None,
        );
        let b = make_artifact(
            "REQ-b",
            uuid(2),
            "B",
            ArtifactShape::Content,
            vec![link(uuid(1), "conflicts-with", h("REQ-a"))],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b])],
        );
        let report = conflicts(&Scope::System, false, &world).unwrap();
        assert_eq!(report.total_pairs, 1);
        assert!(report.pairs[0].bidirectional);
        // UUID-sorted: uuid(1) < uuid(2).
        assert_eq!(report.pairs[0].first.uuid, uuid(1));
        assert_eq!(report.pairs[0].second.uuid, uuid(2));
    }

    #[test]
    fn conflicts_single_directional_edge_still_surfaces_the_pair() {
        let h = |name: &str| hint("sample", "REQ", name);
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(uuid(2), "conflicts-with", h("REQ-b"))],
            None,
        );
        let b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b])],
        );
        let report = conflicts(&Scope::System, false, &world).unwrap();
        assert_eq!(report.total_pairs, 1);
        assert!(!report.pairs[0].bidirectional);
    }

    #[test]
    fn conflicts_scope_filter_keeps_pair_when_any_endpoint_matches() {
        // DES-a conflicts-with REQ-b. Scope=Collection REQ should
        // still keep the pair because REQ-b is in scope.
        let a = make_artifact(
            "DES-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(
                uuid(2),
                "conflicts-with",
                hint("sample", "REQ", "REQ-b"),
            )],
            None,
        );
        let b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![
                ("designs".into(), "DES".into(), vec![a]),
                ("requirements".into(), "REQ".into(), vec![b]),
            ],
        );
        let scoped = conflicts(
            &Scope::Collection {
                slug: "sample".into(),
                prefix: "REQ".into(),
            },
            false,
            &world,
        )
        .unwrap();
        assert_eq!(scoped.total_pairs, 1);
    }

    #[test]
    fn conflicts_scope_excludes_pair_when_no_endpoint_matches() {
        // Same shape, but scope DES and the pair should survive
        // (DES-a is in scope). Then scope to a bogus project:
        // 404.
        let a = make_artifact(
            "DES-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(
                uuid(2),
                "conflicts-with",
                hint("sample", "REQ", "REQ-b"),
            )],
            None,
        );
        let b = make_artifact("REQ-b", uuid(2), "B", ArtifactShape::Content, vec![], None);
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![
                ("designs".into(), "DES".into(), vec![a]),
                ("requirements".into(), "REQ".into(), vec![b]),
            ],
        );
        assert!(matches!(
            conflicts(&Scope::Project("nowhere".into()), false, &world),
            Err(ReportError::ProjectNotMounted(_))
        ));
    }

    #[test]
    fn conflicts_ignores_self_conflicts_by_contract() {
        // A.conflicts-with = [A] — meaningless; don't report.
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(
                uuid(1),
                "conflicts-with",
                hint("sample", "REQ", "REQ-a"),
            )],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a])],
        );
        let report = conflicts(&Scope::System, false, &world).unwrap();
        assert_eq!(report.total_pairs, 0);
    }

    // -------- Coverage matrix --------

    fn req_query(covering: Option<&str>) -> super::super::ReportQuery {
        super::super::ReportQuery {
            scope: None,
            include_inactive: None,
            covering_link_types: covering.map(|s| s.to_owned()),
            seed: None,
            direction: None,
        }
    }

    #[test]
    fn coverage_matrix_default_set_flags_uncovered_parents_as_gaps() {
        // REQ-login is covered by DES-loginForm (satisfies) but
        // not verified. REQ-logout has no covering children at
        // all. The default set is {satisfies, verifies}, so
        // REQ-login still has a gap via missing verify — but only
        // matters when the UI requires BOTH; the report here
        // reports a gap iff NO covering link hits the parent.
        let req_login = make_artifact(
            "REQ-login",
            uuid(1),
            "Login",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let req_logout = make_artifact(
            "REQ-logout",
            uuid(2),
            "Logout",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let des_login = make_artifact(
            "DES-loginForm",
            uuid(3),
            "Login form",
            ArtifactShape::Content,
            vec![link(
                uuid(1),
                "satisfies",
                hint("sample", "REQ", "REQ-login"),
            )],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![
                (
                    "requirements".into(),
                    "REQ".into(),
                    vec![req_login, req_logout],
                ),
                ("designs".into(), "DES".into(), vec![des_login]),
            ],
        );
        let scope = Scope::Collection {
            slug: "sample".into(),
            prefix: "REQ".into(),
        };
        let q = req_query(None);
        let report = coverage_matrix(&scope, false, &q, &world).unwrap();
        assert_eq!(report.covering_link_types, vec!["satisfies", "verifies"]);
        assert!(report.unknown_requested_types.is_empty());
        assert_eq!(report.total_parents, 2);
        assert_eq!(report.gap_count, 1);
        let login_row = report
            .parents
            .iter()
            .find(|p| p.parent.artifact_name == "REQ-login")
            .unwrap();
        assert!(!login_row.has_gap);
        assert_eq!(login_row.covering_children.len(), 1);
        assert_eq!(login_row.covering_children[0].link_type, "satisfies");
        let logout_row = report
            .parents
            .iter()
            .find(|p| p.parent.artifact_name == "REQ-logout")
            .unwrap();
        assert!(logout_row.has_gap);
    }

    #[test]
    fn coverage_matrix_custom_set_overrides_default() {
        // Override the covering set to {derives-from} so the
        // satisfies link no longer counts; REQ-login now has a
        // gap.
        let req_login = make_artifact(
            "REQ-login",
            uuid(1),
            "Login",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let des_login = make_artifact(
            "DES-loginForm",
            uuid(3),
            "Login form",
            ArtifactShape::Content,
            vec![link(
                uuid(1),
                "satisfies",
                hint("sample", "REQ", "REQ-login"),
            )],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![
                ("requirements".into(), "REQ".into(), vec![req_login]),
                ("designs".into(), "DES".into(), vec![des_login]),
            ],
        );
        let scope = Scope::Collection {
            slug: "sample".into(),
            prefix: "REQ".into(),
        };
        let q = req_query(Some("derives-from"));
        let report = coverage_matrix(&scope, false, &q, &world).unwrap();
        assert_eq!(report.covering_link_types, vec!["derives-from"]);
        assert_eq!(report.gap_count, 1);
    }

    #[test]
    fn coverage_matrix_echoes_unknown_requested_types_separately() {
        let req_login = make_artifact(
            "REQ-login",
            uuid(1),
            "Login",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![req_login])],
        );
        let q = req_query(Some("satisfies,bogus-type"));
        let report = coverage_matrix(&Scope::System, false, &q, &world).unwrap();
        assert_eq!(report.covering_link_types, vec!["satisfies"]);
        assert_eq!(report.unknown_requested_types, vec!["bogus-type"]);
    }

    // -------- Impact analysis --------

    fn impact_query(seed: Option<Uuid>, direction: Option<&str>) -> super::super::ReportQuery {
        super::super::ReportQuery {
            scope: None,
            include_inactive: None,
            covering_link_types: None,
            seed: seed.map(|u| u.to_string()),
            direction: direction.map(|s| s.to_owned()),
        }
    }

    #[test]
    fn impact_analysis_missing_seed_returns_friendly_banner_not_error() {
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![])],
        );
        let q = impact_query(None, None);
        let report = impact_analysis(&Scope::System, false, &q, &world).unwrap();
        assert!(report.seed.is_none());
        assert!(report.missing_seed_reason.is_some());
        assert_eq!(report.total_impacted, 0);
    }

    #[test]
    fn impact_analysis_dependents_walks_incoming_edges_transitively() {
        // A <- B <- C on derives-from. Seed A; dependents = {B, C}.
        let a = make_artifact("REQ-a", uuid(1), "A", ArtifactShape::Content, vec![], None);
        let b = make_artifact(
            "REQ-b",
            uuid(2),
            "B",
            ArtifactShape::Content,
            vec![link(
                uuid(1),
                "derives-from",
                hint("sample", "REQ", "REQ-a"),
            )],
            None,
        );
        let c = make_artifact(
            "REQ-c",
            uuid(3),
            "C",
            ArtifactShape::Content,
            vec![link(
                uuid(2),
                "derives-from",
                hint("sample", "REQ", "REQ-b"),
            )],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b, c])],
        );
        let q = impact_query(Some(uuid(1)), Some("dependents"));
        let report = impact_analysis(&Scope::System, false, &q, &world).unwrap();
        assert_eq!(report.total_impacted, 2);
        assert_eq!(report.direction, "dependents");
        // B is depth 1, C is depth 2.
        let b_row = report
            .impacted
            .iter()
            .find(|e| e.node.artifact_name == "REQ-b")
            .unwrap();
        let c_row = report
            .impacted
            .iter()
            .find(|e| e.node.artifact_name == "REQ-c")
            .unwrap();
        assert_eq!(b_row.depth, 1);
        assert_eq!(c_row.depth, 2);
    }

    #[test]
    fn impact_analysis_dependencies_walks_outgoing_edges() {
        // A -> B -> C on derives-from. Seed A; dependencies = {B, C}.
        let a = make_artifact(
            "REQ-a",
            uuid(1),
            "A",
            ArtifactShape::Content,
            vec![link(
                uuid(2),
                "derives-from",
                hint("sample", "REQ", "REQ-b"),
            )],
            None,
        );
        let b = make_artifact(
            "REQ-b",
            uuid(2),
            "B",
            ArtifactShape::Content,
            vec![link(
                uuid(3),
                "derives-from",
                hint("sample", "REQ", "REQ-c"),
            )],
            None,
        );
        let c = make_artifact("REQ-c", uuid(3), "C", ArtifactShape::Content, vec![], None);
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![a, b, c])],
        );
        let q = impact_query(Some(uuid(1)), Some("dependencies"));
        let report = impact_analysis(&Scope::System, false, &q, &world).unwrap();
        assert_eq!(report.total_impacted, 2);
        let names: Vec<&str> = report
            .impacted
            .iter()
            .map(|e| e.node.artifact_name.as_str())
            .collect();
        assert!(names.contains(&"REQ-b"));
        assert!(names.contains(&"REQ-c"));
    }

    #[test]
    fn impact_analysis_bad_direction_surfaces_typed_error() {
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![("requirements".into(), "REQ".into(), vec![])],
        );
        let q = impact_query(Some(uuid(1)), Some("sideways"));
        let err = impact_analysis(&Scope::System, false, &q, &world).unwrap_err();
        assert!(matches!(err, ReportError::InvalidDirection(_)));
    }

    #[test]
    fn impact_analysis_collects_link_types_used_per_impacted_node() {
        // A is reached from B via derives-from AND from C via
        // satisfies when seeding A with dependents direction.
        let a = make_artifact("REQ-a", uuid(1), "A", ArtifactShape::Content, vec![], None);
        let b = make_artifact(
            "REQ-b",
            uuid(2),
            "B",
            ArtifactShape::Content,
            vec![link(
                uuid(1),
                "derives-from",
                hint("sample", "REQ", "REQ-a"),
            )],
            None,
        );
        let c = make_artifact(
            "DES-c",
            uuid(3),
            "C",
            ArtifactShape::Content,
            vec![link(uuid(1), "satisfies", hint("sample", "REQ", "REQ-a"))],
            None,
        );
        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![
                ("requirements".into(), "REQ".into(), vec![a, b]),
                ("designs".into(), "DES".into(), vec![c]),
            ],
        );
        let q = impact_query(Some(uuid(1)), None);
        let report = impact_analysis(&Scope::System, false, &q, &world).unwrap();
        let b_row = report
            .impacted
            .iter()
            .find(|e| e.node.artifact_name == "REQ-b")
            .unwrap();
        assert_eq!(b_row.link_types, vec!["derives-from"]);
        let c_row = report
            .impacted
            .iter()
            .find(|e| e.node.artifact_name == "DES-c")
            .unwrap();
        assert_eq!(c_row.link_types, vec!["satisfies"]);
    }

    // -------- Review status --------

    fn review_log_entry(outcome: &str) -> crate::schema::ReviewLogEntry {
        crate::schema::ReviewLogEntry {
            timestamp: chrono::Utc::now(),
            reviewer: "t@t".to_owned(),
            outcome: outcome.to_owned(),
            explanation: None,
            added_todos: Vec::new(),
            resolved_todos: Vec::new(),
            overflow: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn review_status_aggregates_totals_plus_facets() {
        let mut approved = make_artifact(
            "REQ-ok",
            uuid(1),
            "OK",
            ArtifactShape::Content,
            vec![],
            None,
        );
        approved.metadata.review_log = vec![review_log_entry("approved")];
        let never = make_artifact(
            "REQ-fresh",
            uuid(2),
            "Fresh",
            ArtifactShape::Content,
            vec![],
            None,
        );
        let mut rejected = make_artifact(
            "DES-nope",
            uuid(3),
            "Nope",
            ArtifactShape::Content,
            vec![],
            None,
        );
        rejected.metadata.review_log = vec![review_log_entry("rejected")];

        let world = make_world(
            "sample",
            PathBuf::from("/tmp/sample"),
            vec![
                ("requirements".into(), "REQ".into(), vec![approved, never]),
                ("designs".into(), "DES".into(), vec![rejected]),
            ],
        );
        let report = review_status(&Scope::System, false, &world).unwrap();
        assert_eq!(report.totals.approved, 1);
        assert_eq!(report.totals.rejected, 1);
        assert_eq!(report.totals.never_reviewed, 1);
        assert_eq!(report.totals.total(), 3);
        let req_counts = report
            .by_collection
            .iter()
            .find(|c| c.collection_prefix == "REQ")
            .unwrap();
        assert_eq!(req_counts.counts.approved, 1);
        assert_eq!(req_counts.counts.never_reviewed, 1);
        assert_eq!(report.by_shape.content.approved, 1);
        assert_eq!(report.by_shape.content.rejected, 1);
    }

    // -------- Filesystem orphans --------

    #[test]
    fn filesystem_orphans_reports_binary_without_sidecar() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path().join("sample");
        let collection_dir = project_root.join("artifacts/DES");
        std::fs::create_dir_all(&collection_dir).unwrap();
        // Stray PNG, no sidecar.
        std::fs::write(collection_dir.join("DES-logo.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        let world = make_world(
            "sample",
            project_root.clone(),
            vec![("DES".into(), "DES".into(), vec![])],
        );
        // make_world puts the collection at <root>/<dir>, but the
        // real discovery pipeline puts it under artifacts/<dir>;
        // patch the collection dir_path to match our on-disk
        // layout.
        let mut patched = world;
        if let crate::mount::MountState::Project(ref mut p) = patched.mounts[0].state {
            p.collections[0].dir_path = collection_dir.clone();
        }
        let report = filesystem_orphans(&Scope::System, &patched).unwrap();
        assert_eq!(report.missing_sidecar.len(), 1);
        assert_eq!(report.missing_sidecar[0].filename, "DES-logo.png");
        assert_eq!(report.missing_sidecar[0].media_type, "image/png");
        assert!(report.missing_binary.is_empty());
    }

    #[test]
    fn filesystem_orphans_reports_sidecar_whose_blob_path_does_not_resolve() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path().join("sample");
        let collection_dir = project_root.join("artifacts/DES");
        std::fs::create_dir_all(&collection_dir).unwrap();
        // Sidecar exists; blob_path points at a file that doesn't.
        let sidecar_path = collection_dir.join("DES-ghost.pdf.reqforge.json");
        let meta = serde_json::json!({
            "schemaVersion": 1,
            "uuid": "0194f6d0-0000-7000-8000-000000000001",
            "title": "Ghost",
            "shape": "blob",
            "createdAt": "2026-04-22T00:00:00Z",
            "modifiedAt": "2026-04-22T00:00:00Z",
            "links": [],
            "reviewLog": [],
            "blobPath": "artifacts/DES/DES-ghost.pdf",
        });
        std::fs::write(&sidecar_path, meta.to_string()).unwrap();
        let world = make_world(
            "sample",
            project_root.clone(),
            vec![("DES".into(), "DES".into(), vec![])],
        );
        let mut patched = world;
        if let crate::mount::MountState::Project(ref mut p) = patched.mounts[0].state {
            p.collections[0].dir_path = collection_dir.clone();
        }
        let report = filesystem_orphans(&Scope::System, &patched).unwrap();
        assert_eq!(report.missing_binary.len(), 1);
        assert_eq!(
            report.missing_binary[0].sidecar_filename,
            "DES-ghost.pdf.reqforge.json"
        );
        assert_eq!(
            report.missing_binary[0].declared_blob_path,
            "artifacts/DES/DES-ghost.pdf"
        );
    }

    #[test]
    fn filesystem_orphans_ignores_extensions_outside_allowlist() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path().join("sample");
        let collection_dir = project_root.join("artifacts/DES");
        std::fs::create_dir_all(&collection_dir).unwrap();
        std::fs::write(collection_dir.join("README.txt"), b"hello").unwrap();
        let world = make_world(
            "sample",
            project_root.clone(),
            vec![("DES".into(), "DES".into(), vec![])],
        );
        let mut patched = world;
        if let crate::mount::MountState::Project(ref mut p) = patched.mounts[0].state {
            p.collections[0].dir_path = collection_dir.clone();
        }
        let report = filesystem_orphans(&Scope::System, &patched).unwrap();
        assert!(report.missing_sidecar.is_empty());
        assert!(report.missing_binary.is_empty());
    }
}
