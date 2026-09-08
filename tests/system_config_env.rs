//! Regression for #422: `provreq serve` must honor `PROVREQ_SYSTEM_CONFIG`.
//!
//! Before the fix, `single_subject_state` hardcoded `system_config_path:
//! None` and nothing read the env var the 409 / `/api/system` responses
//! name, so a system config could never load. This test drives the real
//! serve construction path (`single_subject_state` + `server::router`) with
//! the env var set and asserts the config loads.
//!
//! It lives in its own test binary on purpose: `PROVREQ_SYSTEM_CONFIG` is
//! process-global, so isolating it here keeps it from leaking into the
//! single-subject state other test files build.

mod support;

use std::os::unix::fs::PermissionsExt;

use axum::http::StatusCode;
use support::{SUBJECT_SLUG, get_json, write_project};

#[tokio::test]
async fn serve_loads_system_config_named_by_env_var() {
    let temp = tempfile::tempdir().unwrap();
    let subject = temp.path().join(SUBJECT_SLUG);
    write_project(&subject, SUBJECT_SLUG);

    // A valid System config, mode 0600 (loader rejects group/other bits).
    let sys = temp.path().join("system.json");
    std::fs::write(&sys, r#"{"schemaVersion":1,"name":"t","projects":[]}"#).unwrap();
    std::fs::set_permissions(&sys, std::fs::Permissions::from_mode(0o600)).unwrap();

    // SAFETY: single test in a dedicated binary; no other thread reads env here.
    unsafe { std::env::set_var("PROVREQ_SYSTEM_CONFIG", &sys) };
    let state = provreq::server::single_subject_state(subject)
        .await
        .unwrap();
    let router = provreq::server::router(state);
    let (status, body) = get_json(&router, "/api/system").await;
    unsafe { std::env::remove_var("PROVREQ_SYSTEM_CONFIG") };

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["loaded"], true,
        "system config named by PROVREQ_SYSTEM_CONFIG should load: {body}"
    );
    assert_eq!(body["name"], "t");
}
