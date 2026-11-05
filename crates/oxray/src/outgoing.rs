//! Reqwest middleware that records outgoing HTTP calls.

use std::sync::Arc;
use std::time::Instant;

use http::HeaderMap;
use http_body::Body as HttpBody;
use pin_project_lite::pin_project;
use reqwest::ResponseBuilderExt;

use crate::capture::Header;
use crate::collect::OutgoingState;
use crate::oxray::Oxray;

/// Middleware that tees an outgoing response into the active capture.
#[derive(Clone)]
pub struct OxrayMiddleware {
    oxray: Oxray,
}

impl OxrayMiddleware {
    pub(crate) fn new(oxray: Oxray) -> Self {
        Self { oxray }
    }
}

#[async_trait::async_trait]
impl reqwest_middleware::Middleware for OxrayMiddleware {
    async fn handle(
        &self,
        req: reqwest::Request,
        extensions: &mut http::Extensions,
        next: reqwest_middleware::Next<'_>,
    ) -> reqwest_middleware::Result<reqwest::Response> {
        let started = Instant::now();
        let state = Arc::new(std::sync::Mutex::new(OutgoingState {
            method: req.method().to_string(),
            url: req.url().to_string(),
            offset_us: 0,
            started,
            status: None,
            request_headers: header_list(req.headers()),
            request_type: req
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            request_body: req
                .body()
                .and_then(reqwest::Body::as_bytes)
                .map(|bytes| String::from_utf8_lossy(bytes).into_owned()),
            response_headers: Vec::new(),
            response_type: None,
            response_body: Vec::new(),
            response_truncated: false,
            error: None,
            limit: self.oxray.body_limit(),
            finished_us: None,
        }));
        let key = self.oxray.park_outgoing(Arc::clone(&state));
        tracing::trace!(target: "oxray::outgoing", key, "outgoing");
        match next.run(req, extensions).await {
            Ok(response) => {
                if let Ok(mut guard) = state.lock() {
                    guard.finished_us = Some(elapsed_us(started));
                }
                Ok(rebuild(response, state))
            }
            Err(error) => {
                if let Ok(mut guard) = state.lock() {
                    guard.finished_us = Some(elapsed_us(started));
                    guard.error = Some(error.to_string());
                }
                Err(error)
            }
        }
    }
}

fn rebuild(
    response: reqwest::Response,
    state: Arc<std::sync::Mutex<OutgoingState>>,
) -> reqwest::Response {
    let url = response.url().clone();
    if let Ok(mut guard) = state.lock() {
        guard.status = Some(response.status().as_u16());
        guard.response_headers = header_list(response.headers());
        guard.response_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
    }
    let http_response = http::Response::from(response);
    let (parts, body) = http_response.into_parts();
    let teed = reqwest::Body::wrap(Tee { inner: body, state });
    let built = http::Response::builder()
        .status(parts.status)
        .version(parts.version)
        .url(url)
        .body(teed);
    let Ok(mut built) = built else {
        return reqwest::Response::from(http::Response::new(reqwest::Body::from(String::new())));
    };
    *built.headers_mut() = parts.headers;
    built.extensions_mut().extend(parts.extensions);
    reqwest::Response::from(built)
}

fn elapsed_us(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn header_list(headers: &HeaderMap) -> Vec<Header> {
    headers
        .iter()
        .map(|(name, value)| Header {
            name: name.to_string(),
            value: value.to_str().unwrap_or("").to_owned(),
        })
        .collect()
}

pin_project! {
    struct Tee {
        #[pin]
        inner: reqwest::Body,
        state: Arc<std::sync::Mutex<OutgoingState>>,
    }
}

impl HttpBody for Tee {
    type Data = bytes::Bytes;
    type Error = reqwest::Error;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        match this.inner.poll_frame(cx) {
            std::task::Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref()
                    && let Ok(mut guard) = this.state.lock()
                {
                    guard.push_response(data);
                }
                std::task::Poll::Ready(Some(Ok(frame)))
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
