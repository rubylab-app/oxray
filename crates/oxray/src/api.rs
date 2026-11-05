//! HTTP API mounted under the Oxray path.

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::CapturePage;
use crate::guard;
use crate::oxray::{Mode, Oxray};

#[derive(Debug, Deserialize)]
struct ListQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

/// Router for this handle.
pub fn router(oxray: Oxray) -> Router {
    let mount = oxray.inner.mount.clone();
    // A top-level fallback panics when the app router already has one.
    let inner = if oxray.mode() == Mode::Disabled {
        Router::new().fallback(|| async { StatusCode::NOT_FOUND })
    } else {
        Router::new()
            .route("/api/captures", get(list))
            .route("/api/captures/{id}", get(detail))
            .route("/api/meta", get(meta))
            .fallback(|| async { StatusCode::NOT_FOUND })
            .layer(axum::middleware::from_fn_with_state(
                oxray.clone(),
                guard_layer,
            ))
            .with_state(oxray)
    };
    Router::new().nest(&mount, inner)
}

async fn guard_layer(
    State(oxray): State<Oxray>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if let Some(reason) = guard::reject_dev(&request) {
        return (
            StatusCode::FORBIDDEN,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            reason,
        )
            .into_response();
    }
    let _ = oxray;
    next.run(request).await
}

async fn list(State(oxray): State<Oxray>, Query(query): Query<ListQuery>) -> Json<CapturePage> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let (captures, next_cursor) = oxray.inner.store.page(query.cursor.as_deref(), limit);
    Json(CapturePage {
        items: captures.iter().map(|capture| capture.summary()).collect(),
        next_cursor,
    })
}

async fn detail(State(oxray): State<Oxray>, Path(id): Path<String>) -> Response {
    match oxray.inner.store.get(&id) {
        Some(capture) => Json(capture).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!("Capture {id} is not in the store"),
        )
            .into_response(),
    }
}

async fn meta(State(oxray): State<Oxray>) -> Json<Meta> {
    let usage = oxray.inner.store.usage();
    Json(Meta {
        version: env!("CARGO_PKG_VERSION"),
        mode: match oxray.mode() {
            Mode::Dev => "dev",
            Mode::Disabled => "disabled",
        },
        store: StoreMeta {
            captures: usage.captures,
            bytes: usage.bytes,
            max_captures: usage.max_captures,
            max_bytes: usage.max_bytes,
        },
        redaction_rules: oxray.inner.redactor.rule_names(),
        persistence: None,
    })
}

#[derive(serde::Serialize)]
struct Meta {
    version: &'static str,
    mode: &'static str,
    store: StoreMeta,
    redaction_rules: &'static [&'static str],
    persistence: Option<String>,
}

#[derive(serde::Serialize)]
struct StoreMeta {
    captures: usize,
    bytes: usize,
    max_captures: usize,
    max_bytes: usize,
}
