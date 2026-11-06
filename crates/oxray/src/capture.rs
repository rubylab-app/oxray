//! Schema v1 capture model.

use serde::{Deserialize, Serialize};

/// One captured request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Capture {
    /// Schema version. Always `1` for this model.
    pub schema_version: u32,
    /// UUID v7.
    pub id: String,
    /// Last eight hex digits of `id`.
    pub short_id: String,
    /// UTC timestamp with millisecond precision.
    pub started_at: String,
    /// Elapsed time in microseconds.
    pub duration_us: u64,
    /// HTTP method.
    pub method: String,
    /// Concrete request path.
    pub path: String,
    /// Redacted query string without `?`.
    pub query: String,
    /// Router template, when Axum matched one.
    pub matched_path: Option<String>,
    /// Response status. Absent when the handler panicked first.
    pub status: Option<u16>,
    /// `dev` or `disabled`.
    pub mode: String,
    /// Request headers and body.
    pub request: Message,
    /// Response headers and body.
    pub response: Option<Message>,
    /// Tracing events. Empty until the tracing layer records them.
    pub logs: Vec<LogEntry>,
    /// SQL events. Empty until the tracing layer records them.
    pub queries: Vec<QueryEntry>,
    /// Outgoing HTTP calls. Empty until that middleware records them.
    pub outgoing: Vec<OutgoingCall>,
    /// Panics and error events. Empty until the panic hook records them.
    pub errors: Vec<ErrorEntry>,
    /// Repeated-query groups. Empty until grouping is recorded.
    pub n_plus_one: Vec<NPlusOne>,
    /// Redactions applied before the capture was stored.
    pub redactions: Vec<Redaction>,
    /// Summary flags.
    pub flags: Flags,
}

/// Headers plus body for one direction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Header pairs in encounter order.
    pub headers: Vec<Header>,
    /// Body bytes that were kept.
    pub body: BodyCapture,
}

/// A single header.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Header {
    /// Header name.
    pub name: String,
    /// Header value.
    pub value: String,
}

/// Captured body bytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BodyCapture {
    /// Content-Type header, when present.
    pub content_type: Option<String>,
    /// Full size when known.
    pub size_bytes: Option<u64>,
    /// Bytes copied into the capture before redaction.
    pub captured_bytes: u64,
    /// How `text` should be read.
    pub encoding: BodyEncoding,
    /// Redacted text, when the body was stored.
    pub text: Option<String>,
    /// The copy hit `body_limit`.
    pub truncated: bool,
    /// Why the body was not stored as text.
    pub omitted_reason: Option<String>,
}

/// Whether body text was kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyEncoding {
    /// UTF-8 text.
    Utf8,
    /// Text was not stored.
    Omitted,
}

/// A tracing event attached to the request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    /// Microseconds after the request started.
    pub offset_us: u64,
    /// Level name.
    pub level: String,
    /// Tracing target.
    pub target: String,
    /// Rendered message.
    pub message: String,
    /// Structured fields.
    pub fields: serde_json::Map<String, serde_json::Value>,
}

/// A SQL event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryEntry {
    /// Microseconds after the request started.
    pub offset_us: u64,
    /// Query duration in microseconds.
    pub duration_us: u64,
    /// SQL text.
    pub sql: String,
    /// Normalized SQL fingerprint.
    pub fingerprint: String,
    /// Rows changed, when reported.
    pub rows_affected: Option<u64>,
    /// Rows returned, when reported.
    pub rows_returned: Option<u64>,
}

/// An outgoing HTTP call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutgoingCall {
    /// Microseconds after the request started.
    pub offset_us: u64,
    /// Call duration in microseconds.
    pub duration_us: u64,
    /// HTTP method.
    pub method: String,
    /// Redacted URL.
    pub url: String,
    /// Response status.
    pub status: Option<u16>,
    /// Outgoing request.
    pub request: Message,
    /// Outgoing response.
    pub response: Option<Message>,
    /// Transport error.
    pub error: Option<String>,
}

/// A panic or error event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorEntry {
    /// Microseconds after the request started.
    pub offset_us: u64,
    /// Panic or tracing error.
    pub kind: ErrorKind,
    /// Error text.
    pub message: String,
    /// Tracing target.
    pub target: Option<String>,
    /// Source location.
    pub location: Option<String>,
    /// Parsed frames.
    pub backtrace: Vec<BacktraceFrame>,
}

/// Kind of error entry. New kinds can arrive in a minor release, so a
/// `match` needs a `_` arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErrorKind {
    /// A panicked handler.
    Panic,
    /// A tracing ERROR event.
    ErrorEvent,
}

/// One backtrace frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BacktraceFrame {
    /// Function name.
    pub function: String,
    /// Source file.
    pub file: Option<String>,
    /// Source line.
    pub line: Option<u32>,
    /// Frame belongs to the application.
    pub user: bool,
}

/// A repeated SQL fingerprint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NPlusOne {
    /// Normalized SQL fingerprint.
    pub fingerprint: String,
    /// How many times it ran.
    pub count: u64,
    /// Representative SQL.
    pub sql: String,
}

/// One redaction applied to the capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Redaction {
    /// JSON Pointer prefixed with the surface.
    pub path: String,
    /// Rule name.
    pub rule: String,
}

/// Flags stored on the capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flags {
    /// Panic, error event, or status >= 500.
    pub has_error: bool,
    /// The handler panicked.
    pub panicked: bool,
    /// Request body hit the limit.
    pub truncated_request: bool,
    /// Response body hit the limit.
    pub truncated_response: bool,
    /// Response was streamed.
    pub streaming: bool,
    /// Events dropped by a per-capture cap.
    pub dropped_events: u64,
}

/// List-row summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureSummary {
    /// UUID v7.
    pub id: String,
    /// Last eight hex digits.
    pub short_id: String,
    /// UTC timestamp.
    pub started_at: String,
    /// Elapsed microseconds.
    pub duration_us: u64,
    /// HTTP method.
    pub method: String,
    /// Concrete path.
    pub path: String,
    /// Router template.
    pub matched_path: Option<String>,
    /// Response status.
    pub status: Option<u16>,
    /// Child counts.
    pub counts: SummaryCounts,
    /// Row flags.
    pub flags: SummaryFlags,
}

/// Counts shown on a list row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SummaryCounts {
    /// SQL events.
    pub queries: u64,
    /// Outgoing calls.
    pub outgoing: u64,
    /// Log events.
    pub logs: u64,
    /// Error entries.
    pub errors: u64,
    /// Redactions.
    pub redactions: u64,
}

/// Flags shown on a list row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SummaryFlags {
    /// The capture failed.
    pub has_error: bool,
    /// An N+1 group was recorded.
    pub n_plus_one: bool,
    /// A body was truncated.
    pub truncated: bool,
    /// The handler panicked.
    pub panicked: bool,
}

/// One page of summaries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CapturePage {
    /// Rows, newest first.
    pub items: Vec<CaptureSummary>,
    /// Id to pass as `cursor` for the next page.
    pub next_cursor: Option<String>,
}

impl Capture {
    /// Builds the list row for this capture.
    pub fn summary(&self) -> CaptureSummary {
        CaptureSummary {
            id: self.id.clone(),
            short_id: self.short_id.clone(),
            started_at: self.started_at.clone(),
            duration_us: self.duration_us,
            method: self.method.clone(),
            path: self.path.clone(),
            matched_path: self.matched_path.clone(),
            status: self.status,
            counts: SummaryCounts {
                queries: u64_len(&self.queries),
                outgoing: u64_len(&self.outgoing),
                logs: u64_len(&self.logs),
                errors: u64_len(&self.errors),
                redactions: u64_len(&self.redactions),
            },
            flags: SummaryFlags {
                has_error: self.flags.has_error,
                n_plus_one: !self.n_plus_one.is_empty(),
                truncated: self.flags.truncated_request || self.flags.truncated_response,
                panicked: self.flags.panicked,
            },
        }
    }
}

fn u64_len<T>(items: &[T]) -> u64 {
    u64::try_from(items.len()).unwrap_or(u64::MAX)
}
