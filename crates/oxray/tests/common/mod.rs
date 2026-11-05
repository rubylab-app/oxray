use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::{ConnectInfo, DefaultBodyLimit};
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use bytes::Bytes;
use http::header;
use oxray::Oxray;
use serde_json::Value;
use tower::ServiceExt;

pub fn routes_only() -> Router {
    routes().fallback(|| async { StatusCode::NOT_FOUND })
}

pub fn stack(oxray: Oxray, peer: bool) -> Router {
    keep_helpers();
    let app = routes()
        .merge(oxray.router())
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(oxray.layer());
    if peer {
        app.layer(axum::middleware::from_fn(insert_peer))
    } else {
        app
    }
}

fn routes() -> Router {
    Router::new()
        .route("/echo", post(echo))
        .route("/len", post(len))
        .route("/stream", get(stream))
        .route("/events", get(events))
        .route("/only-get", get(|| async { "get" }))
        .route("/boom", get(boom))
        .route("/fail", get(|| async { StatusCode::INTERNAL_SERVER_ERROR }))
        .route("/json", post(echo))
        .route("/text", post(text))
        .route("/sink", post(sink))
        .layer(DefaultBodyLimit::disable())
}

async fn echo(Json(value): Json<Value>) -> Json<Value> {
    Json(value)
}

async fn len(body: Bytes) -> String {
    body.len().to_string()
}

async fn text(body: String) -> String {
    body
}

async fn sink(_body: Bytes) -> &'static str {
    "ok"
}

#[expect(clippy::panic, reason = "the handler under test has to panic")]
async fn boom() {
    panic!("boom-marker");
}

async fn stream() -> Body {
    let stream = futures_util::stream::unfold(0, |step| async move {
        match step {
            0 => Some((Ok::<Bytes, std::io::Error>(Bytes::from("a")), 1)),
            1 => {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                Some((Ok(Bytes::from("b")), 2))
            }
            _ => None,
        }
    });
    Body::from_stream(stream)
}

async fn events() -> impl axum::response::IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/event-stream")],
        "data: hi\n\n",
    )
}

async fn insert_peer(mut request: Request<Body>, next: Next) -> Response {
    if request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_none()
    {
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 9))));
    }
    next.run(request).await
}

pub fn request(method: &str, path: &str, host: &str, body: Body) -> Request<Body> {
    request_with(method, path, host, None, &[], body)
}

pub fn request_with(
    method: &str,
    path: &str,
    host: &str,
    content_type: Option<&str>,
    extra: &[(&str, &str)],
    body: Body,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("host", host);
    if let Some(content_type) = content_type {
        builder = builder.header("content-type", content_type);
    }
    for (name, value) in extra {
        builder = builder.header(*name, *value);
    }
    match builder.body(body) {
        Ok(request) => request,
        Err(_) => Request::new(Body::empty()),
    }
}

pub async fn call(
    app: Router,
    request: Request<Body>,
) -> (StatusCode, Vec<(String, String)>, Bytes) {
    let response = match app.oneshot(request).await {
        Ok(response) => response,
        Err(error) => match error {},
    };
    let status = response.status();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                value.to_str().unwrap_or("").to_owned(),
            )
        })
        .collect();
    let Ok(bytes) = axum::body::to_bytes(response.into_body(), usize::MAX).await else {
        return (status, headers, Bytes::new());
    };
    (status, headers, bytes)
}

fn keep_helpers() {
    let _pending = call(
        routes_only(),
        request("GET", "/", "127.0.0.1", Body::empty()),
    );
    let _ = request_with("GET", "/", "127.0.0.1", None, &[], Body::empty());
    let _ = without_id(&[]);
}

pub fn without_id(headers: &[(String, String)]) -> Vec<(String, String)> {
    headers
        .iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case("x-oxray-id"))
        .cloned()
        .collect()
}
