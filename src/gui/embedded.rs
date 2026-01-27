//! Embedded web asset service - serves bundled assets from memory

use axum::{
    body::Body,
    http::{Request, Response, StatusCode, header},
};
use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;
use tower::Service;

/// Embedded web bundle (compressed tar.zst archive)
#[cfg(feature = "gui")]
const EMBEDDED_BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/web-bundle.tar.zst"
));

/// Extracted assets (path -> content)
static ASSETS: OnceLock<HashMap<String, Vec<u8>>> = OnceLock::new();

/// Extract and decompress the embedded bundle
fn extract_assets() -> HashMap<String, Vec<u8>> {
    let mut assets = HashMap::new();

    // Decompress zstd
    let decompressed =
        zstd::decode_all(EMBEDDED_BUNDLE).expect("Failed to decompress embedded bundle");

    // Extract tar archive
    let mut archive = tar::Archive::new(&decompressed[..]);
    for entry in archive.entries().expect("Failed to read tar entries") {
        let mut entry = entry.expect("Failed to read tar entry");
        let path = entry.path().expect("Failed to read entry path");

        // Strip "web-bundle/" prefix and convert to String
        let path_str = path
            .strip_prefix("web-bundle/")
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .expect("Failed to read entry content");

        assets.insert(path_str, content);
    }

    assets
}

/// Get MIME type from file extension
fn mime_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}

/// Service that serves embedded assets
#[derive(Clone)]
pub struct EmbeddedAssetService;

impl EmbeddedAssetService {
    /// Create a new embedded asset service
    pub fn new() -> Self {
        // Trigger asset extraction on first creation
        ASSETS.get_or_init(extract_assets);
        Self
    }
}

impl Default for EmbeddedAssetService {
    fn default() -> Self {
        Self::new()
    }
}

impl Service<Request<Body>> for EmbeddedAssetService {
    type Response = Response<Body>;
    type Error = std::convert::Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let assets = ASSETS.get().expect("Assets not initialized");
        let path = req.uri().path();

        // Remove leading slash
        let path = path.trim_start_matches('/');

        // Default to index.html for root
        let path = if path.is_empty() { "index.html" } else { path };

        // Look up asset
        let response = if let Some(content) = assets.get(path) {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime_type(path))
                .body(Body::from(content.clone()))
                .unwrap()
        } else {
            // Try index.html as fallback for client-side routing
            if let Some(content) = assets.get("index.html") {
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                    .body(Body::from(content.clone()))
                    .unwrap()
            } else {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("404 Not Found"))
                    .unwrap()
            }
        };

        std::future::ready(Ok(response))
    }
}
