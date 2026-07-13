//! provreq — PRL native provisioner + backend server (skeleton).
//!
//! This crate is intentionally minimal at this stage: just enough of a real
//! binary for the 6-target release pipeline to have something to build and
//! publish (issue #2). Real server routes and the embedded web UI arrive in a
//! follow-up issue.

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
