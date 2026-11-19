use std::borrow::Cow;

use axum::{
    body::Body,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

/// The PWA build is compiled into the daemon so a single binary can serve both
/// the API and the app. `ONI_PWA_DIR` overrides this with a disk directory for
/// development.
#[derive(RustEmbed)]
#[folder = "../clients/pwa/dist"]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let requested = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = Assets::get(requested) {
        return file_response(requested, file.data);
    }

    if requested == "favicon.ico" {
        if let Some(file) = Assets::get("favicon.svg") {
            return file_response("favicon.svg", file.data);
        }
    }

    // Client-side routes have no file extension: return the app shell so the
    // SPA can resolve the deep link.
    if !requested.contains('.') {
        if let Some(index) = Assets::get("index.html") {
            return file_response("index.html", index.data);
        }
    }

    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn file_response(path: &str, data: Cow<'static, [u8]>) -> Response {
    let mime = mime_guess::from_path(path)
        .first_raw()
        .unwrap_or("application/octet-stream");

    let cache_control =
        if path == "index.html" || path == "sw.js" || path.ends_with("manifest.webmanifest") {
            "no-cache"
        } else if path.starts_with("assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "public, max-age=3600"
        };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, cache_control)
        .body(Body::from(data))
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "response error").into_response())
}
