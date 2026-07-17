//! provreq — PRL native provisioner + backend server.
//!
//! The [`server`] module hosts the local HTTP backend and serves the embedded
//! web UI; [`health_json`] is the payload behind `GET /health`. The [`doorstop`]
//! and [`adopt`] modules back the `init` command that discovers a subject repo's
//! Doorstop layout and scaffolds its companion tree.

pub mod adopt;
pub mod doorstop;
pub mod draft;
pub mod engine;
pub mod formalize;
pub mod grounding;
pub mod kani;
pub mod llm;
pub mod prl;
pub mod rust_adapter;
pub mod server;
pub mod source;
pub mod status;
pub mod triage;
pub mod verdict;

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
