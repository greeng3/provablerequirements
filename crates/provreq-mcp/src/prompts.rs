//! Canned MCP prompts per `LLM-mcpPrompts`.
//!
//! Six workflow templates from INTENTIONS.md. Each prompt's
//! `messages` array is a starting point — the agent fills in
//! the gaps by calling tools (provreq_run_report,
//! provreq_get_artifact, etc.) and weaving the results into
//! its own response.
//!
//! Pure compute: no HTTP calls, no state. Hence no wiremock
//! tests — the unit tests cover the static registry + the
//! argument-injection path.

use serde_json::Value;

use crate::error::HandlerError;
use crate::protocol::{
    ContentBlock, GetPromptParams, GetPromptResult, ListPromptsResult, PromptArgument,
    PromptDefinition, PromptMessage, PromptRole,
};

pub fn prompt_definitions() -> Vec<PromptDefinition> {
    vec![
        PromptDefinition {
            name: "gap_analysis".into(),
            description: "Identify coverage and traceability gaps. Walk the coverage-matrix \
                 and unresolved-links reports, group the findings, and recommend \
                 next steps."
                .into(),
            arguments: vec![scope_argument()],
        },
        PromptDefinition {
            name: "coverage_summary".into(),
            description: "Write a short human-readable summary of traceability coverage \
                 across the selected scope. Quote parent/child counts, gap rates, \
                 and which collections contribute most of the gap surface."
                .into(),
            arguments: vec![scope_argument()],
        },
        PromptDefinition {
            name: "review_assist".into(),
            description: "Help a reviewer work through the queue. Summarise what's pending \
                 review, call out blocking TODOs, and suggest an order based on \
                 impact and recency."
                .into(),
            arguments: Vec::new(),
        },
        PromptDefinition {
            name: "implementation_planning".into(),
            description: "Given a seed artifact UUID, propose an implementation approach: \
                 which code layers or tests to touch, which downstream artifacts \
                 to update, and risks to flag."
                .into(),
            arguments: vec![uuid_argument(true)],
        },
        PromptDefinition {
            name: "test_gap_planning".into(),
            description: "Identify requirements lacking verification links. For each gap, \
                 propose a test-case skeleton (unit / integration / E2E as \
                 appropriate) and a target location for the test."
                .into(),
            arguments: vec![scope_argument()],
        },
        PromptDefinition {
            name: "impact_analysis_narrative".into(),
            description: "Narrate the impact of changing a seed artifact. Walk the impact- \
                 analysis report, group affected artifacts by type, and describe \
                 what each group means in plain language."
                .into(),
            arguments: vec![uuid_argument(true)],
        },
    ]
}

fn scope_argument() -> PromptArgument {
    PromptArgument {
        name: "scope".into(),
        description:
            "Optional scope selector (system, project:<slug>, collection:<slug>/<prefix>). Defaults to system."
                .into(),
        required: false,
    }
}

fn uuid_argument(required: bool) -> PromptArgument {
    PromptArgument {
        name: "uuid".into(),
        description: "Seed artifact UUID.".into(),
        required,
    }
}

pub fn list_prompts() -> ListPromptsResult {
    ListPromptsResult {
        prompts: prompt_definitions(),
    }
}

pub fn get_prompt(params: GetPromptParams) -> Result<GetPromptResult, HandlerError> {
    let scope = params
        .arguments
        .as_ref()
        .and_then(|v| v.get("scope"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "system".to_owned());
    let uuid = params
        .arguments
        .as_ref()
        .and_then(|v| v.get("uuid"))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let definition = prompt_definitions()
        .into_iter()
        .find(|p| p.name == params.name)
        .ok_or_else(|| HandlerError::InvalidParams(format!("unknown prompt '{}'", params.name)))?;
    let uuid_required = definition
        .arguments
        .iter()
        .any(|a| a.name == "uuid" && a.required);
    if uuid_required && uuid.is_none() {
        return Err(HandlerError::InvalidParams(format!(
            "prompt '{}' requires a 'uuid' argument",
            params.name
        )));
    }
    let body = match params.name.as_str() {
        "gap_analysis" => gap_analysis(&scope),
        "coverage_summary" => coverage_summary(&scope),
        "review_assist" => review_assist(),
        "implementation_planning" => implementation_planning(uuid.as_deref().unwrap()),
        "test_gap_planning" => test_gap_planning(&scope),
        "impact_analysis_narrative" => impact_analysis_narrative(uuid.as_deref().unwrap()),
        _ => unreachable!("definition lookup above caught unknown names"),
    };
    Ok(GetPromptResult {
        description: Some(definition.description),
        messages: vec![PromptMessage {
            role: PromptRole::User,
            content: ContentBlock::text(body),
        }],
    })
}

// --- Prompt bodies --------------------------------------------------------

fn gap_analysis(scope: &str) -> String {
    format!(
        "Analyse traceability gaps in the provreq System, scoped to `{scope}`.\n\n\
         Workflow:\n\
         1. Call `provreq_run_report` with `kind: \"coverage-matrix\"` and scope={scope}. \
         Note every parent artifact flagged `hasGap: true`.\n\
         2. Call `provreq_run_report` with `kind: \"unresolved-links\"` and the same scope. \
         Distinguish true orphans (missing UUID) from name-drift cases.\n\
         3. Group the findings by Collection so the report is scannable per team / component.\n\
         4. For each gap, suggest a concrete next step: add a covering child artifact, \
         fix a dangling link target, or close the parent if obsolete.\n\n\
         Structure your response with a short executive summary first, then the per-Collection \
         breakdown. Cite artifact paths as `slug/PREFIX/name` and UUIDs in parentheses."
    )
}

fn coverage_summary(scope: &str) -> String {
    format!(
        "Write a concise coverage summary for provreq scope `{scope}`.\n\n\
         Workflow:\n\
         1. Call `provreq_run_report` with `kind: \"coverage-matrix\"` and scope={scope}.\n\
         2. Compute: total parent artifacts, gap count, gap rate, and which Collections \
         contribute the most to the gap surface.\n\
         3. Compare against any covering-code-evidence data if present (Phase 9b).\n\n\
         Keep the summary under 200 words. Highlight the single biggest gap driver so the \
         reader knows where to focus."
    )
}

fn review_assist() -> String {
    "Help the user work through the review queue.\n\n\
     Workflow:\n\
     1. Call `provreq_run_report` with `kind: \"review-status\"` (system scope).\n\
     2. Summarise what's pending: counts by state, any artifacts with blocking TODOs, \
     anything re-requested after rejection.\n\
     3. Propose a suggested review order — start with re-requested items (finite work), \
     then blocking-TODO items (unblocks downstream), then newly-submitted items.\n\
     4. For each artifact you call out, quote its path (`slug/PREFIX/name`) and UUID.\n\n\
     Keep the summary scannable — a reviewer uses this to decide what to tackle next, \
     not as a replacement for reading the artifacts themselves."
        .to_owned()
}

fn implementation_planning(uuid: &str) -> String {
    format!(
        "Propose an implementation plan for provreq artifact `{uuid}`.\n\n\
         Workflow:\n\
         1. Call `provreq_get_artifact` with `uuid: \"{uuid}\"` to fetch the content and links.\n\
         2. Call `provreq_get_incoming_links` with the same uuid to see what depends on it.\n\
         3. Call `provreq_run_report` with `kind: \"code-traceability\"` if you need \
         existing-code context.\n\
         4. Break the work into phases. For each phase, list files / modules to touch, \
         tests to add, and which downstream artifacts need updating.\n\
         5. Flag risks: API/schema changes, migrations, performance regressions, \
         cross-cutting concerns.\n\n\
         End with a short \"checklist before shipping\" the user can run through."
    )
}

fn test_gap_planning(scope: &str) -> String {
    format!(
        "Identify requirements lacking verification coverage in scope `{scope}`.\n\n\
         Workflow:\n\
         1. Call `provreq_run_report` with `kind: \"coverage-matrix\"` and scope={scope} \
         using default covering types (satisfies, verifies).\n\
         2. Separately call it with `coveringLinkTypes: \"verifies\"` alone to isolate \
         pure verification gaps.\n\
         3. For each verification gap, propose a concrete test: unit / integration / E2E, \
         a target file path, and a one-line test description.\n\
         4. Suggest which gaps to tackle first based on risk (criticality tags, prior defect \
         density in the area if known).\n\n\
         Cite artifacts as `slug/PREFIX/name (uuid)`. Keep per-gap entries to three lines."
    )
}

fn impact_analysis_narrative(uuid: &str) -> String {
    format!(
        "Describe the impact of changing provreq artifact `{uuid}` in plain language.\n\n\
         Workflow:\n\
         1. Call `provreq_run_report` with `kind: \"impact-analysis\"` and the seed uuid \
         in the report arguments.\n\
         2. Group affected artifacts by shape (content / blob / url) and by link direction \
         (depends-on vs depended-on-by).\n\
         3. For each group, write one paragraph: what the group represents, rough count, \
         and what changes would ripple through.\n\
         4. End with a call-out of the highest-impact items — things whose change would cascade \
         widest.\n\n\
         Assume a technical reader. Quote artifact paths as `slug/PREFIX/name`."
    )
}

#[allow(dead_code)]
fn stringify_args(args: &Option<Value>) -> String {
    args.as_ref()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "{}".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn list_prompts_returns_exactly_six() {
        let out = list_prompts();
        assert_eq!(out.prompts.len(), 6);
        let names: Vec<&str> = out.prompts.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"gap_analysis"));
        assert!(names.contains(&"coverage_summary"));
        assert!(names.contains(&"review_assist"));
        assert!(names.contains(&"implementation_planning"));
        assert!(names.contains(&"test_gap_planning"));
        assert!(names.contains(&"impact_analysis_narrative"));
    }

    #[test]
    fn implementation_planning_requires_uuid() {
        let err = get_prompt(GetPromptParams {
            name: "implementation_planning".into(),
            arguments: None,
        })
        .unwrap_err();
        match err {
            HandlerError::InvalidParams(m) => assert!(m.contains("uuid")),
            other => panic!("expected InvalidParams, got {other:?}"),
        }
    }

    #[test]
    fn get_prompt_injects_scope_into_template() {
        let out = get_prompt(GetPromptParams {
            name: "gap_analysis".into(),
            arguments: Some(json!({ "scope": "project:sample" })),
        })
        .unwrap();
        assert_eq!(out.messages.len(), 1);
        let ContentBlock::Text { text } = &out.messages[0].content;
        assert!(text.contains("project:sample"));
    }

    #[test]
    fn get_prompt_defaults_scope_to_system() {
        let out = get_prompt(GetPromptParams {
            name: "coverage_summary".into(),
            arguments: None,
        })
        .unwrap();
        let ContentBlock::Text { text } = &out.messages[0].content;
        assert!(text.contains("`system`"));
    }

    #[test]
    fn get_prompt_injects_uuid_into_implementation_planning() {
        let uuid = "11111111-1111-1111-1111-111111111111";
        let out = get_prompt(GetPromptParams {
            name: "implementation_planning".into(),
            arguments: Some(json!({ "uuid": uuid })),
        })
        .unwrap();
        let ContentBlock::Text { text } = &out.messages[0].content;
        assert!(text.contains(uuid));
    }

    #[test]
    fn get_prompt_unknown_name_errors_out() {
        let err = get_prompt(GetPromptParams {
            name: "not_a_real_prompt".into(),
            arguments: None,
        })
        .unwrap_err();
        match err {
            HandlerError::InvalidParams(m) => assert!(m.contains("not_a_real_prompt")),
            other => panic!("expected InvalidParams, got {other:?}"),
        }
    }

    #[test]
    fn review_assist_has_no_required_arguments() {
        let out = get_prompt(GetPromptParams {
            name: "review_assist".into(),
            arguments: None,
        })
        .unwrap();
        assert_eq!(out.messages.len(), 1);
    }
}
