//! Claude Code CLI transport for the Anthropic family (REQ089).
//!
//! Instead of the HTTP Messages API, this adapter shells out to the
//! local `claude` binary in headless mode (`-p --output-format json`).
//! `claude` authenticates through the operator's own logged-in session,
//! so generation draws on their Claude subscription rather than an API
//! credit balance — the whole reason the transport exists.
//!
//! Two obligations the HTTP transport doesn't carry (REQ089):
//! the prompt is untrusted requirement text, so it is delivered on the
//! child's stdin, never assembled into an argument or a shell line; and
//! the child is spawned with no tool access (`--allowedTools ""`) and in
//! print mode, so a generation can neither touch the server's files nor
//! block on an interactive permission prompt.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use crate::llm::config::ProviderFamily;
use crate::llm::provider::{
    Adapter, AdapterError, BoxFuture, PromptRequest, PromptResponse, PromptRole, PromptUsage,
};

const FAMILY: &str = "anthropic";
const DEFAULT_PROGRAM: &str = "claude";
const TIMEOUT_MS: u64 = 30_000;

pub struct ClaudeCodeAdapter {
    program: String,
    model: String,
    /// Resolved once at construction: does `program` exist on the
    /// server's `PATH`? Surfaced as `api_key_available` so the chain
    /// skips a slot with no binary before spawning a doomed process,
    /// exactly as it skips an HTTP slot with no key. Whether the stored
    /// session is *authenticated* can't be known without running, so —
    /// like an invalid HTTP key — that surfaces as an `Auth` failure at
    /// send time.
    available: bool,
}

impl ClaudeCodeAdapter {
    pub fn new(model: String) -> Self {
        Self::with_program(DEFAULT_PROGRAM.to_owned(), model)
    }

    /// Construct against an explicit binary path. Used by `new` with the
    /// default `claude`, and by tests pointing at a stub.
    pub fn with_program(program: String, model: String) -> Self {
        let available = binary_on_path(&program);
        Self {
            program,
            model,
            available,
        }
    }
}

/// Fold the generic prompt into the single text `claude -p` reads from
/// stdin. The system prime rides `--append-system-prompt`; the
/// conversation is joined turn-by-turn. ponytail: role labels are added
/// only for multi-turn input — the real callers (rename/triage/translate)
/// send one user turn, which passes through verbatim.
fn stdin_prompt(req: &PromptRequest) -> String {
    if req.messages.len() == 1 {
        return req.messages[0].content.clone();
    }
    req.messages
        .iter()
        .map(|m| {
            let role = match m.role {
                PromptRole::User => "User",
                PromptRole::Assistant => "Assistant",
            };
            format!("{role}: {}", m.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The `--output-format json` envelope. We read only the answer text and
/// (best-effort) token usage; `is_error` flags a request the CLI itself
/// rejected while still exiting cleanly.
#[derive(Deserialize)]
struct CliEnvelope {
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    usage: Option<CliUsage>,
}

#[derive(Deserialize)]
struct CliUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

/// Is `program` runnable — an explicit path that is a file, or a bare
/// name found on `PATH`? Cheap enough to run at construction.
fn binary_on_path(program: &str) -> bool {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return Path::new(program).is_file();
    }
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(program).is_file())
}

/// Classify a non-zero exit. The CLI's own stderr is the message that
/// names what to fix, so it rides along either way. An auth-shaped
/// stderr becomes `Auth` (a subscription that isn't logged in); anything
/// else is `Rejected` — both permanent, so the health tracker won't spin
/// on them.
fn exit_to_error(code: Option<i32>, stderr: &str) -> AdapterError {
    let lower = stderr.to_ascii_lowercase();
    let looks_like_auth = [
        "not logged in",
        "unauthorized",
        "authenticate",
        "log in",
        "login",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    if looks_like_auth {
        return AdapterError::Auth {
            family: FAMILY,
            detail: format!("claude CLI is not authenticated: {}", stderr.trim()),
        };
    }
    AdapterError::Rejected {
        family: FAMILY,
        status: code.unwrap_or(-1).unsigned_abs() as u16,
        detail: stderr.trim().to_owned(),
    }
}

impl Adapter for ClaudeCodeAdapter {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::Anthropic
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn endpoint(&self) -> &str {
        "claude-code-cli (local)"
    }
    fn api_key_available(&self) -> bool {
        self.available
    }

    fn send_prompt<'a>(
        &'a self,
        req: &'a PromptRequest,
    ) -> BoxFuture<'a, Result<PromptResponse, AdapterError>> {
        Box::pin(async move {
            let prompt = stdin_prompt(req);
            let timeout_ms = req.timeout_ms.unwrap_or(TIMEOUT_MS);

            let mut cmd = tokio::process::Command::new(&self.program);
            cmd.arg("-p")
                .args(["--output-format", "json"])
                .args(["--model", &self.model])
                // No tool access to the host, ever. Print mode is already
                // non-interactive, so with no tools there is nothing to
                // prompt for — the child cannot block on a permission ask.
                .args(["--allowedTools", ""])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                // A timed-out child is reaped when we drop it, not orphaned.
                .kill_on_drop(true);
            if let Some(system) = &req.system {
                cmd.args(["--append-system-prompt", system]);
            }

            let mut child = cmd.spawn().map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AdapterError::Connection {
                        family: FAMILY,
                        detail: format!("`{}` not found on PATH", self.program),
                    }
                } else {
                    AdapterError::Connection {
                        family: FAMILY,
                        detail: e.to_string(),
                    }
                }
            })?;

            // Write the untrusted prompt to stdin and close it, then wait
            // for the process to finish — under the request's deadline.
            let run = async {
                if let Some(mut stdin) = child.stdin.take() {
                    // A child that rejects the request and exits before reading its prompt
                    // closes the read end, so the write fails with BrokenPipe. That must not
                    // shadow the real outcome — the child's exit status and stderr — with a
                    // misleading "connection" error, so swallow a BrokenPipe here and let
                    // wait_with_output + exit classification speak. Any other write error is a
                    // genuine I/O fault and propagates.
                    match stdin.write_all(prompt.as_bytes()).await {
                        Ok(()) => {
                            let _ = stdin.shutdown().await;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
                        Err(e) => return Err(e),
                    }
                }
                child.wait_with_output().await
            };
            let output = match tokio::time::timeout(Duration::from_millis(timeout_ms), run).await {
                Err(_) => {
                    return Err(AdapterError::Timeout {
                        family: FAMILY,
                        ms: timeout_ms,
                    });
                }
                Ok(Err(e)) => {
                    return Err(AdapterError::Connection {
                        family: FAMILY,
                        detail: e.to_string(),
                    });
                }
                Ok(Ok(output)) => output,
            };

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(exit_to_error(output.status.code(), &stderr));
            }

            let envelope: CliEnvelope =
                serde_json::from_slice(&output.stdout).map_err(|e| AdapterError::Malformed {
                    family: FAMILY,
                    detail: format!("could not parse `claude --output-format json`: {e}"),
                })?;

            if envelope.is_error {
                return Err(AdapterError::Rejected {
                    family: FAMILY,
                    status: 0,
                    detail: envelope
                        .subtype
                        .or(envelope.result)
                        .unwrap_or_else(|| "claude reported an error result".to_owned()),
                });
            }

            let text = envelope.result.unwrap_or_default();
            if text.is_empty() {
                return Err(AdapterError::Malformed {
                    family: FAMILY,
                    detail: "claude returned an empty result".into(),
                });
            }
            let usage = envelope.usage.map(|u| PromptUsage {
                input_tokens: u.input_tokens,
                output_tokens: u.output_tokens,
            });
            Ok(PromptResponse { text, usage })
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::llm::provider::PromptMessage;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    /// A fresh temp dir unique to this test invocation.
    fn scratch(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("provreq-cc-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Write an executable `claude` stub running `body` and return its path.
    fn stub(dir: &Path, body: &str) -> String {
        let path = dir.join("claude");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn req(system: Option<&str>, user: &str) -> PromptRequest {
        PromptRequest {
            system: system.map(|s| s.to_owned()),
            messages: vec![PromptMessage {
                role: PromptRole::User,
                content: user.to_owned(),
            }],
            max_tokens: 256,
            temperature: 0.0,
            timeout_ms: Some(5_000),
        }
    }

    #[tokio::test]
    async fn success_returns_text_and_forwards_the_right_argv_and_stdin() {
        let dir = scratch("ok");
        let cap = dir.to_string_lossy().into_owned();
        let program = stub(
            &dir,
            &format!(
                r#"cat > "{cap}/stdin"
: > "{cap}/args"
for a in "$@"; do printf '%s\n' "$a" >> "{cap}/args"; done
printf '%s' '{{"is_error":false,"result":"hi there","usage":{{"input_tokens":11,"output_tokens":7}}}}'"#
            ),
        );

        let adapter = ClaudeCodeAdapter::with_program(program, "claude-opus-4-8".into());
        assert!(adapter.api_key_available(), "stub exists on disk");
        let resp = adapter
            .send_prompt(&req(Some("SYS"), "hello"))
            .await
            .unwrap();

        assert_eq!(resp.text, "hi there");
        let usage = resp.usage.unwrap();
        assert_eq!((usage.input_tokens, usage.output_tokens), (11, 7));

        // Prompt went in on stdin, not as an argument.
        assert_eq!(fs::read_to_string(dir.join("stdin")).unwrap(), "hello");
        let args = fs::read_to_string(dir.join("args")).unwrap();
        for expected in [
            "-p",
            "--output-format",
            "json",
            "--model",
            "claude-opus-4-8",
            "--allowedTools",
            "--append-system-prompt",
            "SYS",
        ] {
            assert!(
                args.lines().any(|l| l == expected),
                "missing arg {expected:?} in {args:?}"
            );
        }
        // The tool allowlist is passed empty — no host tool access.
        assert!(
            args.contains("--allowedTools\n\n"),
            "allowedTools not empty: {args:?}"
        );
    }

    #[tokio::test]
    async fn nonzero_exit_maps_to_rejected_with_stderr() {
        let dir = scratch("rej");
        let program = stub(&dir, "echo 'model claude-nope not found' 1>&2\nexit 3");
        let adapter = ClaudeCodeAdapter::with_program(program, "claude-nope".into());
        let err = adapter.send_prompt(&req(None, "hi")).await.unwrap_err();
        match err {
            AdapterError::Rejected { detail, .. } => assert!(detail.contains("not found")),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn auth_shaped_stderr_maps_to_auth() {
        let dir = scratch("auth");
        let program = stub(
            &dir,
            "echo 'Not logged in. Please run claude login.' 1>&2\nexit 1",
        );
        let adapter = ClaudeCodeAdapter::with_program(program, "claude-opus-4-8".into());
        let err = adapter.send_prompt(&req(None, "hi")).await.unwrap_err();
        assert!(matches!(err, AdapterError::Auth { .. }), "got {err:?}");
    }

    #[tokio::test]
    async fn malformed_stdout_maps_to_malformed() {
        let dir = scratch("bad");
        let program = stub(&dir, "printf 'this is not json'");
        let adapter = ClaudeCodeAdapter::with_program(program, "claude-opus-4-8".into());
        let err = adapter.send_prompt(&req(None, "hi")).await.unwrap_err();
        assert!(matches!(err, AdapterError::Malformed { .. }), "got {err:?}");
    }

    #[tokio::test]
    async fn missing_binary_is_unavailable_and_connection_error() {
        let adapter = ClaudeCodeAdapter::with_program(
            "/no/such/claude/binary".into(),
            "claude-opus-4-8".into(),
        );
        assert!(!adapter.api_key_available());
        let err = adapter.send_prompt(&req(None, "hi")).await.unwrap_err();
        assert!(
            matches!(err, AdapterError::Connection { .. }),
            "got {err:?}"
        );
    }
}
