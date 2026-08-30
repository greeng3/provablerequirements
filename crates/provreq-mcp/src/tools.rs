//! Read-only MCP tools per `LLM-mcpTools`.
//!
//! Eleven handlers mapping one-to-one to existing
//! `provreq server` REST endpoints. Each converts its typed
//! arguments into a URL + query string, calls the server, and
//! wraps the JSON body in a text content block.

use serde_json::{Value, json};

use crate::client::ProvreqClient;
use crate::error::HandlerError;
use crate::protocol::{CallToolResult, ToolDefinition};

/// Full tool set, statically built at process start. The
/// dispatcher in `transport.rs` looks up a handler by name.
pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        tool(
            "provreq_list_projects",
            "List every project mounted in the provreq System. Returns a summary per project.",
            json!({"type": "object", "properties": {}, "additionalProperties": false}),
        ),
        tool(
            "provreq_get_project",
            "Fetch a project's detail (name, description, artifacts path, and collection summaries).",
            json!({
                "type": "object",
                "properties": {
                    "slug": { "type": "string", "description": "Project slug." }
                },
                "required": ["slug"],
                "additionalProperties": false
            }),
        ),
        tool(
            "provreq_list_collections",
            "List every collection in a project.",
            json!({
                "type": "object",
                "properties": {
                    "slug": { "type": "string", "description": "Project slug." }
                },
                "required": ["slug"],
                "additionalProperties": false
            }),
        ),
        tool(
            "provreq_get_collection",
            "Fetch a collection's summary (name, description, artifact count, expectsCodeTrace).",
            json!({
                "type": "object",
                "properties": {
                    "slug": { "type": "string" },
                    "prefix": { "type": "string", "description": "Collection prefix, e.g. REQ." }
                },
                "required": ["slug", "prefix"],
                "additionalProperties": false
            }),
        ),
        tool(
            "provreq_list_artifacts",
            "List every artifact in a collection, with names, UUIDs, titles, shapes, and review state.",
            json!({
                "type": "object",
                "properties": {
                    "slug": { "type": "string" },
                    "prefix": { "type": "string" }
                },
                "required": ["slug", "prefix"],
                "additionalProperties": false
            }),
        ),
        tool(
            "provreq_get_artifact",
            "Fetch an artifact's full detail by UUID (title, body, links, tags, review log, etc.).",
            json!({
                "type": "object",
                "properties": {
                    "uuid": { "type": "string", "description": "Artifact UUID." }
                },
                "required": ["uuid"],
                "additionalProperties": false
            }),
        ),
        tool(
            "provreq_get_artifact_by_path",
            "Fetch an artifact by its human-readable {slug, prefix, name} triple — resolves to the UUID endpoint.",
            json!({
                "type": "object",
                "properties": {
                    "slug": { "type": "string" },
                    "prefix": { "type": "string" },
                    "name": { "type": "string", "description": "Artifact filename stem." }
                },
                "required": ["slug", "prefix", "name"],
                "additionalProperties": false
            }),
        ),
        tool(
            "provreq_get_incoming_links",
            "List every traceability link pointing at this artifact.",
            json!({
                "type": "object",
                "properties": {
                    "uuid": { "type": "string" }
                },
                "required": ["uuid"],
                "additionalProperties": false
            }),
        ),
        tool(
            "provreq_search",
            "Full-text search across artifacts (Tantivy). Supports field-scoped queries (title:, body:, tags:) and a shape / review-state filter set.",
            json!({
                "type": "object",
                "properties": {
                    "q": { "type": "string", "description": "Query string. Empty runs a match-all." },
                    "scope": { "type": "string", "description": "System, project, or collection scope." },
                    "shape": {
                        "type": "array",
                        "items": { "type": "string", "enum": ["content", "blob", "url"] }
                    },
                    "reviewState": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "includeInactive": { "type": "boolean" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200 },
                    "offset": { "type": "integer", "minimum": 0 }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "provreq_run_report",
            "Run a named report kind (unresolved-links, link-orphans, cycles, conflicts, coverage-matrix, impact-analysis, review-status, filesystem-orphans, code-traceability).",
            json!({
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "description": "Report kind." },
                    "scope": { "type": "string" },
                    "includeInactive": { "type": "boolean" }
                },
                "required": ["kind"],
                "additionalProperties": false
            }),
        ),
        tool(
            "provreq_get_graph",
            "Return the traceability graph as nodes + edges, scoped and optionally filtered by link type / tag.",
            json!({
                "type": "object",
                "properties": {
                    "scope": { "type": "string" },
                    "linkTypes": { "type": "array", "items": { "type": "string" } },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "includeInactive": { "type": "boolean" }
                },
                "additionalProperties": false
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, schema: Value) -> ToolDefinition {
    ToolDefinition {
        name: name.to_owned(),
        description: description.to_owned(),
        input_schema: schema,
    }
}

/// Dispatch a `tools/call` request to the right handler.
/// `args` is the raw `arguments` field from the MCP request,
/// possibly `null`.
pub async fn dispatch(
    client: &ProvreqClient,
    name: &str,
    args: Option<Value>,
) -> Result<CallToolResult, HandlerError> {
    let args = args.unwrap_or_else(|| json!({}));
    match name {
        "provreq_list_projects" => list_projects(client, args).await,
        "provreq_get_project" => get_project(client, args).await,
        "provreq_list_collections" => list_collections(client, args).await,
        "provreq_get_collection" => get_collection(client, args).await,
        "provreq_list_artifacts" => list_artifacts(client, args).await,
        "provreq_get_artifact" => get_artifact(client, args).await,
        "provreq_get_artifact_by_path" => get_artifact_by_path(client, args).await,
        "provreq_get_incoming_links" => get_incoming_links(client, args).await,
        "provreq_search" => search(client, args).await,
        "provreq_run_report" => run_report(client, args).await,
        "provreq_get_graph" => get_graph(client, args).await,
        other => Err(HandlerError::InvalidParams(format!(
            "unknown tool '{other}'"
        ))),
    }
}

// --- Handlers -------------------------------------------------------------

async fn list_projects(
    client: &ProvreqClient,
    _args: Value,
) -> Result<CallToolResult, HandlerError> {
    let body = client.get_json("/api/projects").await?;
    Ok(json_result(&body))
}

async fn get_project(client: &ProvreqClient, args: Value) -> Result<CallToolResult, HandlerError> {
    let slug = require_string(&args, "slug")?;
    let body = client
        .get_json(&format!("/api/projects/{}", encode(&slug)))
        .await?;
    Ok(json_result(&body))
}

async fn list_collections(
    client: &ProvreqClient,
    args: Value,
) -> Result<CallToolResult, HandlerError> {
    let slug = require_string(&args, "slug")?;
    let body = client
        .get_json(&format!("/api/projects/{}/collections", encode(&slug)))
        .await?;
    Ok(json_result(&body))
}

async fn get_collection(
    client: &ProvreqClient,
    args: Value,
) -> Result<CallToolResult, HandlerError> {
    let slug = require_string(&args, "slug")?;
    let prefix = require_string(&args, "prefix")?;
    let body = client
        .get_json(&format!(
            "/api/projects/{}/collections/{}",
            encode(&slug),
            encode(&prefix)
        ))
        .await?;
    Ok(json_result(&body))
}

async fn list_artifacts(
    client: &ProvreqClient,
    args: Value,
) -> Result<CallToolResult, HandlerError> {
    let slug = require_string(&args, "slug")?;
    let prefix = require_string(&args, "prefix")?;
    let body = client
        .get_json(&format!(
            "/api/projects/{}/collections/{}/artifacts",
            encode(&slug),
            encode(&prefix)
        ))
        .await?;
    Ok(json_result(&body))
}

async fn get_artifact(
    client: &ProvreqClient,
    args: Value,
) -> Result<CallToolResult, HandlerError> {
    let uuid = require_string(&args, "uuid")?;
    let body = client
        .get_json(&format!("/api/artifacts/{}", encode(&uuid)))
        .await?;
    Ok(json_result(&body))
}

async fn get_artifact_by_path(
    client: &ProvreqClient,
    args: Value,
) -> Result<CallToolResult, HandlerError> {
    let slug = require_string(&args, "slug")?;
    let prefix = require_string(&args, "prefix")?;
    let name = require_string(&args, "name")?;
    // Walk the artifacts listing for the collection and pick
    // the matching stem. The REST API doesn't offer a direct
    // /api/projects/{s}/collections/{p}/artifacts/{n} GET, so
    // this is the most portable resolution.
    let listing = client
        .get_json(&format!(
            "/api/projects/{}/collections/{}/artifacts",
            encode(&slug),
            encode(&prefix)
        ))
        .await?;
    let entries = listing
        .as_array()
        .ok_or_else(|| HandlerError::Upstream("artifacts listing was not an array".into()))?;
    let matched = entries.iter().find(|a| {
        a.get("name")
            .and_then(|v| v.as_str())
            .map(|n| n == name)
            .unwrap_or(false)
    });
    let uuid = matched
        .and_then(|a| a.get("uuid").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            HandlerError::InvalidParams(format!("no artifact '{name}' in {slug}/{prefix}"))
        })?;
    let body = client
        .get_json(&format!("/api/artifacts/{}", encode(uuid)))
        .await?;
    Ok(json_result(&body))
}

async fn get_incoming_links(
    client: &ProvreqClient,
    args: Value,
) -> Result<CallToolResult, HandlerError> {
    let uuid = require_string(&args, "uuid")?;
    let body = client
        .get_json(&format!("/api/artifacts/{}/incoming-links", encode(&uuid)))
        .await?;
    Ok(json_result(&body))
}

async fn search(client: &ProvreqClient, args: Value) -> Result<CallToolResult, HandlerError> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(q) = args.get("q").and_then(|v| v.as_str()) {
        parts.push(format!("q={}", urlencoding(q)));
    }
    if let Some(s) = args.get("scope").and_then(|v| v.as_str())
        && !s.is_empty()
    {
        parts.push(format!("scope={}", urlencoding(s)));
    }
    if let Some(arr) = args.get("shape").and_then(|v| v.as_array()) {
        for v in arr {
            if let Some(s) = v.as_str() {
                parts.push(format!("shape={}", urlencoding(s)));
            }
        }
    }
    if let Some(arr) = args.get("reviewState").and_then(|v| v.as_array()) {
        for v in arr {
            if let Some(s) = v.as_str() {
                parts.push(format!("reviewState={}", urlencoding(s)));
            }
        }
    }
    if args.get("includeInactive").and_then(|v| v.as_bool()) == Some(true) {
        parts.push("includeInactive=true".into());
    }
    if let Some(n) = args.get("limit").and_then(|v| v.as_u64()) {
        parts.push(format!("limit={n}"));
    }
    if let Some(n) = args.get("offset").and_then(|v| v.as_u64()) {
        parts.push(format!("offset={n}"));
    }
    let qs = if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    };
    let body = client.get_json(&format!("/api/search{qs}")).await?;
    Ok(json_result(&body))
}

async fn run_report(client: &ProvreqClient, args: Value) -> Result<CallToolResult, HandlerError> {
    let kind = require_string(&args, "kind")?;
    let mut parts: Vec<String> = Vec::new();
    if let Some(s) = args.get("scope").and_then(|v| v.as_str())
        && !s.is_empty()
    {
        parts.push(format!("scope={}", urlencoding(s)));
    }
    if args.get("includeInactive").and_then(|v| v.as_bool()) == Some(true) {
        parts.push("includeInactive=true".into());
    }
    let qs = if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    };
    let body = client
        .get_json(&format!("/api/reports/{}{qs}", encode(&kind)))
        .await?;
    Ok(json_result(&body))
}

async fn get_graph(client: &ProvreqClient, args: Value) -> Result<CallToolResult, HandlerError> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(s) = args.get("scope").and_then(|v| v.as_str())
        && !s.is_empty()
    {
        parts.push(format!("scope={}", urlencoding(s)));
    }
    if let Some(arr) = args.get("linkTypes").and_then(|v| v.as_array()) {
        let joined: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
        if !joined.is_empty() {
            parts.push(format!("linkTypes={}", urlencoding(&joined.join(","))));
        }
    }
    if let Some(arr) = args.get("tags").and_then(|v| v.as_array()) {
        let joined: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
        if !joined.is_empty() {
            parts.push(format!("tags={}", urlencoding(&joined.join(","))));
        }
    }
    if args.get("includeInactive").and_then(|v| v.as_bool()) == Some(true) {
        parts.push("includeInactive=true".into());
    }
    let qs = if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    };
    let body = client.get_json(&format!("/api/graph{qs}")).await?;
    Ok(json_result(&body))
}

// --- Helpers --------------------------------------------------------------

fn json_result(body: &Value) -> CallToolResult {
    CallToolResult::text(serde_json::to_string_pretty(body).unwrap_or_else(|_| body.to_string()))
}

fn require_string(args: &Value, field: &str) -> Result<String, HandlerError> {
    args.get(field)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| HandlerError::InvalidParams(format!("missing required string '{field}'")))
}

fn encode(s: &str) -> String {
    encode_path(s)
}

/// URL-encode a single path / query component. Exposed so the
/// resources module can share the same minimal encoder without
/// pulling in a new crate.
pub fn encode_path(s: &str) -> String {
    urlencoding(s)
}

fn urlencoding(s: &str) -> String {
    // Percent-encode the small set that appears in path /
    // query components. Avoids pulling in a new crate for one
    // narrow use.
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let safe = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if safe {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_client(base: &str) -> ProvreqClient {
        ProvreqClient::new(Url::parse(base).unwrap())
    }

    #[test]
    fn tool_definitions_are_exactly_eleven() {
        let defs = tool_definitions();
        assert_eq!(defs.len(), 11);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"provreq_list_projects"));
        assert!(names.contains(&"provreq_search"));
        assert!(names.contains(&"provreq_get_graph"));
    }

    #[test]
    fn every_tool_has_object_schema_with_additional_properties_false() {
        for def in tool_definitions() {
            let t = def.input_schema.get("type").and_then(|v| v.as_str());
            assert_eq!(
                t,
                Some("object"),
                "tool {} must have object schema",
                def.name
            );
            assert_eq!(
                def.input_schema
                    .get("additionalProperties")
                    .and_then(|v| v.as_bool()),
                Some(false),
                "tool {} must deny additional properties",
                def.name
            );
        }
    }

    #[tokio::test]
    async fn list_projects_returns_server_body_as_text_block() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "slug": "a", "name": "Alpha" }
            ])))
            .mount(&server)
            .await;
        let client = make_client(&server.uri());
        let out = dispatch(&client, "provreq_list_projects", None)
            .await
            .unwrap();
        let text = match &out.content[0] {
            crate::protocol::ContentBlock::Text { text } => text,
        };
        assert!(text.contains("Alpha"));
        assert!(text.contains("\"slug\""));
    }

    #[tokio::test]
    async fn get_project_validates_required_args() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let err = dispatch(&client, "provreq_get_project", Some(serde_json::json!({})))
            .await
            .unwrap_err();
        match err {
            HandlerError::InvalidParams(m) => assert!(m.contains("slug")),
            other => panic!("expected InvalidParams, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_artifact_by_path_walks_listing_and_follows_uuid() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects/sample/collections/REQ/artifacts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "name": "REQ-a", "uuid": "11111111-1111-1111-1111-111111111111" },
                { "name": "REQ-b", "uuid": "22222222-2222-2222-2222-222222222222" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/artifacts/22222222-2222-2222-2222-222222222222"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uuid": "22222222-2222-2222-2222-222222222222",
                "title": "Second artifact"
            })))
            .mount(&server)
            .await;

        let client = make_client(&server.uri());
        let out = dispatch(
            &client,
            "provreq_get_artifact_by_path",
            Some(serde_json::json!({
                "slug": "sample", "prefix": "REQ", "name": "REQ-b"
            })),
        )
        .await
        .unwrap();
        let text = match &out.content[0] {
            crate::protocol::ContentBlock::Text { text } => text,
        };
        assert!(text.contains("Second artifact"));
    }

    #[tokio::test]
    async fn get_artifact_by_path_errors_with_invalid_params_when_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects/sample/collections/REQ/artifacts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;
        let client = make_client(&server.uri());
        let err = dispatch(
            &client,
            "provreq_get_artifact_by_path",
            Some(serde_json::json!({
                "slug": "sample", "prefix": "REQ", "name": "REQ-ghost"
            })),
        )
        .await
        .unwrap_err();
        match err {
            HandlerError::InvalidParams(m) => assert!(m.contains("REQ-ghost")),
            other => panic!("expected InvalidParams, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn search_builds_query_string_from_args() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .and(query_param("q", "hello world"))
            .and(query_param("scope", "project:foo"))
            .and(query_param("includeInactive", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hits": [], "total": 0
            })))
            .mount(&server)
            .await;
        let client = make_client(&server.uri());
        let _ = dispatch(
            &client,
            "provreq_search",
            Some(serde_json::json!({
                "q": "hello world",
                "scope": "project:foo",
                "includeInactive": true
            })),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn run_report_routes_kind_into_the_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/reports/unresolved-links"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "kind": "unresolved-links", "links": []
            })))
            .mount(&server)
            .await;
        let client = make_client(&server.uri());
        let out = dispatch(
            &client,
            "provreq_run_report",
            Some(serde_json::json!({ "kind": "unresolved-links" })),
        )
        .await
        .unwrap();
        let text = match &out.content[0] {
            crate::protocol::ContentBlock::Text { text } => text,
        };
        assert!(text.contains("unresolved-links"));
    }

    #[tokio::test]
    async fn upstream_http_error_propagates_as_handler_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .respond_with(ResponseTemplate::new(500).set_body_string("{\"error\":\"boom\"}"))
            .mount(&server)
            .await;
        let client = make_client(&server.uri());
        let err = dispatch(&client, "provreq_list_projects", None)
            .await
            .unwrap_err();
        assert!(matches!(err, HandlerError::Upstream(_)));
        assert!(err.to_string().contains("HTTP 500"));
    }

    #[tokio::test]
    async fn unknown_tool_name_surfaces_as_invalid_params() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let err = dispatch(&client, "provreq_does_not_exist", None)
            .await
            .unwrap_err();
        match err {
            HandlerError::InvalidParams(m) => assert!(m.contains("does_not_exist")),
            other => panic!("expected InvalidParams, got {other:?}"),
        }
    }
}
