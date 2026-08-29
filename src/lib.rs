//! provreq — PRL native provisioner + backend server.
//!
//! The [`server`] module hosts the local HTTP backend and serves the embedded
//! web UI; [`health_json`] is the payload behind `GET /health`. The [`doorstop`]
//! and [`adopt`] modules back the `init` command that discovers a subject repo's
//! Doorstop layout and scaffolds its companion tree.

pub mod adopt;
// The reqforge application layer, absorbed in arc-2 slice 7 (#371): `app` holds the single-mount
// `AppState`; `http` holds the ~60 axum handlers + DTOs; `watcher` polls the subject and broadcasts
// change events for SSE. Configured single-subject (discovery finds one mount) per #370. Compiled
// in-tree but not yet wired into `serve` (that is 7b: router merge + proof graft).
pub mod app;
pub mod buildenv;
pub mod check;
pub mod contract_draft;
pub mod create;
pub mod creusot;
pub mod detail;
pub mod doorstop;
pub mod draft;
pub mod engine;
pub mod formalize;
pub mod grounding;
pub mod http;
pub mod kani;
pub mod llm;
pub mod lowering;
pub mod migrate;
pub mod mirror_draft;
pub mod monitor;
pub mod prl;
pub mod proving_env;
pub mod provision;
pub mod prusti;
pub mod report;
pub mod reqforge;
pub mod rust_adapter;
pub mod semantic_draft;
pub mod server;
pub mod source;
pub mod spec_paths;
pub mod status;
pub mod subject_tree;
pub mod tla_adapter;
pub mod tlc;
pub mod trace;
pub mod triage;
pub mod ui;
pub mod verdict;
pub mod verdict_store;
pub mod verify;
pub mod watcher;

/// The health payload the backend reports (and will later serve at `/health`).
/// Kept as a pure function so it is unit-testable without standing up a server.
///
/// Implements: REQ001 (native self-contained executable that hosts the backend).
pub fn health_json() -> String {
    format!(
        r#"{{"status":"ok","version":"{}"}}"#,
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    // Verifies: REQ001 (the binary produces its health payload with the build version).
    #[test]
    fn health_json_reports_ok_and_current_version() {
        let s = super::health_json();
        assert!(s.contains("\"status\":\"ok\""), "missing ok status: {s}");
        assert!(
            s.contains(env!("CARGO_PKG_VERSION")),
            "health payload must embed the crate version: {s}"
        );
    }
}
