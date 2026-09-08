//! Sample-content generator for the `UX-initSampleContent`
//! onboarding choice (Phase 11b).
//!
//! A pure-compute function that returns a fixed set of
//! [`CollectionDraft`] values describing the "Task Tracker" demo
//! scenario — three collections (REQ, DES, UC), seven artifacts,
//! interlinked with `satisfies`, `derives-from`, and `verifies`
//! so the graph, matrix, and coverage-matrix reports all have
//! something interesting to render on a fresh project.
//!
//! The HTTP handler in `http::handlers` composes these drafts
//! into sequential calls to the existing atomic-write + refresh
//! stack so the sample content goes through the same paths as
//! hand-created artifacts. That keeps 11b's blast radius narrow:
//! this module is otherwise inert.

use uuid::Uuid;

use crate::schema::{Link, LinkHint};

/// One collection worth of sample drafts. The `directory_name` /
/// `prefix` / `name` / `description` map one-to-one to
/// `CollectionConfig`; artifacts walk through
/// `create_artifact`-equivalent write paths.
#[derive(Debug, Clone)]
pub struct CollectionDraft {
    pub directory_name: String,
    pub prefix: String,
    pub name: String,
    pub description: Option<String>,
    pub artifacts: Vec<ArtifactDraft>,
}

/// One artifact draft. UUID is pre-allocated so links between
/// drafts can reference each other before anything hits disk —
/// the existing link-hint resolver then fills in the hint
/// fields on load. Every draft in 11b is content-hosted; blob /
/// URL samples need binary content and are deferred.
#[derive(Debug, Clone)]
pub struct ArtifactDraft {
    pub uuid: Uuid,
    pub name: String,
    pub title: String,
    pub description: Option<String>,
    pub body: String,
    pub tags: Vec<String>,
    pub links: Vec<Link>,
}

/// Generate the full Task Tracker sample set. Deterministic
/// except for the generated UUIDs — unit tests assert structure,
/// not literal identifiers.
pub fn generate(project_slug: &str) -> Vec<CollectionDraft> {
    // Pre-allocate every UUID so the link graph can reference
    // artifacts by UUID before any file hits disk. UUID v7 is
    // time-ordered, so the generated set sorts naturally.
    let req_task_creation = Uuid::now_v7();
    let req_task_status = Uuid::now_v7();
    let req_notifications = Uuid::now_v7();
    let des_data_model = Uuid::now_v7();
    let des_notification_service = Uuid::now_v7();
    let uc_create_task = Uuid::now_v7();
    let uc_receive_notification = Uuid::now_v7();

    let hint = |prefix: &str, name: &str| -> LinkHint {
        LinkHint {
            project_slug: project_slug.to_owned(),
            collection_prefix: prefix.to_owned(),
            artifact_name: name.to_owned(),
            overflow: Default::default(),
        }
    };
    let link = |target: Uuid, type_name: &str, prefix: &str, name: &str| -> Link {
        Link {
            target_uuid: target,
            type_name: type_name.to_owned(),
            hint: hint(prefix, name),
            overflow: Default::default(),
        }
    };

    let req = CollectionDraft {
        directory_name: "requirements".to_owned(),
        prefix: "REQ".to_owned(),
        name: "Requirements".to_owned(),
        description: Some("Functional requirements for the Task Tracker.".to_owned()),
        artifacts: vec![
            ArtifactDraft {
                uuid: req_task_creation,
                name: "REQ-task-creation".to_owned(),
                title: "Users can create tasks".to_owned(),
                description: Some("Core creation requirement.".to_owned()),
                body: "# Users can create tasks\n\n\
                       Users shall be able to create tasks with a title, an \
                       optional description, and a due date. Titles are \
                       required and must be non-empty.\n"
                    .to_owned(),
                tags: vec!["sample".into(), "core".into()],
                links: Vec::new(),
            },
            ArtifactDraft {
                uuid: req_task_status,
                name: "REQ-task-status".to_owned(),
                title: "Tasks carry a lifecycle status".to_owned(),
                description: Some("Status-machine requirement.".to_owned()),
                body: "# Tasks carry a lifecycle status\n\n\
                       Every task shall carry a status of `todo`, \
                       `in-progress`, or `done`. New tasks default to `todo`.\n"
                    .to_owned(),
                tags: vec!["sample".into(), "core".into()],
                links: Vec::new(),
            },
            ArtifactDraft {
                uuid: req_notifications,
                name: "REQ-due-date-notifications".to_owned(),
                title: "Users receive due-date notifications".to_owned(),
                description: Some("Timely-reminder requirement.".to_owned()),
                body: "# Users receive due-date notifications\n\n\
                       When a task's due date is within 24 hours, the system \
                       shall notify the owning user. Users shall be able to \
                       disable notifications per-task.\n"
                    .to_owned(),
                tags: vec!["sample".into()],
                links: vec![link(
                    req_task_creation,
                    "derives-from",
                    "REQ",
                    "REQ-task-creation",
                )],
            },
        ],
    };

    let des = CollectionDraft {
        directory_name: "design".to_owned(),
        prefix: "DES".to_owned(),
        name: "Design".to_owned(),
        description: Some("Design documents for the Task Tracker.".to_owned()),
        artifacts: vec![
            ArtifactDraft {
                uuid: des_data_model,
                name: "DES-data-model".to_owned(),
                title: "Task data model".to_owned(),
                description: Some("Relational model backing tasks + users.".to_owned()),
                body: "# Task data model\n\n\
                       A task is stored as `(id, owner_id, title, description, \
                       status, due_at, created_at, updated_at)`. Users are \
                       `(id, email, display_name, timezone)`. Status is a \
                       text column constrained to the three lifecycle values.\n"
                    .to_owned(),
                tags: vec!["sample".into()],
                links: vec![
                    link(req_task_creation, "satisfies", "REQ", "REQ-task-creation"),
                    link(req_task_status, "satisfies", "REQ", "REQ-task-status"),
                ],
            },
            ArtifactDraft {
                uuid: des_notification_service,
                name: "DES-notification-service".to_owned(),
                title: "Notification service".to_owned(),
                description: Some("Async delivery via a message queue.".to_owned()),
                body: "# Notification service\n\n\
                       A scheduled worker polls the tasks table for due dates \
                       inside the next 24 hours and enqueues a notification \
                       onto the outbound message queue. The delivery side \
                       fans out to email / push / in-app channels.\n"
                    .to_owned(),
                tags: vec!["sample".into()],
                links: vec![link(
                    req_notifications,
                    "satisfies",
                    "REQ",
                    "REQ-due-date-notifications",
                )],
            },
        ],
    };

    let uc = CollectionDraft {
        directory_name: "use-cases".to_owned(),
        prefix: "UC".to_owned(),
        name: "Use Cases".to_owned(),
        description: Some("Operator-facing scenarios.".to_owned()),
        artifacts: vec![
            ArtifactDraft {
                uuid: uc_create_task,
                name: "UC-create-task".to_owned(),
                title: "Operator creates a task".to_owned(),
                description: Some("Happy-path task-creation flow.".to_owned()),
                body: "# Operator creates a task\n\n\
                       1. Operator opens the task tracker.\n\
                       2. Operator clicks \"New task\".\n\
                       3. Operator fills in title, optional description, and \
                       due date.\n\
                       4. System stores the task with status `todo` and \
                       returns the task detail view.\n"
                    .to_owned(),
                tags: vec!["sample".into()],
                links: vec![
                    link(req_task_creation, "verifies", "REQ", "REQ-task-creation"),
                    link(des_data_model, "satisfies", "DES", "DES-data-model"),
                ],
            },
            ArtifactDraft {
                uuid: uc_receive_notification,
                name: "UC-receive-notification".to_owned(),
                title: "Operator receives a due-date notification".to_owned(),
                description: Some("End-to-end notification delivery.".to_owned()),
                body: "# Operator receives a due-date notification\n\n\
                       1. A task's due date enters the 24-hour window.\n\
                       2. System enqueues a notification.\n\
                       3. Notification service delivers it on the operator's \
                       preferred channel.\n\
                       4. Operator sees the notification and clicks through \
                       to the task detail view.\n"
                    .to_owned(),
                tags: vec!["sample".into()],
                links: vec![
                    link(
                        req_notifications,
                        "verifies",
                        "REQ",
                        "REQ-due-date-notifications",
                    ),
                    link(
                        des_notification_service,
                        "satisfies",
                        "DES",
                        "DES-notification-service",
                    ),
                ],
            },
        ],
    };

    vec![req, des, uc]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generates_three_collections_with_distinct_prefixes() {
        let collections = generate("demo");
        assert_eq!(collections.len(), 3);
        let prefixes: HashSet<&str> = collections.iter().map(|c| c.prefix.as_str()).collect();
        assert!(prefixes.contains("REQ"));
        assert!(prefixes.contains("DES"));
        assert!(prefixes.contains("UC"));
    }

    #[test]
    fn every_collection_has_artifacts() {
        for c in generate("demo") {
            assert!(
                !c.artifacts.is_empty(),
                "collection {} must carry artifacts",
                c.prefix
            );
        }
    }

    #[test]
    fn total_artifact_count_is_seven_for_the_task_tracker_scenario() {
        let total: usize = generate("demo").iter().map(|c| c.artifacts.len()).sum();
        assert_eq!(total, 7);
    }

    #[test]
    fn every_artifact_uuid_is_unique() {
        let collections = generate("demo");
        let mut seen = HashSet::new();
        for c in &collections {
            for a in &c.artifacts {
                assert!(
                    seen.insert(a.uuid),
                    "duplicate UUID {} in draft set",
                    a.uuid
                );
            }
        }
    }

    #[test]
    fn every_link_target_resolves_to_an_artifact_in_the_draft_set() {
        let collections = generate("demo");
        let uuids: HashSet<Uuid> = collections
            .iter()
            .flat_map(|c| c.artifacts.iter().map(|a| a.uuid))
            .collect();
        for c in &collections {
            for a in &c.artifacts {
                for l in &a.links {
                    assert!(
                        uuids.contains(&l.target_uuid),
                        "link target {} from {} does not exist in the draft set",
                        l.target_uuid,
                        a.name
                    );
                }
            }
        }
    }

    #[test]
    fn link_hints_carry_the_caller_supplied_project_slug() {
        let collections = generate("onboarding-sample");
        for c in &collections {
            for a in &c.artifacts {
                for l in &a.links {
                    assert_eq!(l.hint.project_slug, "onboarding-sample");
                }
            }
        }
    }

    #[test]
    fn sample_exercises_all_three_link_types() {
        let collections = generate("demo");
        let types: HashSet<&str> = collections
            .iter()
            .flat_map(|c| c.artifacts.iter())
            .flat_map(|a| a.links.iter().map(|l| l.type_name.as_str()))
            .collect();
        assert!(types.contains("satisfies"));
        assert!(types.contains("derives-from"));
        assert!(types.contains("verifies"));
    }

    #[test]
    fn every_artifact_participates_in_the_link_graph() {
        // Either outgoing link(s) or cited as a target of one.
        let collections = generate("demo");
        let mut outgoing: HashSet<Uuid> = HashSet::new();
        let mut incoming: HashSet<Uuid> = HashSet::new();
        for c in &collections {
            for a in &c.artifacts {
                if !a.links.is_empty() {
                    outgoing.insert(a.uuid);
                }
                for l in &a.links {
                    incoming.insert(l.target_uuid);
                }
            }
        }
        for c in &collections {
            for a in &c.artifacts {
                assert!(
                    outgoing.contains(&a.uuid) || incoming.contains(&a.uuid),
                    "{} is an orphan in the sample graph",
                    a.name
                );
            }
        }
    }

    #[test]
    fn artifact_names_are_all_unique_and_safe_filename_stems() {
        let collections = generate("demo");
        let mut names = HashSet::new();
        for c in &collections {
            for a in &c.artifacts {
                assert!(names.insert(a.name.clone()), "duplicate name {}", a.name);
                // Alphanumeric + dot/underscore/hyphen only.
                let ok = a
                    .name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
                assert!(ok, "name {} is not a safe filename stem", a.name);
                assert!(!a.name.is_empty());
            }
        }
    }
}
