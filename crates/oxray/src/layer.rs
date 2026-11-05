//! Tower layer that records requests outside the mount path.
//!
//! `Router::layer` runs after routing, so a matched route exposes
//! `MatchedPath`. A fallback has no template. A path that matches nothing
//! and has no fallback never reaches this layer.

use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Instant;

use axum::body::Body;
use axum::extract::MatchedPath;
use bytes::Bytes;
use futures_util::FutureExt;
use http::{HeaderMap, Request, Response};
use http_body::Body as HttpBody;
use http_body_util::BodyExt;
use pin_project_lite::pin_project;
use tower_layer::Layer;
use tower_service::Service;
use tracing::Instrument;

use crate::capture::{BodyCapture, BodyEncoding, Capture, Flags, Header, Message};
use crate::oxray::{Mode, Oxray};

/// Layer returned by [`Oxray::layer`](crate::Oxray::layer).
#[derive(Clone)]
pub struct OxrayLayer {
    pub(crate) oxray: Oxray,
}

/// Service produced by [`OxrayLayer`].
#[derive(Clone)]
pub struct OxrayService<S> {
    oxray: Oxray,
    inner: S,
}

impl<S> Layer<S> for OxrayLayer {
    type Service = OxrayService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        OxrayService {
            oxray: self.oxray.clone(),
            inner,
        }
    }
}

impl<S> Service<Request<Body>> for OxrayService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response<Body>, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let oxray = self.oxray.clone();
        let clone = self.inner.clone();
        let ready = std::mem::replace(&mut self.inner, clone);
        Box::pin(async move { serve(oxray, ready, req).await })
    }
}

async fn serve<S>(
    oxray: Oxray,
    mut inner: S,
    req: Request<Body>,
) -> Result<Response<Body>, S::Error>
where
    S: Service<Request<Body>, Response = Response<Body>> + Send,
    S::Error: Send,
{
    if oxray.mode() == Mode::Disabled || is_mount(&oxray, req.uri().path()) {
        return inner.call(req).await;
    }
    let id = uuid::Uuid::now_v7();
    let id_text = id.to_string();
    let short_id = id_text
        .chars()
        .rev()
        .take(8)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    let started = Instant::now();
    let started_at = timestamp();
    let method = req.method().to_string();
    let path = req.uri().path().to_owned();
    let query = req.uri().query().unwrap_or("").to_owned();
    let matched_path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|matched| matched.as_str().to_owned());
    let request_headers = copy_headers(req.headers());
    let content_type = header_value(req.headers(), "content-type");
    let content_length = header_value(req.headers(), "content-length")
        .and_then(|value| value.parse().ok())
        .or_else(|| req.body().size_hint().exact());
    let (parts, body) = req.into_parts();
    let request_buf = SharedBuf::new(oxray.inner.body_limit);
    let body = prepare_request_body(
        body,
        &content_type,
        content_length,
        oxray.inner.body_limit,
        &request_buf,
    )
    .await;
    let req = Request::from_parts(parts, body);
    let span = tracing::info_span!("oxray.request", id = %id_text);
    let called = std::panic::AssertUnwindSafe(inner.call(req).instrument(span))
        .catch_unwind()
        .await;
    match called {
        Ok(Ok(response)) => Ok(finish_response(
            oxray,
            response,
            Draft {
                id: id_text,
                short_id,
                started,
                started_at,
                method,
                path,
                query,
                matched_path,
                request_headers,
                request_buf,
                request_type: content_type,
                request_length: content_length,
            },
        )),
        Ok(Err(error)) => Err(error),
        Err(panic) => {
            store_draft(
                &oxray,
                &Draft {
                    id: id_text,
                    short_id,
                    started,
                    started_at,
                    method,
                    path,
                    query,
                    matched_path,
                    request_headers,
                    request_buf,
                    request_type: content_type,
                    request_length: content_length,
                },
                None,
                true,
            );
            std::panic::resume_unwind(panic);
        }
    }
}

struct Draft {
    id: String,
    short_id: String,
    started: Instant,
    started_at: String,
    method: String,
    path: String,
    query: String,
    matched_path: Option<String>,
    request_headers: Vec<Header>,
    request_buf: SharedBuf,
    request_type: Option<String>,
    request_length: Option<u64>,
}

fn finish_response(oxray: Oxray, response: Response<Body>, draft: Draft) -> Response<Body> {
    let (mut parts, body) = response.into_parts();
    if oxray.inner.response_header
        && let Ok(value) = http::HeaderValue::from_str(&draft.id)
    {
        parts.headers.insert("x-oxray-id", value);
    }
    let status = parts.status.as_u16();
    let response_headers = copy_headers(&parts.headers);
    let response_type = header_value(&parts.headers, "content-type");
    let response_length =
        header_value(&parts.headers, "content-length").and_then(|value| value.parse().ok());
    let response_encoding = header_value(&parts.headers, "content-encoding");
    let streaming = response_length.is_none() || is_stream(&response_type);
    let response_buf = SharedBuf::new(oxray.inner.body_limit);
    let body = if let Some(reason) =
        omit_reason_for(&response_type, response_encoding.as_deref(), parts.status)
    {
        response_buf.note(&reason);
        body
    } else {
        Body::new(Tee {
            inner: body,
            buf: response_buf.clone(),
        })
    };
    let slot = std::sync::Arc::new(FinishSlot {
        oxray,
        draft,
        status,
        response_headers,
        response_type,
        response_length,
        response_buf,
        streaming,
        done: AtomicBool::new(false),
    });
    let body = Body::new(FinishBody { inner: body, slot });
    Response::from_parts(parts, body)
}

struct FinishSlot {
    oxray: Oxray,
    draft: Draft,
    status: u16,
    response_headers: Vec<Header>,
    response_type: Option<String>,
    response_length: Option<u64>,
    response_buf: SharedBuf,
    streaming: bool,
    done: AtomicBool,
}

impl FinishSlot {
    fn finish(&self) {
        if self.done.swap(true, Ordering::Relaxed) {
            return;
        }
        let response = Message {
            headers: self.response_headers.clone(),
            body: body_from_buf(
                &self.response_buf,
                self.response_type.clone(),
                self.response_length,
            ),
        };
        store_draft(
            &self.oxray,
            &self.draft,
            Some((self.status, response, self.streaming)),
            false,
        );
    }
}

fn store_draft(
    oxray: &Oxray,
    draft: &Draft,
    response: Option<(u16, Message, bool)>,
    panicked: bool,
) {
    let request_body = body_from_buf(
        &draft.request_buf,
        draft.request_type.clone(),
        draft.request_length,
    );
    let truncated_response = response
        .as_ref()
        .is_some_and(|(_, message, _)| message.body.truncated);
    let (status, response_message, streaming) = match response {
        Some((status, message, streaming)) => (Some(status), Some(message), streaming),
        None => (None, None, false),
    };
    let duration_us = u64::try_from(draft.started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let has_error = panicked || status.is_some_and(|code| code >= 500);
    let capture = Capture {
        schema_version: 1,
        id: draft.id.clone(),
        short_id: draft.short_id.clone(),
        started_at: draft.started_at.clone(),
        duration_us,
        method: draft.method.clone(),
        path: draft.path.clone(),
        query: draft.query.clone(),
        matched_path: draft.matched_path.clone(),
        status,
        mode: "dev".to_owned(),
        request: Message {
            headers: draft.request_headers.clone(),
            body: request_body.clone(),
        },
        response: response_message,
        logs: Vec::new(),
        queries: Vec::new(),
        outgoing: Vec::new(),
        errors: Vec::new(),
        n_plus_one: Vec::new(),
        redactions: Vec::new(),
        flags: Flags {
            has_error,
            panicked,
            truncated_request: request_body.truncated,
            truncated_response,
            streaming,
            dropped_events: 0,
        },
    };
    oxray.store_capture(capture);
}

fn body_from_buf(
    buf: &SharedBuf,
    content_type: Option<String>,
    content_length: Option<u64>,
) -> BodyCapture {
    if buf.omitted.load(Ordering::Relaxed) {
        return BodyCapture {
            content_type,
            size_bytes: content_length,
            captured_bytes: 0,
            encoding: BodyEncoding::Omitted,
            text: None,
            truncated: false,
            omitted_reason: Some(
                buf.reason
                    .lock()
                    .ok()
                    .and_then(|g| g.clone())
                    .unwrap_or_else(|| "omitted".to_owned()),
            ),
        };
    }
    let data = buf.data.lock().map(|g| g.clone()).unwrap_or_default();
    let truncated = buf.truncated.load(Ordering::Relaxed);
    if data.is_empty()
        && content_length.is_some_and(|len| len > u64::try_from(buf.limit).unwrap_or(u64::MAX))
    {
        return BodyCapture {
            content_type,
            size_bytes: content_length,
            captured_bytes: 0,
            encoding: BodyEncoding::Omitted,
            text: None,
            truncated: false,
            omitted_reason: Some("not_read_by_app".to_owned()),
        };
    }
    if should_omit_type(content_type.as_deref()) {
        return BodyCapture {
            content_type: content_type.clone(),
            size_bytes: content_length.or_else(|| u64::try_from(data.len()).ok()),
            captured_bytes: u64::try_from(data.len()).unwrap_or(u64::MAX),
            encoding: BodyEncoding::Omitted,
            text: None,
            truncated,
            omitted_reason: Some(omit_reason(content_type.as_deref())),
        };
    }
    match String::from_utf8(data.clone()) {
        Ok(text) => BodyCapture {
            content_type,
            size_bytes: content_length.or_else(|| u64::try_from(data.len()).ok()),
            captured_bytes: u64::try_from(data.len()).unwrap_or(u64::MAX),
            encoding: BodyEncoding::Utf8,
            text: Some(text),
            truncated,
            omitted_reason: None,
        },
        Err(_) => BodyCapture {
            content_type,
            size_bytes: content_length,
            captured_bytes: u64::try_from(data.len()).unwrap_or(u64::MAX),
            encoding: BodyEncoding::Omitted,
            text: None,
            truncated,
            omitted_reason: Some("not_utf8".to_owned()),
        },
    }
}

fn should_omit_type(content_type: Option<&str>) -> bool {
    let content_type = lowered(content_type.unwrap_or(""));
    if content_type.starts_with("multipart/") || content_type.starts_with("text/event-stream") {
        return true;
    }
    if content_type.starts_with("text/")
        || content_type.contains("json")
        || content_type.contains("xml")
        || content_type.contains("javascript")
        || content_type.contains("x-www-form-urlencoded")
        || content_type.is_empty()
    {
        return false;
    }
    true
}

fn omit_reason(content_type: Option<&str>) -> String {
    let lowered = lowered(content_type.unwrap_or(""));
    if lowered.starts_with("text/event-stream") {
        return "event_stream".to_owned();
    }
    if lowered.starts_with("multipart/") {
        return "multipart".to_owned();
    }
    match content_type {
        Some(value) => format!("binary:{value}"),
        None => "binary".to_owned(),
    }
}

fn omit_reason_for(
    content_type: &Option<String>,
    encoding: Option<&str>,
    status: http::StatusCode,
) -> Option<String> {
    if status == http::StatusCode::SWITCHING_PROTOCOLS {
        return Some("upgrade".to_owned());
    }
    if let Some(encoding) = encoding
        && !encoding.eq_ignore_ascii_case("identity")
    {
        return Some(format!("compressed:{encoding}"));
    }
    let content_type = lowered(content_type.as_deref().unwrap_or(""));
    if content_type.starts_with("text/event-stream") {
        return Some("event_stream".to_owned());
    }
    if content_type.starts_with("multipart/") {
        return Some("multipart".to_owned());
    }
    None
}

fn is_stream(content_type: &Option<String>) -> bool {
    content_type
        .as_deref()
        .is_some_and(|value| lowered(value).starts_with("text/event-stream"))
}

fn is_multipart(content_type: &Option<String>) -> bool {
    content_type
        .as_deref()
        .is_some_and(|value| lowered(value).starts_with("multipart/"))
}

fn lowered(value: &str) -> String {
    value.to_ascii_lowercase()
}

async fn prepare_request_body(
    body: Body,
    content_type: &Option<String>,
    content_length: Option<u64>,
    limit: usize,
    buf: &SharedBuf,
) -> Body {
    let preread = content_length.is_some_and(|len| len <= u64::try_from(limit).unwrap_or(u64::MAX))
        && !is_stream(content_type)
        && !is_multipart(content_type);
    if !preread {
        return Body::new(Tee {
            inner: body,
            buf: buf.clone(),
        });
    }
    match body.collect().await {
        Ok(collected) => {
            let bytes = collected.to_bytes();
            buf.push(&bytes);
            Body::from(bytes)
        }
        Err(_) => {
            buf.note("read_error");
            Body::new(FailBody { pending: true })
        }
    }
}

fn is_mount(oxray: &Oxray, path: &str) -> bool {
    mount_matches(&oxray.inner.mount, path)
}

fn mount_matches(mount: &str, path: &str) -> bool {
    // A trailing slash would look for `{mount}/` with a double slash, so `/_oxray/api` would be captured.
    let mount = mount.trim_end_matches('/');
    if mount.is_empty() {
        return path == "/";
    }
    path == mount || path.starts_with(&format!("{mount}/"))
}

fn copy_headers(headers: &HeaderMap) -> Vec<Header> {
    headers
        .iter()
        .map(|(name, value)| Header {
            name: name.to_string(),
            value: value.to_str().unwrap_or("").to_owned(),
        })
        .collect()
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn timestamp() -> String {
    let now = time::OffsetDateTime::now_utc();
    let Ok(format) = time::format_description::parse_borrowed::<2>(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z",
    ) else {
        return now.unix_timestamp().to_string();
    };
    now.format(&format)
        .unwrap_or_else(|_| now.unix_timestamp().to_string())
}

#[derive(Clone)]
struct SharedBuf {
    data: std::sync::Arc<Mutex<Vec<u8>>>,
    truncated: std::sync::Arc<AtomicBool>,
    omitted: std::sync::Arc<AtomicBool>,
    reason: std::sync::Arc<Mutex<Option<String>>>,
    limit: usize,
}

impl SharedBuf {
    fn new(limit: usize) -> Self {
        Self {
            data: std::sync::Arc::new(Mutex::new(Vec::new())),
            truncated: std::sync::Arc::new(AtomicBool::new(false)),
            omitted: std::sync::Arc::new(AtomicBool::new(false)),
            reason: std::sync::Arc::new(Mutex::new(None)),
            limit,
        }
    }

    fn push(&self, chunk: &[u8]) {
        let mut data = match self.data.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let room = self.limit.saturating_sub(data.len());
        if chunk.len() > room {
            data.extend_from_slice(&chunk[..room]);
            self.truncated.store(true, Ordering::Relaxed);
        } else {
            data.extend_from_slice(chunk);
        }
    }

    fn note(&self, reason: &str) {
        self.omitted.store(true, Ordering::Relaxed);
        if let Ok(mut guard) = self.reason.lock() {
            *guard = Some(reason.to_owned());
        }
    }
}

pin_project! {
    struct Tee {
        #[pin]
        inner: Body,
        buf: SharedBuf,
    }
}

impl HttpBody for Tee {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        match this.inner.poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    this.buf.push(data);
                }
                Poll::Ready(Some(Ok(frame)))
            }
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

struct FailBody {
    pending: bool,
}

impl HttpBody for FailBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        if self.pending {
            self.pending = false;
            Poll::Ready(Some(Err(std::io::Error::other("oxray pre-read failed"))))
        } else {
            Poll::Ready(None)
        }
    }
}

struct FinishBody {
    inner: Body,
    slot: std::sync::Arc<FinishSlot>,
}

impl HttpBody for FinishBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_frame(cx) {
            Poll::Ready(None) => {
                this.slot.finish();
                Poll::Ready(None)
            }
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

impl Drop for FinishBody {
    fn drop(&mut self) {
        self.slot.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_multipart, is_stream, mount_matches, omit_reason, omit_reason_for, should_omit_type,
    };

    #[test]
    fn trailing_slash_still_marks_inspector_paths() {
        assert!(mount_matches("/_oxray/", "/_oxray"));
        assert!(mount_matches("/_oxray/", "/_oxray/api/captures"));
        assert!(!mount_matches("/_oxray/", "/_oxray-other"));
        assert!(!mount_matches("/_oxray/", "/only-get"));
        assert!(mount_matches("/_oxray", "/_oxray/api"));
        assert!(!mount_matches("/", "/only-get"));
    }

    #[test]
    fn content_type_checks_ignore_case() {
        assert!(!should_omit_type(Some("Application/JSON; charset=UTF-8")));
        assert!(should_omit_type(Some("Multipart/Form-Data; boundary=abc")));
        assert!(should_omit_type(Some("Text/Event-Stream")));
        assert_eq!(omit_reason(Some("Text/Event-Stream")), "event_stream");
        assert_eq!(omit_reason(Some("Multipart/Form-Data")), "multipart");
        assert!(is_stream(&Some("Text/Event-Stream".to_owned())));
        assert!(is_multipart(&Some(
            "Multipart/Form-Data; boundary=abc".to_owned()
        )));
        assert_eq!(
            omit_reason_for(
                &Some("Text/Event-Stream".to_owned()),
                None,
                http::StatusCode::OK,
            ),
            Some("event_stream".to_owned())
        );
    }
}
