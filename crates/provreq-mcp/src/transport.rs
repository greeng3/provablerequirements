//! Stdio JSON-RPC read/write loop + method dispatcher.
//!
//! Reads newline-delimited JSON from `stdin`, dispatches each
//! request to the right handler module, and writes the response
//! (also newline-delimited JSON) to `stdout`. Notifications
//! (requests with no `id`) are handled silently — `initialized`
//! is the only one MCP clients send us in 10c.

use serde_json::{Value, json};
use tokio::io::{
    AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader, Stdin, Stdout, stdin, stdout,
};

use crate::client::ProvreqClient;
use crate::error::HandlerError;
use crate::protocol::{
    CallToolParams, GetPromptParams, InitializeResult, JsonRpcErrorBody, JsonRpcRequest,
    JsonRpcResponse, ListToolsResult, PROTOCOL_VERSION, PromptsCapability, ReadResourceParams,
    ResourcesCapability, SERVER_NAME, SERVER_VERSION, ServerCapabilities, ServerInfo,
    ToolsCapability, error_codes,
};
use crate::{prompts, resources, tools};

/// Run the MCP server loop against the host process's stdio
/// streams. Returns when stdin reaches EOF (the client closed
/// the pipe) or an I/O error occurs.
pub async fn run(client: ProvreqClient) -> std::io::Result<()> {
    let stdin = stdin();
    let stdout = stdout();
    run_with_io(client, stdin, stdout).await
}

/// Testable variant that takes explicit I/O handles. The `R`
/// parameter is `Stdin` in production; the integration test
/// drives the child process from the outside so this layer
/// only sees the real `Stdin` / `Stdout`.
pub async fn run_with_io(
    client: ProvreqClient,
    input: Stdin,
    output: Stdout,
) -> std::io::Result<()> {
    let reader = BufReader::new(input);
    let mut writer = output;
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = handle_line(&client, trimmed).await {
            let encoded = serde_json::to_string(&response).unwrap_or_else(|e| {
                // Should never happen — JsonRpcResponse is all
                // serde-derived. If it does, surface a parse
                // error JSON-RPC response so the client sees
                // something.
                format!(
                    r#"{{"jsonrpc":"2.0","id":null,"error":{{"code":-32603,"message":"serialize: {e}"}}}}"#
                )
            });
            writer.write_all(encoded.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }
    }
    Ok(())
}

/// Handle one JSON-RPC message. Returns `None` for
/// notifications (no `id`) — MCP's `initialized` notification
/// is the main one we expect. Returns `Some(response)` for
/// requests, including parse-failure fallbacks.
pub async fn handle_line(client: &ProvreqClient, line: &str) -> Option<JsonRpcResponse> {
    let request: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return Some(JsonRpcResponse::error(
                Value::Null,
                JsonRpcErrorBody::new(error_codes::PARSE_ERROR, format!("parse error: {e}")),
            ));
        }
    };
    dispatch(client, request).await
}

async fn dispatch(client: &ProvreqClient, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let Some(id) = request.id.clone() else {
        // Notification — do nothing observable.
        // (`initialized` is the only one we expect.)
        return None;
    };
    match request.method.as_str() {
        "initialize" => Some(JsonRpcResponse::success(id, handle_initialize())),
        "ping" => Some(JsonRpcResponse::success(id, json!({}))),
        "tools/list" => Some(JsonRpcResponse::success(id, handle_list_tools())),
        "tools/call" => Some(handle_call_tool(client, id, request.params).await),
        "resources/list" => Some(handle_list_resources(client, id).await),
        "resources/read" => Some(handle_read_resource(client, id, request.params).await),
        "prompts/list" => Some(JsonRpcResponse::success(id, handle_list_prompts())),
        "prompts/get" => Some(handle_get_prompt(id, request.params)),
        other => Some(JsonRpcResponse::error(
            id,
            JsonRpcErrorBody::new(
                error_codes::METHOD_NOT_FOUND,
                format!("method '{other}' is not supported"),
            ),
        )),
    }
}

fn handle_initialize() -> Value {
    let result = InitializeResult {
        protocol_version: PROTOCOL_VERSION,
        capabilities: ServerCapabilities {
            tools: ToolsCapability {
                list_changed: false,
            },
            // `subscribe: false` — the MCP spec has an optional
            // subscription channel for resources; we don't push
            // update notifications in 10c since the underlying
            // provreq server CRUD surface doesn't push change
            // events to the MCP process. Clients poll with
            // `resources/list` when they want a fresh snapshot.
            resources: ResourcesCapability {
                subscribe: false,
                list_changed: false,
            },
            prompts: PromptsCapability {
                list_changed: false,
            },
        },
        server_info: ServerInfo {
            name: SERVER_NAME,
            version: SERVER_VERSION,
        },
    };
    serde_json::to_value(result).expect("initialize result is serde-derivable")
}

fn handle_list_tools() -> Value {
    let result = ListToolsResult {
        tools: tools::tool_definitions(),
    };
    serde_json::to_value(result).expect("list-tools result is serde-derivable")
}

async fn handle_list_resources(client: &ProvreqClient, id: Value) -> JsonRpcResponse {
    match resources::list_resources(client).await {
        Ok(result) => match serde_json::to_value(result) {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(
                id,
                JsonRpcErrorBody::new(
                    error_codes::INTERNAL_ERROR,
                    format!("serialize resources list: {e}"),
                ),
            ),
        },
        Err(err) => JsonRpcResponse::error(id, err.to_json_rpc()),
    }
}

async fn handle_read_resource(
    client: &ProvreqClient,
    id: Value,
    params: Option<Value>,
) -> JsonRpcResponse {
    let Some(params_value) = params else {
        return JsonRpcResponse::error(
            id,
            JsonRpcErrorBody::new(
                error_codes::INVALID_PARAMS,
                "resources/read requires params.uri",
            ),
        );
    };
    let typed: ReadResourceParams = match serde_json::from_value(params_value) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error(
                id,
                JsonRpcErrorBody::new(error_codes::INVALID_PARAMS, format!("invalid params: {e}")),
            );
        }
    };
    match resources::read_resource(client, typed).await {
        Ok(result) => match serde_json::to_value(result) {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(
                id,
                JsonRpcErrorBody::new(
                    error_codes::INTERNAL_ERROR,
                    format!("serialize resource contents: {e}"),
                ),
            ),
        },
        Err(err) => JsonRpcResponse::error(id, err.to_json_rpc()),
    }
}

fn handle_list_prompts() -> Value {
    serde_json::to_value(prompts::list_prompts()).expect("list-prompts result is serde-derivable")
}

fn handle_get_prompt(id: Value, params: Option<Value>) -> JsonRpcResponse {
    let Some(params_value) = params else {
        return JsonRpcResponse::error(
            id,
            JsonRpcErrorBody::new(
                error_codes::INVALID_PARAMS,
                "prompts/get requires params.name",
            ),
        );
    };
    let typed: GetPromptParams = match serde_json::from_value(params_value) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error(
                id,
                JsonRpcErrorBody::new(error_codes::INVALID_PARAMS, format!("invalid params: {e}")),
            );
        }
    };
    match prompts::get_prompt(typed) {
        Ok(result) => match serde_json::to_value(result) {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(
                id,
                JsonRpcErrorBody::new(
                    error_codes::INTERNAL_ERROR,
                    format!("serialize prompt result: {e}"),
                ),
            ),
        },
        Err(err) => JsonRpcResponse::error(id, err.to_json_rpc()),
    }
}

async fn handle_call_tool(
    client: &ProvreqClient,
    id: Value,
    params: Option<Value>,
) -> JsonRpcResponse {
    let Some(params_value) = params else {
        return JsonRpcResponse::error(
            id,
            JsonRpcErrorBody::new(
                error_codes::INVALID_PARAMS,
                "tools/call requires params.name + params.arguments",
            ),
        );
    };
    let call: CallToolParams = match serde_json::from_value(params_value) {
        Ok(c) => c,
        Err(e) => {
            return JsonRpcResponse::error(
                id,
                JsonRpcErrorBody::new(error_codes::INVALID_PARAMS, format!("invalid params: {e}")),
            );
        }
    };
    match tools::dispatch(client, &call.name, call.arguments).await {
        Ok(result) => match serde_json::to_value(result) {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(
                id,
                JsonRpcErrorBody::new(
                    error_codes::INTERNAL_ERROR,
                    format!("serialize tool result: {e}"),
                ),
            ),
        },
        Err(err @ HandlerError::InvalidParams(_)) => JsonRpcResponse::error(id, err.to_json_rpc()),
        Err(err) => {
            // Upstream / Internal errors surface as
            // `CallToolResult { is_error: true }` so the agent
            // can see them as tool output rather than a
            // protocol-level error. This matches the MCP
            // convention that "tool errors" ≠ "protocol
            // errors".
            let tool_result = crate::protocol::CallToolResult::error(err.to_string());
            let value = serde_json::to_value(tool_result).unwrap_or_else(|_| json!({}));
            JsonRpcResponse::success(id, value)
        }
    }
}

/// Used by the module's unit tests, and exposed so the
/// integration test in 10c.2 can drive the full loop without a
/// child process.
#[allow(dead_code)]
pub async fn dispatch_for_test(
    client: &ProvreqClient,
    request: JsonRpcRequest,
) -> Option<JsonRpcResponse> {
    dispatch(client, request).await
}

/// Write-end abstraction so the unit tests below can run
/// without an actual `Stdout` handle.
#[allow(dead_code)]
async fn write_response<W: AsyncWrite + Unpin>(
    writer: &mut W,
    response: &JsonRpcResponse,
) -> std::io::Result<()> {
    let encoded = serde_json::to_string(response)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    writer.write_all(encoded.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
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

    fn req(id: i64, method: &str, params: Option<Value>) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(id)),
            method: method.into(),
            params,
        }
    }

    #[tokio::test]
    async fn initialize_returns_protocol_version_and_capabilities() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(&client, req(1, "initialize", None))
            .await
            .unwrap();
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
        assert!(result["capabilities"]["tools"].is_object());
        assert!(result["capabilities"]["resources"].is_object());
        assert!(result["capabilities"]["prompts"].is_object());
    }

    #[tokio::test]
    async fn ping_returns_empty_result() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(&client, req(2, "ping", None))
            .await
            .unwrap();
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap(), json!({}));
    }

    #[tokio::test]
    async fn tools_list_returns_sixteen_tools() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(&client, req(3, "tools/list", None))
            .await
            .unwrap();
        let tools = resp.result.unwrap()["tools"].as_array().cloned().unwrap();
        assert_eq!(tools.len(), 16);
    }

    #[tokio::test]
    async fn tools_call_returns_tool_result_on_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "slug": "a" }
            ])))
            .mount(&server)
            .await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(
            &client,
            req(
                4,
                "tools/call",
                Some(json!({ "name": "provreq_list_projects" })),
            ),
        )
        .await
        .unwrap();
        assert!(resp.error.is_none());
        let content = resp.result.unwrap()["content"].as_array().cloned().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
    }

    #[tokio::test]
    async fn tools_call_upstream_error_returns_tool_error_not_protocol_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(
            &client,
            req(
                5,
                "tools/call",
                Some(json!({ "name": "provreq_list_projects" })),
            ),
        )
        .await
        .unwrap();
        // Per MCP convention: upstream failures surface as a
        // CallToolResult with is_error=true, not a JSON-RPC
        // error. This lets the agent read the error text like
        // any other tool output.
        assert!(
            resp.error.is_none(),
            "expected tool-level error, got JSON-RPC error"
        );
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn tools_call_invalid_params_returns_json_rpc_error() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(
            &client,
            req(
                6,
                "tools/call",
                Some(json!({ "name": "provreq_get_project" })),
            ),
        )
        .await
        .unwrap();
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, error_codes::INVALID_PARAMS);
        assert!(err.message.contains("slug"));
    }

    #[tokio::test]
    async fn resources_list_walks_the_server_and_returns_one_per_artifact() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "slug": "sample" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/projects/sample/collections"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "prefix": "REQ" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/projects/sample/collections/REQ/artifacts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "uuid": "aaaa1111-1111-1111-1111-111111111111", "name": "REQ-x", "title": "X" }
            ])))
            .mount(&server)
            .await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(&client, req(10, "resources/list", None))
            .await
            .unwrap();
        let body = resp.result.unwrap();
        let resources = body["resources"].as_array().unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(
            resources[0]["uri"],
            "provreq://artifact/aaaa1111-1111-1111-1111-111111111111"
        );
    }

    #[tokio::test]
    async fn resources_read_fetches_by_uuid_and_returns_markdown() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/artifacts/aaaa1111-1111-1111-1111-111111111111"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "uuid": "aaaa1111-1111-1111-1111-111111111111",
                "name": "REQ-x",
                "title": "X",
                "projectSlug": "sample",
                "collectionPrefix": "REQ",
                "shape": "content",
                "body": "body text"
            })))
            .mount(&server)
            .await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(
            &client,
            req(
                11,
                "resources/read",
                Some(json!({
                    "uri": "provreq://artifact/aaaa1111-1111-1111-1111-111111111111"
                })),
            ),
        )
        .await
        .unwrap();
        let body = resp.result.unwrap();
        let text = body["contents"][0]["text"].as_str().unwrap();
        assert!(text.contains("# X"));
        assert!(text.contains("body text"));
    }

    #[tokio::test]
    async fn resources_read_rejects_malformed_uri_with_invalid_params() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(
            &client,
            req(
                12,
                "resources/read",
                Some(json!({ "uri": "file:///etc/passwd" })),
            ),
        )
        .await
        .unwrap();
        assert_eq!(resp.error.unwrap().code, error_codes::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn prompts_list_returns_six_definitions() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(&client, req(13, "prompts/list", None))
            .await
            .unwrap();
        let body = resp.result.unwrap();
        let prompts = body["prompts"].as_array().unwrap();
        assert_eq!(prompts.len(), 6);
    }

    #[tokio::test]
    async fn prompts_get_fills_template_with_arguments() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(
            &client,
            req(
                14,
                "prompts/get",
                Some(json!({
                    "name": "implementation_planning",
                    "arguments": { "uuid": "bbbb2222-2222-2222-2222-222222222222" }
                })),
            ),
        )
        .await
        .unwrap();
        let body = resp.result.unwrap();
        let text = body["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains("bbbb2222-2222-2222-2222-222222222222"));
    }

    #[tokio::test]
    async fn prompts_get_missing_required_uuid_returns_invalid_params() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(
            &client,
            req(
                15,
                "prompts/get",
                Some(json!({ "name": "implementation_planning" })),
            ),
        )
        .await
        .unwrap();
        assert_eq!(resp.error.unwrap().code, error_codes::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn unknown_method_returns_method_not_found() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let resp = dispatch_for_test(&client, req(7, "weird/method", None))
            .await
            .unwrap();
        let err = resp.error.unwrap();
        assert_eq!(err.code, error_codes::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn notification_returns_no_response() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let note = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: None,
            method: "initialized".into(),
            params: None,
        };
        assert!(dispatch_for_test(&client, note).await.is_none());
    }

    #[tokio::test]
    async fn parse_error_returns_parse_error_code() {
        let server = MockServer::start().await;
        let client = make_client(&server.uri());
        let resp = handle_line(&client, "{ not json").await.unwrap();
        assert_eq!(resp.error.unwrap().code, error_codes::PARSE_ERROR);
    }
}
