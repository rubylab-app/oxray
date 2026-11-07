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
use crate::ui;

#[derive(Debug, Deserialize)]
struct ListQuery {
    cursor: Option<String>,
    limit: Option<usize>,
    failed: Option<bool>,
    status: Option<String>,
    after: Option<String>,
}

/// How the `status` filter matches a capture.
#[derive(Clone, Copy, Debug)]
enum StatusFilter {
    /// An exact status code.
    Code(u16),
    /// A status class digit (`1`..=`5`), matched as `status / 100`.
    Class(u16),
}

/// Parses `status` as an exact code (`500`) or a class (`5xx`).
fn parse_status(raw: &str) -> Option<StatusFilter> {
    if let Some(digit) = raw.strip_suffix("xx") {
        let byte = digit.as_bytes();
        if byte.len() == 1 && matches!(byte[0], b'1'..=b'5') {
            return Some(StatusFilter::Class(u16::from(byte[0] - b'0')));
        }
        return None;
    }
    raw.parse::<u16>().ok().map(StatusFilter::Code)
}

/// Router for this handle.
pub fn router(oxray: Oxray) -> Router {
    let mount = oxray.inner.mount.clone();
    let slash = format!("{mount}/");
    // A top-level fallback panics when the app router already has one.
    let inner = if oxray.mode() == Mode::Disabled {
        Router::new().fallback(|| async { StatusCode::NOT_FOUND })
    } else {
        Router::new()
            .route("/api/captures", get(list))
            .route("/api/captures/{id}", get(detail))
            .route("/api/captures/{id}/markdown", get(markdown))
            .route("/api/meta", get(meta))
            .fallback(ui::spa)
            .layer(axum::middleware::from_fn_with_state(
                oxray.clone(),
                guard_layer,
            ))
            .with_state(oxray.clone())
    };
    // `nest` serves `/mount` but not `/mount/`.
    if oxray.mode() == Mode::Disabled {
        Router::new()
            .nest(&mount, inner)
            .route(&slash, get(|| async { StatusCode::NOT_FOUND }))
    } else {
        let slash_router = Router::new()
            .route(&slash, get(ui::spa))
            .layer(axum::middleware::from_fn_with_state(
                oxray.clone(),
                guard_layer,
            ))
            .with_state(oxray);
        Router::new().nest(&mount, inner).merge(slash_router)
    }
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

async fn list(State(oxray): State<Oxray>, Query(query): Query<ListQuery>) -> Response {
    let status_filter = match query.status.as_deref().map(parse_status) {
        Some(Some(filter)) => Some(filter),
        Some(None) => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "status must be a code like 500 or a class like 5xx",
            )
                .into_response();
        }
        None => None,
    };
    let failed = query.failed;
    let after = query.after.clone();
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let (captures, next_cursor) = oxray.inner.store.page(
        query.cursor.as_deref(),
        limit,
        |capture: &crate::Capture| {
            if let Some(failed) = failed {
                let is_failed = capture.flags.has_error || capture.status.is_none();
                if is_failed != failed {
                    return false;
                }
            }
            if let Some(filter) = status_filter {
                let matches = match filter {
                    StatusFilter::Code(code) => capture.status == Some(code),
                    StatusFilter::Class(digit) => {
                        matches!(capture.status, Some(status) if status / 100 == digit)
                    }
                };
                if !matches {
                    return false;
                }
            }
            if let Some(after) = &after
                && capture.id.as_str() <= after.as_str()
            {
                return false;
            }
            true
        },
    );
    Json(CapturePage {
        items: captures.iter().map(|capture| capture.summary()).collect(),
        next_cursor,
    })
    .into_response()
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

async fn markdown(State(oxray): State<Oxray>, Path(id): Path<String>) -> Response {
    match oxray.inner.store.get(&id) {
        Some(capture) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            capture.markdown(),
        )
            .into_response(),
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
