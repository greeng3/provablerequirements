//! URL reachability check (Phase 5b), per `UX-urlArtifactChecking`.
//!
//! The check runs one HTTP request against a URL artifact's stored
//! URL and classifies the outcome into a small stable string set
//! that the client renders as a status pill. The classification is
//! intentionally coarse — ReqForge is not a monitoring tool, and
//! the operator experience is "is this link still pointing at
//! something sensible" rather than "what's the exact failure mode
//! today".
//!
//! Protocol per the locked Phase 5 decisions:
//!
//! - HEAD first; fall through to GET on `405 Method Not Allowed`,
//!   `501 Not Implemented`, or a mid-response connection reset.
//! - 10-second timeout (configurable at the client-builder level
//!   via `REQFORGE_URL_CHECK_TIMEOUT_SECS` in the server wiring).
//! - Up to 10 redirects (reqwest's default cap).
//! - No retries — a flaky endpoint should show as `timeout` and
//!   the operator re-runs the check.

use std::time::Duration;

use reqwest::{Method, StatusCode};

/// Stable set of outcome tags. The client renders the pill colour
/// from this set, so adding variants here means the UI gets a new
/// branch too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    Ok,
    NotFound,
    Forbidden,
    ServerError,
    RedirectError,
    Timeout,
    ConnectionRefused,
    TlsError,
    DnsError,
    Other,
}

impl CheckOutcome {
    /// The wire-format string. `serde` isn't involved because the
    /// outcome lands on a plain `String` field (`Artifact.check_status`)
    /// per `FORMAT-artifactMetadataSchema`.
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::NotFound => "not-found",
            Self::Forbidden => "forbidden",
            Self::ServerError => "server-error",
            Self::RedirectError => "redirect-error",
            Self::Timeout => "timeout",
            Self::ConnectionRefused => "connection-refused",
            Self::TlsError => "tls-error",
            Self::DnsError => "dns-error",
            Self::Other => "other",
        }
    }
}

/// Classify an HTTP status code into a `CheckOutcome`. Extracted so
/// the fallback GET path shares the mapping with the primary HEAD.
pub fn classify_status(status: StatusCode) -> CheckOutcome {
    match status.as_u16() {
        200..=299 => CheckOutcome::Ok,
        401 | 403 => CheckOutcome::Forbidden,
        404 | 410 => CheckOutcome::NotFound,
        500..=599 => CheckOutcome::ServerError,
        _ => CheckOutcome::Other,
    }
}

/// Classify a reqwest error into a `CheckOutcome`. Prefers the
/// most specific diagnosis (timeout / TLS / DNS) with `Other` as
/// the catch-all.
fn classify_error(err: &reqwest::Error) -> CheckOutcome {
    if err.is_timeout() {
        return CheckOutcome::Timeout;
    }
    if err.is_redirect() {
        return CheckOutcome::RedirectError;
    }

    // The cheapest way to distinguish TLS / DNS / connection
    // failures is to inspect the error message. reqwest exposes
    // `is_connect` for connect-phase failures but doesn't split
    // DNS vs refused vs TLS — peek at the message for the common
    // substrings.
    let msg = err.to_string().to_lowercase();
    if msg.contains("invalid certificate") || msg.contains("tls") || msg.contains("ssl") {
        return CheckOutcome::TlsError;
    }
    if msg.contains("dns") || msg.contains("failed to lookup") || msg.contains("no such host") {
        return CheckOutcome::DnsError;
    }
    if err.is_connect() && msg.contains("refused") {
        return CheckOutcome::ConnectionRefused;
    }
    CheckOutcome::Other
}

/// Live HTTP client used by the check handler. Wrapped so the
/// handler can stuff it onto `AppState` once and reuse the
/// connection pool across requests.
#[derive(Debug, Clone)]
pub struct UrlCheckClient {
    client: reqwest::Client,
}

impl UrlCheckClient {
    /// Build a client with the Phase 5 defaults.
    pub fn new(timeout_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!(
                "reqforge/",
                env!("CARGO_PKG_VERSION"),
                " (+url-check)"
            ))
            .timeout(Duration::from_secs(timeout_secs.max(1)))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("reqwest client with rustls should build");
        Self { client }
    }

    /// Run the HEAD-then-GET check against a URL string.
    pub async fn check(&self, url: &str) -> CheckOutcome {
        match self.request(Method::HEAD, url).await {
            Ok(Some(status)) => {
                // 405 / 501 → some servers don't implement HEAD;
                // fall through to GET. Same for the edge case of a
                // server that accepts HEAD but closes the
                // connection mid-response (treated as a transport
                // error higher up).
                if status.as_u16() == 405 || status.as_u16() == 501 {
                    match self.request(Method::GET, url).await {
                        Ok(Some(status)) => classify_status(status),
                        Ok(None) => CheckOutcome::Other,
                        Err(outcome) => outcome,
                    }
                } else {
                    classify_status(status)
                }
            }
            Ok(None) => CheckOutcome::Other,
            Err(outcome) => {
                if matches!(
                    outcome,
                    CheckOutcome::Other | CheckOutcome::ConnectionRefused
                ) {
                    // Some origins drop HEAD but accept GET —
                    // retry with GET for the non-specific errors.
                    match self.request(Method::GET, url).await {
                        Ok(Some(status)) => classify_status(status),
                        Ok(None) => outcome,
                        Err(get_outcome) => get_outcome,
                    }
                } else {
                    outcome
                }
            }
        }
    }

    async fn request(&self, method: Method, url: &str) -> Result<Option<StatusCode>, CheckOutcome> {
        match self.client.request(method, url).send().await {
            Ok(response) => Ok(Some(response.status())),
            Err(err) => Err(classify_error(&err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_status_covers_the_common_ranges() {
        assert_eq!(classify_status(StatusCode::OK), CheckOutcome::Ok);
        assert_eq!(classify_status(StatusCode::CREATED), CheckOutcome::Ok);
        assert_eq!(
            classify_status(StatusCode::FORBIDDEN),
            CheckOutcome::Forbidden
        );
        assert_eq!(
            classify_status(StatusCode::UNAUTHORIZED),
            CheckOutcome::Forbidden
        );
        assert_eq!(
            classify_status(StatusCode::NOT_FOUND),
            CheckOutcome::NotFound
        );
        assert_eq!(classify_status(StatusCode::GONE), CheckOutcome::NotFound);
        assert_eq!(
            classify_status(StatusCode::INTERNAL_SERVER_ERROR),
            CheckOutcome::ServerError,
        );
        assert_eq!(
            classify_status(StatusCode::BAD_GATEWAY),
            CheckOutcome::ServerError,
        );
        assert_eq!(
            classify_status(StatusCode::IM_A_TEAPOT),
            CheckOutcome::Other,
        );
    }

    #[test]
    fn wire_strings_are_kebab_case_and_stable() {
        // The client renders pill colours from these strings, so
        // accidental renames should surface loudly.
        assert_eq!(CheckOutcome::Ok.as_wire(), "ok");
        assert_eq!(CheckOutcome::NotFound.as_wire(), "not-found");
        assert_eq!(CheckOutcome::ServerError.as_wire(), "server-error");
        assert_eq!(CheckOutcome::Timeout.as_wire(), "timeout");
        assert_eq!(CheckOutcome::TlsError.as_wire(), "tls-error");
    }
}
