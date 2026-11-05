//! Embedded inspector shell.

use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

use crate::oxray::Oxray;

const CSP: &str = "default-src 'self'; base-uri 'self'; frame-ancestors 'none'";

#[derive(Embed)]
#[folder = "ui-dist"]
struct Assets;

pub(crate) async fn spa(State(oxray): State<Oxray>, request: axum::extract::Request) -> Response {
    // The trailing-slash route still carries the mount. A dotted mount such as
    // `/v1.0/oxray/` has to stay the shell, not a missing asset.
    let path = relative_path(&oxray.inner.mount, request.uri().path());
    if path.is_empty() || path == "index.html" || !path.contains('.') {
        return shell(&oxray.inner.mount);
    }
    match Assets::get(path) {
        Some(file) => asset(path, file.data.as_ref()),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

fn relative_path<'a>(mount: &str, uri_path: &'a str) -> &'a str {
    let path = uri_path.trim_start_matches('/');
    let mount = mount.trim_start_matches('/').trim_end_matches('/');
    if mount.is_empty() {
        return path;
    }
    if path == mount {
        return "";
    }
    match path.strip_prefix(mount) {
        Some(rest) if rest.starts_with('/') => rest.trim_start_matches('/'),
        _ => path,
    }
}

fn shell(mount: &str) -> Response {
    let Some(file) = Assets::get("index.html") else {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    };
    let html = String::from_utf8_lossy(file.data.as_ref());
    let href = escape_attr(&format!("{mount}/"));
    let content = escape_attr(mount);
    let tags = format!("<base href=\"{href}\"><meta name=\"oxray-mount\" content=\"{content}\">");
    let body = html.replacen("<head>", &format!("<head>{tags}"), 1);
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CONTENT_SECURITY_POLICY, CSP),
        ],
        body,
    )
        .into_response()
}

fn asset(path: &str, bytes: &[u8]) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type(path)),
            (header::CONTENT_SECURITY_POLICY, CSP),
        ],
        bytes.to_vec(),
    )
        .into_response()
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("woff2") => "font/woff2",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}
