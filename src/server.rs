//! HTTP server: the local backend that serves the embedded web UI.
//!
//! The router is built separately from the network bind so handlers are
//! testable via `tower::ServiceExt::oneshot` without opening a socket.
//!
//! Implements: REQ005 (the serve command exposes GET /health as JSON)
//! Implements: REQ006 (embedded UI served locally with SPA fallback to index.html)

use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use std::net::SocketAddr;

/// The built frontend (`web/dist`), baked into the binary at compile time.
/// In debug builds rust-embed reads these files from disk at runtime; release
/// builds embed them so the executable is self-contained (REQ006).
#[derive(RustEmbed)]
#[folder = "web/dist"]
struct WebAssets;

/// Build the application router.
pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .fallback(static_asset)
}

/// Bind to the loopback interface on `port` and serve until the process stops.
///
/// Implements: REQ005 (local server)
pub async fn serve(port: u16) -> std::io::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("provreq serving on http://{addr}");
    axum::serve(listener, router()).await
}

/// GET /health — the health payload as JSON.
///
/// Implements: REQ005
async fn health() -> Response {
    (
        [(header::CONTENT_TYPE, "application/json")],
        crate::health_json(),
    )
        .into_response()
}

/// Serve an embedded asset by request path, falling back to `index.html` for
/// paths with no embedded file so the single-page app can route them.
///
/// Implements: REQ006
async fn static_asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match WebAssets::get(path) {
        Some(file) => embedded_response(path, file.data.into_owned()),
        // ponytail: any unmatched path returns index.html (standard SPA fallback);
        // fine until there is a real 404 surface to distinguish.
        None => match WebAssets::get("index.html") {
            Some(index) => embedded_response("index.html", index.data.into_owned()),
            None => (StatusCode::NOT_FOUND, "web UI not built").into_response(),
        },
    }
}

fn embedded_response(path: &str, bytes: Vec<u8>) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    ([(header::CONTENT_TYPE, mime.as_ref())], Body::from(bytes)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn get_path(path: &str) -> Response {
        router()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    fn content_type(res: &Response) -> String {
        res.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string()
    }

    // Verifies: REQ005
    #[tokio::test]
    async fn health_route_returns_ok_json() {
        let res = get_path("/health").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(content_type(&res), "application/json");
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("\"status\":\"ok\""), "body: {body}");
    }

    // Verifies: REQ006
    #[tokio::test]
    async fn root_serves_embedded_index_html() {
        let res = get_path("/").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(content_type(&res).starts_with("text/html"), "{res:?}");
    }

    // Verifies: REQ006 — an unknown client-side path falls back to index.html.
    #[tokio::test]
    async fn unknown_path_falls_back_to_index() {
        let res = get_path("/some/spa/route").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(content_type(&res).starts_with("text/html"), "{res:?}");
    }
}
