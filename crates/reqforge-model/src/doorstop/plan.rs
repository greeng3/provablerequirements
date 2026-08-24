//! Import-plan construction (Phase 8.1).
//!
//! Given a parsed doorstop tree plus the target project's
//! current Collections, [`build_plan`] produces an
//! [`ImportPlan`] — a data structure describing every write
//! the importer would perform, but performing none of them.
//!
//! The plan carries enough detail for the preview endpoint to
//! render its report (collection counts, link translations,
//! ref dispositions, prefix collisions, unresolved links) and
//! for the 8.2 execute step to write files without going back
//! to the YAML.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::doorstop::ids::reqforge_name_from_uid;
use crate::doorstop::parse::{DoorstopDocument, DoorstopItem, DoorstopSettings, ParseError};
use crate::doorstop::refs::{RefClass, classify_ref};
use crate::load::LoadedProject;

/// Full import plan for one source → project translation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlan {
    /// ISO-8601 timestamp applied uniformly to every imported
    /// artifact's `createdAt` / `modifiedAt` + any synthetic
    /// review entry. Computed once when the plan is built so
    /// the preview and the execute step agree.
    pub import_run_at: DateTime<Utc>,

    pub collections: Vec<PlanCollection>,

    /// Prefix collisions against the target project's existing
    /// Collections. When non-empty the execute step refuses to
    /// run (per INTEROP-doorstopPrefixCollision).
    pub prefix_collisions: Vec<PrefixCollision>,

    /// Every doorstop link that couldn't be resolved to an
    /// imported artifact. Still written on the source side with
    /// its hint populated (per INTEROP-doorstopLinkTranslation)
    /// so `TRACE-unresolvedLinks` handles it post-import.
    pub unresolved_links: Vec<UnresolvedLink>,

    /// Non-fatal warnings the preview + report should surface.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanCollection {
    pub prefix: String,
    /// Default Collection display name — generated from the
    /// prefix per INTEROP-doorstopDocumentMapping. Editable
    /// post-import.
    pub name: String,
    /// Slugified prefix used as the Collection directory name.
    pub directory_name: String,
    pub source_marker_path: PathBuf,
    /// Unmapped doorstop document settings preserved in the
    /// Collection's `importNotes` object.
    pub import_notes: BTreeMap<String, serde_json::Value>,
    pub artifacts: Vec<PlanArtifact>,
    pub empty_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanArtifact {
    /// UUID assigned during plan build; the execute step reuses
    /// it so a preview-then-execute flow stays consistent with
    /// the preview's displayed plan.
    pub uuid: Uuid,
    /// Ultimate ReqForge artifact name (`<prefix>-<nanu>` with
    /// dashes inside the NANU replaced by underscores).
    pub name: String,
    /// Original doorstop UID — also stored in the imported
    /// artifact's `legacy.doorstopUid`.
    pub original_uid: String,
    pub title: String,
    pub body: String,
    pub active: Option<bool>,
    pub derived: Option<bool>,
    pub outline_level: Option<String>,
    /// Translated links as they'll appear on the artifact, in
    /// the order the doorstop source listed them.
    pub links: Vec<PlanArtifactLink>,
    /// Tag set — empty unless the item carried `normative:
    /// false`, in which case `["non-normative"]`.
    pub tags: Vec<String>,
    /// Tracks whether the item's `ref` yielded a URL artifact
    /// or a `legacy.ref` preservation.
    pub ref_disposition: PlanRefDisposition,
    /// Initial review-log entry synthesised from a non-null
    /// `reviewed` hash (per INTEROP-doorstopReviewedHash). `None`
    /// when the item's `reviewed` field was null / absent.
    pub synthetic_review: Option<PlanSyntheticReview>,
    /// Extension fields preserved in the imported artifact's
    /// `legacy` object. Every unrecognised doorstop field lands
    /// here; additional keys (`ref`, `doorstopUid`) are merged
    /// in by the execute step.
    pub legacy_extensions: BTreeMap<String, serde_json::Value>,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanArtifactLink {
    pub target_uuid: Uuid,
    pub link_type: String,
    pub hint: PlanLinkHint,
    /// Marks links whose target couldn't be resolved. The link
    /// is still written with its hint populated so
    /// `TRACE-unresolvedLinks` can surface it in ReqForge
    /// post-import.
    pub unresolved: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanLinkHint {
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum PlanRefDisposition {
    None,
    /// A URL-shaped ref produced a sibling URL artifact +
    /// `cites` link. `url_artifact_uuid` is the UUID of the
    /// generated URL artifact; its metadata is carried in the
    /// same `PlanCollection.artifacts` list for simplicity.
    UrlArtifact {
        url: String,
        url_artifact_uuid: Uuid,
        url_artifact_name: String,
    },
    /// Non-URL value preserved on `legacy.ref`.
    Legacy {
        value: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSyntheticReview {
    pub outcome: String,
    pub reviewer: String,
    pub timestamp: DateTime<Utc>,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefixCollision {
    pub prefix: String,
    pub existing_collection_directory: String,
    pub doorstop_marker_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedLink {
    pub source_uid: String,
    pub target_uid: String,
    pub source_marker_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error(transparent)]
    Parse(#[from] ParseError),
}

/// Build an import plan. `project` is the target ReqForge
/// project (used for prefix-collision detection against its
/// existing Collections). `documents` is the parsed doorstop
/// tree; the caller owns the walk and passes a pre-parsed
/// list so tests can bypass filesystem IO.
pub fn build_plan(
    project: &LoadedProject,
    documents: Vec<DoorstopDocument>,
    now: DateTime<Utc>,
) -> Result<ImportPlan, PlanError> {
    let mut collections: Vec<PlanCollection> = Vec::new();
    let mut prefix_collisions: Vec<PrefixCollision> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // First pass: assign UUIDs to every doorstop item so the
    // second pass can resolve `links` against a complete map.
    // Key is the original doorstop UID; value is the (prefix,
    // normalised-name, UUID) triple.
    let mut index: HashMap<String, ItemKey> = HashMap::new();
    for doc in &documents {
        let sep = doc.settings.sep.as_deref().unwrap_or("");
        for item in &doc.items {
            let Some(name) = reqforge_name_from_uid(&item.uid, &doc.settings.prefix, sep) else {
                warnings.push(format!(
                    "item {} skipped: UID does not start with prefix '{}' + sep '{}'",
                    item.source_path.display(),
                    doc.settings.prefix,
                    sep
                ));
                continue;
            };
            index.insert(
                item.uid.clone(),
                ItemKey {
                    prefix: doc.settings.prefix.clone(),
                    name,
                    uuid: Uuid::now_v7(),
                },
            );
        }
    }

    // Second pass: build PlanCollection / PlanArtifact per
    // document, resolving links against `index`.
    let mut unresolved_links: Vec<UnresolvedLink> = Vec::new();
    for doc in documents {
        // Collision check against the existing project.
        let collision = project
            .collections
            .iter()
            .find(|c| c.config.prefix == doc.settings.prefix);
        if let Some(existing) = collision {
            prefix_collisions.push(PrefixCollision {
                prefix: doc.settings.prefix.clone(),
                existing_collection_directory: existing.dir_name.clone(),
                doorstop_marker_path: doc.marker_path.clone(),
            });
            // Still plan the collection so the preview shows
            // the full impact, but the execute step refuses
            // when the collision list is non-empty.
        }

        let pane_name = generate_collection_name(&doc.settings.prefix);
        let directory_name = slugify_prefix(&doc.settings.prefix);
        let import_notes = build_import_notes(&doc.settings);

        let mut artifacts: Vec<PlanArtifact> = Vec::new();
        let project_slug = project.config.slug.clone();

        let empty_warning = if doc.items.is_empty() {
            Some(format!(
                "doorstop document at {} has no items — producing an empty Collection",
                doc.marker_path.display()
            ))
        } else {
            None
        };
        if let Some(w) = &empty_warning {
            warnings.push(w.clone());
        }

        for item in doc.items {
            let Some(entry) = index.get(&item.uid).cloned() else {
                // Already warned on in the first pass.
                continue;
            };
            let (links, url_companion) = build_artifact_links(
                &item,
                &doc.marker_path,
                &project_slug,
                &entry.prefix,
                &index,
                &mut unresolved_links,
                now,
            );
            let tags = build_tags(&item);
            let ref_disposition = match classify_ref(item.ref_field.as_deref()) {
                RefClass::None => PlanRefDisposition::None,
                RefClass::Url(url) => {
                    let uuid = Uuid::now_v7();
                    let url_name = build_url_artifact_name(&entry.name, &artifacts);
                    PlanRefDisposition::UrlArtifact {
                        url,
                        url_artifact_uuid: uuid,
                        url_artifact_name: url_name,
                    }
                }
                RefClass::NonUrl(value) => PlanRefDisposition::Legacy { value },
            };
            // Legacy extensions — every unrecognised doorstop
            // field ends up here. We don't merge the
            // `legacy.ref` or `legacy.doorstopUid` keys in
            // yet; the execute step adds those when it writes
            // the sidecar since they'd duplicate structured
            // data in the plan output otherwise.
            let legacy_extensions = item
                .extensions
                .iter()
                .filter_map(|(k, v)| serde_json::to_value(v).ok().map(|jv| (k.clone(), jv)))
                .collect();

            let synthetic_review = item.reviewed.as_deref().and_then(|hash| {
                let hash = hash.trim();
                if hash.is_empty() {
                    None
                } else {
                    Some(PlanSyntheticReview {
                        outcome: "approved".into(),
                        reviewer: "imported-from-doorstop".into(),
                        timestamp: now,
                        explanation: format!(
                            "Imported from doorstop; original reviewed hash: {hash}"
                        ),
                    })
                }
            });

            artifacts.push(PlanArtifact {
                uuid: entry.uuid,
                name: entry.name.clone(),
                original_uid: item.uid.clone(),
                title: extract_title(&item),
                body: item.text.clone().unwrap_or_default(),
                active: item.active,
                derived: item.derived,
                outline_level: item.level.clone().filter(|s| !s.is_empty()),
                links,
                tags,
                ref_disposition,
                synthetic_review,
                legacy_extensions,
                source_path: item.source_path.clone(),
            });

            // The URL-companion artifact rides alongside its
            // source artifact in the same collection — appended
            // after the source so the execute step writes them
            // in source-first order.
            if let Some(companion) = url_companion {
                artifacts.push(companion);
            }
        }

        collections.push(PlanCollection {
            prefix: doc.settings.prefix,
            name: pane_name,
            directory_name,
            source_marker_path: doc.marker_path,
            import_notes,
            artifacts,
            empty_warning,
        });
    }

    Ok(ImportPlan {
        import_run_at: now,
        collections,
        prefix_collisions,
        unresolved_links,
        warnings,
    })
}

/// Generate the default Collection display name from a prefix.
/// Callers can rename the Collection after import (the name is
/// stored in `.collection.json` as editable post-import).
pub fn generate_collection_name(prefix: &str) -> String {
    format!("{prefix} (imported from doorstop)")
}

/// Slugify the doorstop prefix into a directory name. Lower-
/// case + replace sequences of non-alphanumeric characters
/// with a single `-`.
pub fn slugify_prefix(prefix: &str) -> String {
    let lowered = prefix.to_ascii_lowercase();
    let mut out = String::with_capacity(lowered.len());
    let mut last_dash = false;
    for c in lowered.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "imported".to_owned()
    } else {
        out
    }
}

fn build_import_notes(settings: &DoorstopSettings) -> BTreeMap<String, serde_json::Value> {
    let mut m: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    if let Some(p) = &settings.parent {
        m.insert(
            "doorstopParent".into(),
            serde_json::Value::String(p.clone()),
        );
    }
    if let Some(s) = &settings.sep {
        m.insert("doorstopSep".into(), serde_json::Value::String(s.clone()));
    }
    if let Some(d) = settings.digits {
        m.insert(
            "doorstopDigits".into(),
            serde_json::Value::Number(serde_json::Number::from(d)),
        );
    }
    if let Some(fmt) = &settings.itemformat {
        m.insert(
            "doorstopItemFormat".into(),
            serde_json::Value::String(fmt.clone()),
        );
    }
    m
}

/// Build the translated links for an item. Every doorstop link
/// becomes a `derives-from` typed link. Unresolved targets are
/// surfaced in the import report but the link is still written
/// with its hint populated.
///
/// Returns the link list plus an optional URL-companion artifact
/// generated from a URL-shaped ref (the URL artifact itself is
/// a sibling in the same collection).
fn build_artifact_links(
    item: &DoorstopItem,
    marker_path: &std::path::Path,
    project_slug: &str,
    _source_prefix: &str,
    index: &HashMap<String, ItemKey>,
    unresolved_out: &mut Vec<UnresolvedLink>,
    now: DateTime<Utc>,
) -> (Vec<PlanArtifactLink>, Option<PlanArtifact>) {
    let mut links: Vec<PlanArtifactLink> = Vec::new();
    for target_uid in &item.links {
        if let Some(target) = index.get(target_uid) {
            links.push(PlanArtifactLink {
                target_uuid: target.uuid,
                link_type: "derives-from".into(),
                hint: PlanLinkHint {
                    project_slug: project_slug.to_owned(),
                    collection_prefix: target.prefix.clone(),
                    artifact_name: target.name.clone(),
                },
                unresolved: false,
            });
        } else {
            // Unresolved — still write the hint so the
            // user can see where the doorstop source
            // pointed.
            unresolved_out.push(UnresolvedLink {
                source_uid: item.uid.clone(),
                target_uid: target_uid.clone(),
                source_marker_path: marker_path.to_path_buf(),
            });
            links.push(PlanArtifactLink {
                // A fresh UUID so the write-side validator
                // doesn't trip on `nil`; the hint carries the
                // doorstop UID so operators can track the
                // intent.
                target_uuid: Uuid::now_v7(),
                link_type: "derives-from".into(),
                hint: PlanLinkHint {
                    project_slug: project_slug.to_owned(),
                    collection_prefix: "".into(),
                    artifact_name: target_uid.clone(),
                },
                unresolved: true,
            });
        }
    }

    // If the item's ref is URL-shaped, generate the companion
    // URL artifact here so the plan carries both the source
    // artifact's updated link list and the sibling URL artifact
    // ready for writing.
    let companion = match classify_ref(item.ref_field.as_deref()) {
        RefClass::Url(url) => {
            let uuid = Uuid::now_v7();
            let name = url_companion_name_for(&item.uid);
            links.push(PlanArtifactLink {
                target_uuid: uuid,
                link_type: "cites".into(),
                hint: PlanLinkHint {
                    project_slug: project_slug.to_owned(),
                    collection_prefix: String::new(),
                    artifact_name: name.clone(),
                },
                unresolved: false,
            });
            Some(PlanArtifact {
                uuid,
                name,
                original_uid: String::new(),
                title: url.clone(),
                body: String::new(),
                active: Some(true),
                derived: Some(false),
                outline_level: None,
                links: Vec::new(),
                tags: Vec::new(),
                ref_disposition: PlanRefDisposition::None,
                synthetic_review: None,
                legacy_extensions: BTreeMap::new(),
                source_path: item.source_path.clone(),
            })
        }
        _ => None,
    };
    let (final_links, companion) = match companion {
        Some(mut c) => {
            // Re-stamp the companion with the fixed timestamp
            // (the plan's import-run instant) — but PlanArtifact
            // carries no timestamp field, so nothing to do here
            // beyond keeping the companion as-is.
            let _ = now;
            c.source_path = item.source_path.clone();
            (links, Some(c))
        }
        None => (links, None),
    };
    (final_links, companion)
}

fn url_companion_name_for(source_uid: &str) -> String {
    format!("{source_uid}_ref")
}

/// Figure out a URL-companion artifact name that doesn't
/// collide with siblings already planned in the same
/// collection. Kept at the module root for callers that need
/// it beyond [`build_artifact_links`].
fn build_url_artifact_name(source_name: &str, already_planned: &[PlanArtifact]) -> String {
    let base = format!("{source_name}_ref");
    if !already_planned.iter().any(|a| a.name == base) {
        return base;
    }
    let mut idx = 2u32;
    loop {
        let candidate = format!("{base}_{idx}");
        if !already_planned.iter().any(|a| a.name == candidate) {
            return candidate;
        }
        idx += 1;
    }
}

fn build_tags(item: &DoorstopItem) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    if item.normative == Some(false) {
        tags.push("non-normative".into());
    }
    tags
}

fn extract_title(item: &DoorstopItem) -> String {
    item.header
        .as_deref()
        .map(|h| h.trim().to_owned())
        .unwrap_or_else(|| item.uid.clone())
}

/// Local alias for the first-pass UUID-assignment map; mirrors
/// the per-item key written in `build_plan`.
#[derive(Clone)]
struct ItemKey {
    prefix: String,
    name: String,
    uuid: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doorstop::parse::{DoorstopItem, DoorstopSettings};
    use std::collections::BTreeMap;

    fn sample_project(slug: &str) -> LoadedProject {
        LoadedProject {
            root: PathBuf::from(format!("/tmp/{slug}")),
            config: crate::schema::ProjectConfig {
                schema_version: 1,
                slug: slug.to_owned(),
                name: slug.to_owned(),
                description: None,
                artifacts_path: None,
                scan_paths: None,
                overflow: BTreeMap::new(),
            },
            collections: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn item(
        source: &str,
        uid: &str,
        header: &str,
        text: &str,
        links: Vec<&str>,
        ref_val: Option<&str>,
        reviewed: Option<&str>,
    ) -> DoorstopItem {
        DoorstopItem {
            source_path: PathBuf::from(source),
            uid: uid.to_owned(),
            header: Some(header.to_owned()),
            text: Some(text.to_owned()),
            active: Some(true),
            derived: Some(false),
            level: Some("1.0".into()),
            normative: Some(true),
            links: links.into_iter().map(str::to_owned).collect(),
            ref_field: ref_val.map(str::to_owned),
            reviewed: reviewed.map(str::to_owned),
            extensions: BTreeMap::new(),
        }
    }

    fn document(
        marker: &str,
        prefix: &str,
        sep: Option<&str>,
        items: Vec<DoorstopItem>,
    ) -> DoorstopDocument {
        DoorstopDocument {
            marker_path: PathBuf::from(marker),
            directory: PathBuf::from(marker).parent().unwrap().to_path_buf(),
            settings: DoorstopSettings {
                prefix: prefix.to_owned(),
                sep: sep.map(str::to_owned),
                digits: Some(3),
                parent: None,
                itemformat: Some("yaml".into()),
            },
            items,
        }
    }

    #[test]
    fn empty_tree_plans_nothing() {
        let project = sample_project("sample");
        let plan = build_plan(&project, Vec::new(), chrono::Utc::now()).unwrap();
        assert!(plan.collections.is_empty());
        assert!(plan.prefix_collisions.is_empty());
        assert!(plan.unresolved_links.is_empty());
    }

    #[test]
    fn single_doc_single_item_full_translation() {
        let project = sample_project("sample");
        let docs = vec![document(
            "/src/req/.doorstop.yml",
            "REQ",
            Some("-"),
            vec![item(
                "/src/req/REQ-001.yml",
                "REQ-001",
                "Pressure envelope",
                "body prose",
                vec![],
                Some("https://example.com/spec"),
                Some("abcd"),
            )],
        )];
        let now = chrono::Utc::now();
        let plan = build_plan(&project, docs, now).unwrap();
        assert_eq!(plan.collections.len(), 1);
        let col = &plan.collections[0];
        assert_eq!(col.prefix, "REQ");
        assert_eq!(col.directory_name, "req");
        assert_eq!(col.import_notes.get("doorstopSep").unwrap(), "-");
        assert_eq!(col.import_notes.get("doorstopItemFormat").unwrap(), "yaml");

        // Source artifact + URL companion.
        assert_eq!(col.artifacts.len(), 2);
        let source = &col.artifacts[0];
        assert_eq!(source.name, "REQ-001");
        assert_eq!(source.original_uid, "REQ-001");
        assert_eq!(source.title, "Pressure envelope");
        assert_eq!(source.body, "body prose");
        // URL companion appended.
        assert!(col.artifacts[1].name.ends_with("_ref"));
        // The cites link points at the companion.
        assert_eq!(source.links.len(), 1);
        assert_eq!(source.links[0].link_type, "cites");
        assert_eq!(source.links[0].target_uuid, col.artifacts[1].uuid);

        // Synthetic review entry from reviewed hash.
        let review = source.synthetic_review.as_ref().unwrap();
        assert_eq!(review.outcome, "approved");
        assert_eq!(review.reviewer, "imported-from-doorstop");
        assert!(review.explanation.contains("abcd"));
    }

    #[test]
    fn prefix_collision_is_reported_not_elided() {
        use crate::load::LoadedCollection;
        let mut project = sample_project("sample");
        project.collections.push(LoadedCollection {
            dir_name: "requirements".into(),
            dir_path: project.root.join("requirements"),
            config: crate::schema::CollectionConfig {
                schema_version: 1,
                prefix: "REQ".into(),
                name: "Existing".into(),
                description: None,
                expects_code_trace: None,
                import_notes: None,
                overflow: BTreeMap::new(),
            },
            artifacts: Vec::new(),
        });
        let docs = vec![document("/src/req/.doorstop.yml", "REQ", Some("-"), vec![])];
        let plan = build_plan(&project, docs, chrono::Utc::now()).unwrap();
        assert_eq!(plan.prefix_collisions.len(), 1);
        assert_eq!(plan.prefix_collisions[0].prefix, "REQ");
        assert_eq!(
            plan.prefix_collisions[0].existing_collection_directory,
            "requirements"
        );
    }

    #[test]
    fn link_resolution_succeeds_across_collections_and_flags_misses() {
        let project = sample_project("sample");
        let docs = vec![
            document(
                "/src/req/.doorstop.yml",
                "REQ",
                Some("-"),
                vec![item(
                    "/src/req/REQ-001.yml",
                    "REQ-001",
                    "A",
                    "a",
                    vec![],
                    None,
                    None,
                )],
            ),
            document(
                "/src/des/.doorstop.yml",
                "DES",
                Some("-"),
                vec![item(
                    "/src/des/DES-001.yml",
                    "DES-001",
                    "B",
                    "b",
                    // One resolvable target + one that points
                    // at a UID that doesn't exist.
                    vec!["REQ-001", "REQ-nope"],
                    None,
                    None,
                )],
            ),
        ];
        let plan = build_plan(&project, docs, chrono::Utc::now()).unwrap();
        let des_art = plan
            .collections
            .iter()
            .find(|c| c.prefix == "DES")
            .unwrap()
            .artifacts
            .first()
            .unwrap();
        assert_eq!(des_art.links.len(), 2);
        assert!(!des_art.links[0].unresolved);
        assert!(des_art.links[1].unresolved);
        assert_eq!(plan.unresolved_links.len(), 1);
        assert_eq!(plan.unresolved_links[0].target_uid, "REQ-nope");
    }

    #[test]
    fn non_url_ref_goes_into_legacy_ref_disposition() {
        let project = sample_project("sample");
        let docs = vec![document(
            "/src/req/.doorstop.yml",
            "REQ",
            Some("-"),
            vec![item(
                "/src/req/REQ-001.yml",
                "REQ-001",
                "A",
                "a",
                vec![],
                Some("Smith 1994"),
                None,
            )],
        )];
        let plan = build_plan(&project, docs, chrono::Utc::now()).unwrap();
        let art = &plan.collections[0].artifacts[0];
        match &art.ref_disposition {
            PlanRefDisposition::Legacy { value } => {
                assert_eq!(value, "Smith 1994");
            }
            other => panic!("expected Legacy, got {other:?}"),
        }
        // No URL companion appended.
        assert_eq!(plan.collections[0].artifacts.len(), 1);
    }

    #[test]
    fn non_normative_item_gets_the_tag() {
        let project = sample_project("sample");
        let mut i = item(
            "/src/req/REQ-001.yml",
            "REQ-001",
            "A",
            "a",
            vec![],
            None,
            None,
        );
        i.normative = Some(false);
        let plan = build_plan(
            &project,
            vec![document(
                "/src/req/.doorstop.yml",
                "REQ",
                Some("-"),
                vec![i],
            )],
            chrono::Utc::now(),
        )
        .unwrap();
        assert_eq!(
            plan.collections[0].artifacts[0].tags,
            vec!["non-normative".to_owned()]
        );
    }

    #[test]
    fn empty_document_warns_and_produces_empty_collection() {
        let project = sample_project("sample");
        let plan = build_plan(
            &project,
            vec![document(
                "/src/empty/.doorstop.yml",
                "EMP",
                Some("-"),
                vec![],
            )],
            chrono::Utc::now(),
        )
        .unwrap();
        assert_eq!(plan.collections.len(), 1);
        assert!(plan.collections[0].artifacts.is_empty());
        assert!(plan.collections[0].empty_warning.is_some());
        assert!(!plan.warnings.is_empty());
    }

    #[test]
    fn item_with_mismatched_prefix_is_skipped_with_a_warning() {
        let project = sample_project("sample");
        let plan = build_plan(
            &project,
            vec![document(
                "/src/req/.doorstop.yml",
                "REQ",
                Some("-"),
                vec![item(
                    "/src/req/X-001.yml",
                    "X-001",
                    "Bad",
                    "b",
                    vec![],
                    None,
                    None,
                )],
            )],
            chrono::Utc::now(),
        )
        .unwrap();
        assert!(plan.collections[0].artifacts.is_empty());
        assert!(plan.warnings.iter().any(|w| w.contains("X-001")));
    }

    #[test]
    fn slugify_prefix_lowercases_and_dashes_separators() {
        assert_eq!(slugify_prefix("REQ"), "req");
        assert_eq!(slugify_prefix("Code Style"), "code-style");
        assert_eq!(slugify_prefix(""), "imported");
    }
}
