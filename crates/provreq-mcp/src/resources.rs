//! MCP resources surface per `LLM-mcpResources`.
//!
//! Every provreq artifact is exposed as a readable MCP
//! resource with URI `provreq://artifact/{uuid}`. Agents get a
//! stable handle they can reference in conversation, and
//! `resources/read` fetches the artifact body so the content
//! lands directly in the agent's context window.
//!
//! `resources/list` walks projects → collections → artifacts
//! on every call. Flat list in 10c — pagination lands if a
//! concrete System size makes the agent-side UI unhappy.

use serde_json::Value;

use crate::client::ProvreqClient;
use crate::error::HandlerError;
use crate::protocol::{
    ListResourcesResult, ReadResourceParams, ReadResourceResult, ResourceContents,
    ResourceDefinition,
};

pub const URI_PREFIX: &str = "provreq://artifact/";

/// Walk the full System and emit one resource per artifact.
pub async fn list_resources(client: &ProvreqClient) -> Result<ListResourcesResult, HandlerError> {
    let projects = client.get_json("/api/projects").await?;
    let project_list = projects
        .as_array()
        .ok_or_else(|| HandlerError::Upstream("projects list was not an array".into()))?;
    let mut resources: Vec<ResourceDefinition> = Vec::new();
    for project in project_list {
        let Some(slug) = project.get("slug").and_then(|v| v.as_str()) else {
            continue;
        };
        let collections = client
            .get_json(&format!(
                "/api/projects/{}/collections",
                super::tools::encode_path(slug)
            ))
            .await?;
        let collection_list = match collections.as_array() {
            Some(l) => l,
            None => continue,
        };
        for collection in collection_list {
            let Some(prefix) = collection.get("prefix").and_then(|v| v.as_str()) else {
                continue;
            };
            let artifacts = client
                .get_json(&format!(
                    "/api/projects/{}/collections/{}/artifacts",
                    super::tools::encode_path(slug),
                    super::tools::encode_path(prefix),
                ))
                .await?;
            let artifact_list = match artifacts.as_array() {
                Some(l) => l,
                None => continue,
            };
            for artifact in artifact_list {
                let Some(entry) = resource_from_listing(slug, prefix, artifact) else {
                    continue;
                };
                resources.push(entry);
            }
        }
    }
    Ok(ListResourcesResult { resources })
}

/// Read one artifact by URI. URI shape is
/// `provreq://artifact/{uuid}` — anything else → InvalidParams.
pub async fn read_resource(
    client: &ProvreqClient,
    params: ReadResourceParams,
) -> Result<ReadResourceResult, HandlerError> {
    let uuid = parse_uuid_uri(&params.uri)?;
    let body = client
        .get_json(&format!(
            "/api/artifacts/{}",
            super::tools::encode_path(&uuid)
        ))
        .await?;
    let text = render_artifact_markdown(&body);
    Ok(ReadResourceResult {
        contents: vec![ResourceContents {
            uri: params.uri,
            mime_type: Some("text/markdown".into()),
            text,
        }],
    })
}

fn resource_from_listing(slug: &str, prefix: &str, artifact: &Value) -> Option<ResourceDefinition> {
    let uuid = artifact.get("uuid").and_then(|v| v.as_str())?;
    let name_stem = artifact.get("name").and_then(|v| v.as_str())?;
    let title = artifact
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let breadcrumb = format!("{slug}/{prefix}/{name_stem}");
    Some(ResourceDefinition {
        uri: format!("{URI_PREFIX}{uuid}"),
        name: breadcrumb,
        description: if title.is_empty() {
            None
        } else {
            Some(title.to_owned())
        },
        mime_type: Some("text/markdown".into()),
    })
}

fn parse_uuid_uri(uri: &str) -> Result<String, HandlerError> {
    let Some(rest) = uri.strip_prefix(URI_PREFIX) else {
        return Err(HandlerError::InvalidParams(format!(
            "resource URI must start with {URI_PREFIX}; got '{uri}'"
        )));
    };
    if rest.is_empty() {
        return Err(HandlerError::InvalidParams(
            "resource URI missing UUID".into(),
        ));
    }
    // Keep just the UUID — strip any trailing path / query.
    let uuid_end = rest.find(['/', '?']).unwrap_or(rest.len());
    Ok(rest[..uuid_end].to_owned())
}

/// Render the artifact as a self-contained markdown document
/// the agent can drop straight into its context window. Shows
/// title, the human breadcrumb, UUID, tags, and the body.
/// Everything lives in markdown so the agent can reason about
/// it uniformly with other file-like context.
fn render_artifact_markdown(artifact: &Value) -> String {
    let title = artifact
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(no title)");
    let slug = artifact
        .get("projectSlug")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let prefix = artifact
        .get("collectionPrefix")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let name_stem = artifact.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let uuid = artifact.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
    let shape = artifact
        .get("shape")
        .and_then(|v| v.as_str())
        .unwrap_or("content");
    let body = artifact.get("body").and_then(|v| v.as_str()).unwrap_or("");

    let mut tags_line = String::new();
    if let Some(tags) = artifact.get("tags").and_then(|v| v.as_array())
        && !tags.is_empty()
    {
        let joined: Vec<&str> = tags.iter().filter_map(|v| v.as_str()).collect();
        tags_line = format!("\n**Tags:** {}", joined.join(", "));
    }

    format!(
        "# {title}\n\n\
         **Path:** `{slug}/{prefix}/{name_stem}`  \n\
         **UUID:** `{uuid}`  \n\
         **Shape:** `{shape}`{tags_line}\n\n\
         ---\n\n\
         {body}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use url::Url;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_client(base: &str) -> ProvreqClient {
        ProvreqClient::new(Url::parse(base).unwrap())
    }

    #[test]
    fn parse_uuid_uri_strips_prefix() {
        let uuid =
            parse_uuid_uri("provreq://artifact/11111111-1111-1111-1111-111111111111").unwrap();
        assert_eq!(uuid, "11111111-1111-1111-1111-111111111111");
    }

    #[test]
    fn parse_uuid_uri_rejects_wrong_prefix() {
        assert!(matches!(
            parse_uuid_uri("file:///not/an/artifact"),
            Err(HandlerError::InvalidParams(_))
        ));
        assert!(matches!(
            parse_uuid_uri("provreq://artifact/"),
            Err(HandlerError::InvalidParams(_))
        ));
    }

    #[test]
    fn render_markdown_includes_title_path_uuid_and_body() {
        let value = json!({
            "title": "Pressure envelope",
            "projectSlug": "sample",
            "collectionPrefix": "REQ",
            "name": "REQ-one",
            "uuid": "11111111-1111-1111-1111-111111111111",
            "shape": "content",
            "tags": ["critical", "phase-2"],
            "body": "The system shall maintain pressure."
        });
        let out = render_artifact_markdown(&value);
        assert!(out.starts_with("# Pressure envelope"));
        assert!(out.contains("sample/REQ/REQ-one"));
        assert!(out.contains("11111111-1111-1111-1111-111111111111"));
        assert!(out.contains("critical, phase-2"));
        assert!(out.contains("The system shall maintain pressure"));
    }

    #[tokio::test]
    async fn list_resources_walks_projects_collections_and_artifacts() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "slug": "sample", "name": "Sample" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/projects/sample/collections"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "prefix": "REQ", "name": "Requirements" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/projects/sample/collections/REQ/artifacts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {
                    "uuid": "11111111-1111-1111-1111-111111111111",
                    "name": "REQ-one",
                    "title": "First",
                    "shape": "content"
                },
                {
                    "uuid": "22222222-2222-2222-2222-222222222222",
                    "name": "REQ-two",
                    "title": "Second",
                    "shape": "content"
                }
            ])))
            .mount(&server)
            .await;

        let client = make_client(&server.uri());
        let out = list_resources(&client).await.unwrap();
        assert_eq!(out.resources.len(), 2);
        assert_eq!(
            out.resources[0].uri,
            "provreq://artifact/11111111-1111-1111-1111-111111111111"
        );
        assert_eq!(out.resources[0].name, "sample/REQ/REQ-one");
        assert_eq!(out.resources[0].description.as_deref(), Some("First"));
        assert_eq!(out.resources[0].mime_type.as_deref(), Some("text/markdown"));
    }

    #[tokio::test]
    async fn list_resources_tolerates_empty_system() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        let client = make_client(&server.uri());
        let out = list_resources(&client).await.unwrap();
        assert!(out.resources.is_empty());
    }

    #[tokio::test]
    async fn read_resource_fetches_by_uuid_and_renders_markdown() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/artifacts/11111111-1111-1111-1111-111111111111"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "title": "Pressure envelope",
                "projectSlug": "sample",
                "collectionPrefix": "REQ",
                "name": "REQ-one",
                "uuid": "11111111-1111-1111-1111-111111111111",
                "shape": "content",
                "body": "The system shall maintain pressure."
            })))
            .mount(&server)
            .await;

        let client = make_client(&server.uri());
        let out = read_resource(
            &client,
            ReadResourceParams {
                uri: "provreq://artifact/11111111-1111-1111-1111-111111111111".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(out.contents.len(), 1);
        assert_eq!(out.contents[0].mime_type.as_deref(), Some("text/markdown"));
        assert!(out.contents[0].text.contains("Pressure envelope"));
        assert!(out.contents[0].text.contains("sample/REQ/REQ-one"));
    }

    #[tokio::test]
    async fn read_resource_rejects_malformed_uri() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let err = read_resource(
            &client,
            ReadResourceParams {
                uri: "file:///etc/passwd".into(),
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, HandlerError::InvalidParams(_)));
    }
}
