//! Import execution (Phase 8.2).
//!
//! Takes an [`ImportPlan`] and writes every file it describes:
//! Collection sidecars, content-shape artifacts for each
//! imported doorstop item, URL-companion artifacts for
//! URL-shaped refs, and synthetic review entries on the
//! imported artifacts whose source carried a non-null
//! `reviewed` hash.
//!
//! Every write goes through the existing Phase 5a atomic
//! write path (`write::atomic_write` plus
//! `write::reconcile_ownership`) so an interrupted run leaves
//! no half-written files. The original doorstop source is
//! never touched (per INTEROP-doorstopPreserveOriginal).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::doorstop::plan::{ImportPlan, PlanArtifact, PlanCollection, PlanRefDisposition};
use crate::doorstop::report::{ImportReport, ReportCollection, ReportRefDisposition, ReportTotals};
use crate::schema::{Artifact, ArtifactShape, CollectionConfig, Link, LinkHint, ReviewLogEntry};

/// Minimal owned slice of a `LoadedProject` — the bits execute
/// actually needs. Keeps the handler free to detach from the
/// borrowed `World` before handing work to a blocking thread.
#[derive(Debug, Clone)]
pub struct ExecuteTarget {
    pub project_slug: String,
    pub project_root: PathBuf,
    pub artifacts_root: PathBuf,
}

impl ExecuteTarget {
    pub fn from_project(project: &crate::load::LoadedProject) -> Self {
        let artifacts_root = project.root.join(project.config.effective_artifacts_path());
        Self {
            project_slug: project.config.slug.clone(),
            project_root: project.root.clone(),
            artifacts_root,
        }
    }
}
use crate::write::OwnershipOverrides;
use crate::write::artifact_file::{WriteArtifactError, write_artifact_file};
use crate::write::atomic::{AtomicWriteError, atomic_write};
use crate::write::ownership::{OwnershipError, reconcile_ownership};

/// Execute a plan against a project on disk. Returns the
/// filled-in import report on success.
///
/// Refuses when the plan carries any prefix collisions — per
/// INTEROP-doorstopPrefixCollision the caller must resolve
/// them before re-running.
pub fn execute(
    target: &ExecuteTarget,
    source_label: &str,
    plan: ImportPlan,
    overrides: OwnershipOverrides,
) -> Result<ImportReport, ExecuteError> {
    if !plan.prefix_collisions.is_empty() {
        return Err(ExecuteError::PrefixCollision {
            prefixes: plan
                .prefix_collisions
                .iter()
                .map(|c| c.prefix.clone())
                .collect(),
        });
    }

    let project_root = target.project_root.clone();
    let artifacts_root = target.artifacts_root.clone();
    if !artifacts_root.exists() {
        // Phase 1 load requires artifacts/ to exist for the
        // project to mount; keep the runtime assertion here
        // so we fail loudly if a caller somehow reaches
        // execute with a project that shouldn't have mounted.
        return Err(ExecuteError::ArtifactsRootMissing {
            path: artifacts_root,
        });
    }

    let mut report_collections: Vec<ReportCollection> = Vec::new();
    let mut totals = ReportTotals {
        collections_created: 0,
        artifacts_imported: 0,
        derives_from_links: 0,
        url_artifacts: 0,
        cites_links: 0,
        legacy_refs: 0,
        synthetic_review_entries: 0,
        legacy_preserved_fields: 0,
        unresolved_link_count: plan.unresolved_links.len(),
    };
    let mut ref_dispositions: Vec<ReportRefDisposition> = Vec::new();

    for collection in &plan.collections {
        let collection_dir = artifacts_root.join(&collection.directory_name);
        std::fs::create_dir_all(&collection_dir).map_err(|source| {
            ExecuteError::CollectionDirCreate {
                path: collection_dir.clone(),
                source,
            }
        })?;

        // Write the Collection sidecar (`.collection.json`)
        // with doorstop settings preserved in importNotes.
        let config = CollectionConfig {
            schema_version: 1,
            prefix: collection.prefix.clone(),
            name: collection.name.clone(),
            description: None,
            expects_code_trace: None,
            import_notes: if collection.import_notes.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(
                    collection
                        .import_notes
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                ))
            },
            overflow: Default::default(),
        };
        write_collection_config(&collection_dir, &config, &project_root, overrides)?;

        // Write every artifact in the pane, in plan order.
        let mut artifact_count = 0usize;
        let mut synthetic_review_count = 0usize;
        let mut legacy_preserved_count = 0usize;
        let mut derives_from_link_count = 0usize;
        let mut url_artifact_count = 0usize;
        for artifact in &collection.artifacts {
            let file_path = collection_dir.join(format!("{}.md", artifact.name));
            let (metadata, body) = build_artifact_file(
                artifact,
                collection,
                &plan.import_run_at,
                &mut totals,
                &mut ref_dispositions,
            );
            if artifact.synthetic_review.is_some() {
                synthetic_review_count += 1;
            }
            if has_legacy_payload(artifact) {
                legacy_preserved_count += 1;
            }
            // Count derives-from + URL-companion-induced
            // changes only for *imported* doorstop items,
            // not the URL-companion artifacts themselves
            // (companions have empty original_uid).
            if !artifact.original_uid.is_empty() {
                derives_from_link_count += artifact
                    .links
                    .iter()
                    .filter(|l| l.link_type == "derives-from")
                    .count();
            } else {
                url_artifact_count += 1;
            }

            write_artifact_file(&file_path, &project_root, &metadata, &body, overrides)?;
            artifact_count += 1;
        }

        totals.collections_created += 1;
        totals.artifacts_imported += artifact_count;
        totals.synthetic_review_entries += synthetic_review_count;
        totals.legacy_preserved_fields += legacy_preserved_count;

        report_collections.push(ReportCollection {
            prefix: collection.prefix.clone(),
            name: collection.name.clone(),
            directory_name: collection.directory_name.clone(),
            artifact_count,
            synthetic_review_count,
            legacy_preserved_count,
            derives_from_link_count,
            url_artifact_count,
            source_marker_path: collection.source_marker_path.clone(),
        });
    }

    Ok(ImportReport {
        project_slug: target.project_slug.clone(),
        source: source_label.to_owned(),
        import_run_at: plan.import_run_at,
        collections: report_collections,
        totals,
        ref_dispositions,
        unresolved_links: plan.unresolved_links,
        prefix_collisions: plan.prefix_collisions,
        warnings: plan.warnings,
    })
}

fn write_collection_config(
    dir: &Path,
    config: &CollectionConfig,
    project_root: &Path,
    overrides: OwnershipOverrides,
) -> Result<(), ExecuteError> {
    let path = dir.join(".collection.json");
    let mut bytes = serde_json::to_vec_pretty(config)
        .map_err(|source| ExecuteError::SerializeCollection { source })?;
    bytes.push(b'\n');
    atomic_write(&path, &bytes)?;
    reconcile_ownership(&path, project_root, overrides)?;
    Ok(())
}

/// Assemble the `Artifact` metadata + rendered body for one
/// imported item. Counts mutate `totals` + `ref_dispositions`
/// as a side effect so the execute loop produces its report in
/// a single pass.
fn build_artifact_file(
    artifact: &PlanArtifact,
    collection: &PlanCollection,
    import_run_at: &chrono::DateTime<chrono::Utc>,
    totals: &mut ReportTotals,
    ref_dispositions: &mut Vec<ReportRefDisposition>,
) -> (Artifact, String) {
    let is_url_companion = artifact.original_uid.is_empty()
        && matches!(artifact.ref_disposition, PlanRefDisposition::None)
        && artifact.body.is_empty();

    let shape = if is_url_companion {
        ArtifactShape::Url
    } else {
        ArtifactShape::Content
    };

    // Link list: plan links map 1-to-1 to schema Links.
    let links: Vec<Link> = artifact
        .links
        .iter()
        .map(|pl| {
            if pl.link_type == "cites" {
                totals.cites_links += 1;
            }
            Link {
                target_uuid: pl.target_uuid,
                type_name: pl.link_type.clone(),
                hint: LinkHint {
                    project_slug: pl.hint.project_slug.clone(),
                    collection_prefix: pl.hint.collection_prefix.clone(),
                    artifact_name: pl.hint.artifact_name.clone(),
                    overflow: Default::default(),
                },
                overflow: Default::default(),
            }
        })
        .collect();

    // Synthetic review entry.
    let review_log: Vec<ReviewLogEntry> = artifact
        .synthetic_review
        .as_ref()
        .map(|r| {
            vec![ReviewLogEntry {
                timestamp: r.timestamp,
                reviewer: r.reviewer.clone(),
                outcome: r.outcome.clone(),
                explanation: Some(r.explanation.clone()),
                added_todos: Vec::new(),
                resolved_todos: Vec::new(),
                overflow: Default::default(),
            }]
        })
        .unwrap_or_default();

    // `legacy` assembly.
    //
    // - `doorstopUid` carries the original doorstop UID so the
    //   mapping from old → new name is recoverable.
    // - `ref` appears here only when the ref was non-URL.
    // - Extension fields flow through verbatim.
    let mut legacy: BTreeMap<String, serde_json::Value> = artifact.legacy_extensions.clone();
    if !artifact.original_uid.is_empty() {
        legacy.insert(
            "doorstopUid".into(),
            serde_json::Value::String(artifact.original_uid.clone()),
        );
    }
    if let PlanRefDisposition::Legacy { value } = &artifact.ref_disposition {
        legacy.insert("ref".into(), serde_json::Value::String(value.clone()));
        totals.legacy_refs += 1;
        ref_dispositions.push(ReportRefDisposition::Legacy {
            source_uid: artifact.original_uid.clone(),
            value: value.clone(),
        });
    }

    if let PlanRefDisposition::UrlArtifact {
        url,
        url_artifact_uuid: _,
        url_artifact_name,
    } = &artifact.ref_disposition
    {
        totals.url_artifacts += 1;
        ref_dispositions.push(ReportRefDisposition::UrlArtifact {
            source_uid: artifact.original_uid.clone(),
            url: url.clone(),
            url_artifact_name: url_artifact_name.clone(),
        });
    }

    let legacy_value = if legacy.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(legacy.into_iter().collect()))
    };

    let tags = if artifact.tags.is_empty() {
        None
    } else {
        Some(artifact.tags.clone())
    };

    let title = if artifact.title.is_empty() {
        artifact.name.clone()
    } else {
        artifact.title.clone()
    };

    let metadata = Artifact {
        schema_version: 1,
        uuid: artifact.uuid,
        title,
        shape,
        created_at: *import_run_at,
        modified_at: *import_run_at,
        links,
        review_log,
        description: None,
        expects_code_trace: None,
        active: artifact.active,
        derived: artifact.derived,
        tags,
        outline_level: artifact.outline_level.clone(),
        legacy: legacy_value,
        blob_path: None,
        url: if matches!(shape, ArtifactShape::Url) {
            match &artifact.ref_disposition {
                PlanRefDisposition::UrlArtifact { url, .. } => Some(url.clone()),
                _ => url_from_siblings(collection, artifact),
            }
        } else {
            None
        },
        checked_at: None,
        check_status: None,
        overflow: Default::default(),
    };
    let body = if matches!(shape, ArtifactShape::Url) {
        String::new()
    } else {
        ensure_trailing_newline(&artifact.body)
    };
    (metadata, body)
}

fn ensure_trailing_newline(body: &str) -> String {
    if body.is_empty() {
        String::new()
    } else if body.ends_with('\n') {
        body.to_owned()
    } else {
        format!("{body}\n")
    }
}

/// URL companion artifacts don't carry their URL in their own
/// `PlanRefDisposition` (it lives on the source artifact).
/// Walk the pane to recover it when we're rendering the
/// companion.
fn url_from_siblings(collection: &PlanCollection, companion: &PlanArtifact) -> Option<String> {
    for sibling in &collection.artifacts {
        if let PlanRefDisposition::UrlArtifact {
            url,
            url_artifact_uuid,
            ..
        } = &sibling.ref_disposition
            && *url_artifact_uuid == companion.uuid
        {
            return Some(url.clone());
        }
    }
    None
}

fn has_legacy_payload(artifact: &PlanArtifact) -> bool {
    !artifact.legacy_extensions.is_empty()
}

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("refusing import: prefix collisions present: {}", prefixes.join(", "))]
    PrefixCollision { prefixes: Vec<String> },
    #[error("artifacts root missing: {path}")]
    ArtifactsRootMissing { path: PathBuf },
    #[error("creating collection directory {path}: {source}")]
    CollectionDirCreate {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    AtomicWrite(#[from] AtomicWriteError),
    #[error(transparent)]
    Ownership(#[from] OwnershipError),
    #[error(transparent)]
    Artifact(#[from] WriteArtifactError),
    #[error("serialising collection config: {source}")]
    SerializeCollection {
        #[source]
        source: serde_json::Error,
    },
}
