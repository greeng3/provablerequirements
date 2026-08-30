//! Phase 10c.2 end-to-end test for `provreq-mcp`.
//!
//! Spawns the compiled binary as a child process, feeds it
//! JSON-RPC messages over stdin, and reads the responses off
//! stdout. A `wiremock::MockServer` stands in for a running
//! `provreq server` — the binary sees the same HTTP shapes
//! either way, and keeping `provreq server` out of the mcp
//! crate's dep graph is part of the "standalone binary" locked
//! decision.
//!
//! Exercises the full `initialize → tools/list → tools/call →
//! resources/list → resources/read → prompts/list → prompts/get`
//! round-trip.

use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BINARY: &str = env!("CARGO_BIN_EXE_provreq-mcp");

/// Convenience: a line-by-line framed JSON-RPC client over
/// the child process's pipes. Every request gets sent with a
/// trailing newline, every response is read as a single line.
struct McpClient {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    /// Held to keep stderr drained so the child doesn't block
    /// on a full stderr buffer during long tests. Dropped at
    /// shutdown.
    _stderr: ChildStderr,
}

impl McpClient {
    async fn send(&mut self, req: Value) {
        let line = req.to_string();
        self.stdin.write_all(line.as_bytes()).await.unwrap();
        self.stdin.write_all(b"\n").await.unwrap();
        self.stdin.flush().await.unwrap();
    }

    async fn recv(&mut self) -> Value {
        let mut buf = String::new();
        let read = timeout(Duration::from_secs(5), self.stdout.read_line(&mut buf))
            .await
            .expect("stdout read timed out — child process may have hung")
            .expect("stdout read failed");
        assert!(read > 0, "stdout closed before response arrived");
        serde_json::from_str(&buf)
            .unwrap_or_else(|e| panic!("response is not JSON: {e}\nraw: {buf}"))
    }

    async fn call(&mut self, id: i64, method: &str, params: Option<Value>) -> Value {
        let mut request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
        });
        if let Some(p) = params {
            request["params"] = p;
        }
        self.send(request).await;
        self.recv().await
    }
}

/// Verifies: REQ081
#[tokio::test]
async fn full_mcp_round_trip_against_mock_provreq_server() {
    let server = fixture_server().await;

    let mut child = Command::new(BINARY)
        .arg("--url")
        .arg(server.uri())
        .arg("--allow-remote") // wiremock binds to 127.0.0.1 — loopback — but
        // server.uri() sometimes returns a form like
        // `http://127.0.0.1:PORT` which *is* loopback, so
        // --allow-remote is a safety belt only. Keep it in
        // case a future OS returns `localhost` in some form
        // that trips the gate.
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn provreq-mcp");

    let mut mcp = McpClient {
        stdin: child.stdin.take().expect("stdin pipe"),
        stdout: BufReader::new(child.stdout.take().expect("stdout pipe")),
        _stderr: child.stderr.take().expect("stderr pipe"),
    };

    // --- initialize -------------------------------------------------------
    let init = mcp.call(1, "initialize", None).await;
    assert_eq!(init["id"], 1);
    let init_result = &init["result"];
    assert_eq!(init_result["protocolVersion"], "2024-11-05");
    assert_eq!(init_result["serverInfo"]["name"], "provreq-mcp");
    assert!(init_result["capabilities"]["tools"].is_object());
    assert!(init_result["capabilities"]["resources"].is_object());
    assert!(init_result["capabilities"]["prompts"].is_object());

    // --- tools/list -------------------------------------------------------
    let tools = mcp.call(2, "tools/list", None).await;
    let tool_arr = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tool_arr.len(), 16);

    // --- tools/call (list_projects) --------------------------------------
    let call = mcp
        .call(
            3,
            "tools/call",
            Some(json!({ "name": "provreq_list_projects" })),
        )
        .await;
    let content = call["result"]["content"].as_array().unwrap();
    let text = content[0]["text"].as_str().unwrap();
    assert!(text.contains("\"slug\""));
    assert!(text.contains("sample"));

    // --- resources/list --------------------------------------------------
    let list = mcp.call(4, "resources/list", None).await;
    let resources = list["result"]["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(
        resources[0]["uri"],
        "provreq://artifact/11111111-1111-1111-1111-111111111111"
    );
    assert_eq!(resources[0]["name"], "sample/REQ/REQ-one");

    // --- resources/read --------------------------------------------------
    let read = mcp
        .call(
            5,
            "resources/read",
            Some(json!({
                "uri": "provreq://artifact/11111111-1111-1111-1111-111111111111"
            })),
        )
        .await;
    let contents = read["result"]["contents"].as_array().unwrap();
    let body = contents[0]["text"].as_str().unwrap();
    assert!(body.contains("# Pressure envelope"));
    assert!(body.contains("The system shall maintain pressure"));
    assert_eq!(contents[0]["mimeType"].as_str(), Some("text/markdown"));

    // --- prompts/list ----------------------------------------------------
    let prompts = mcp.call(6, "prompts/list", None).await;
    let prompt_arr = prompts["result"]["prompts"].as_array().unwrap();
    assert_eq!(prompt_arr.len(), 6);

    // --- prompts/get -----------------------------------------------------
    let got = mcp
        .call(
            7,
            "prompts/get",
            Some(json!({
                "name": "gap_analysis",
                "arguments": { "scope": "project:sample" }
            })),
        )
        .await;
    let msg_text = got["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap();
    assert!(msg_text.contains("project:sample"));

    // --- ping ------------------------------------------------------------
    let pong = mcp.call(8, "ping", None).await;
    assert_eq!(pong["result"], json!({}));

    // --- shutdown --------------------------------------------------------
    drop(mcp.stdin);
    let status = timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("child process did not exit after stdin closed")
        .expect("child wait failed");
    assert!(status.success(), "child exit status: {status:?}");
}

/// Program a mock server with the minimum set of endpoints the
/// E2E flow exercises. Keep bodies small — the test reads them
/// back via MCP, not for correctness of the server itself.
async fn fixture_server() -> MockServer {
    let server = MockServer::start().await;

    // Projects → Collections → Artifacts walk for resources/list.
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "slug": "sample", "name": "Sample Project" }
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
                "title": "Pressure envelope",
                "shape": "content"
            }
        ])))
        .mount(&server)
        .await;

    // resources/read + tools/call(get_artifact) target.
    Mock::given(method("GET"))
        .and(path("/api/artifacts/11111111-1111-1111-1111-111111111111"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "uuid": "11111111-1111-1111-1111-111111111111",
            "name": "REQ-one",
            "title": "Pressure envelope",
            "projectSlug": "sample",
            "collectionPrefix": "REQ",
            "shape": "content",
            "body": "The system shall maintain pressure within the envelope."
        })))
        .mount(&server)
        .await;

    server
}
