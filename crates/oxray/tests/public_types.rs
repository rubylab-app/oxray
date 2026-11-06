//! A user who reads a capture can name every type inside it.

use oxray::{
    BacktraceFrame, BodyCapture, Capture, CaptureSummary, ErrorEntry, ErrorKind, Flags, LogEntry,
    Message, NPlusOne, OutgoingCall, QueryEntry, Redaction, SummaryCounts, SummaryFlags,
};

const FIXTURE: &str = include_str!("fixtures/capture_v1.json");

#[test]
fn capture_parts_have_public_names() {
    let capture: Capture = serde_json::from_str(FIXTURE).unwrap();
    let request: &Message = &capture.request;
    let body: &BodyCapture = &request.body;
    let log: &LogEntry = &capture.logs[0];
    let query: &QueryEntry = &capture.queries[0];
    let call: &OutgoingCall = &capture.outgoing[0];
    let error: &ErrorEntry = &capture.errors[0];
    let frame: &BacktraceFrame = &error.backtrace[0];
    let repeated: &NPlusOne = &capture.n_plus_one[0];
    let redaction: &Redaction = &capture.redactions[0];
    let flags: &Flags = &capture.flags;

    assert!(body.captured_bytes <= body.size_bytes.unwrap_or(u64::MAX));
    assert!(!log.message.is_empty());
    assert!(!query.sql.is_empty());
    assert!(!call.url.is_empty());
    assert_eq!(error.kind, ErrorKind::Panic);
    assert!(frame.user);
    assert!(repeated.count > 1);
    assert!(!redaction.rule.is_empty());
    assert_eq!(
        flags.panicked,
        capture.errors.iter().any(|e| e.kind == ErrorKind::Panic)
    );
}

#[test]
fn summary_parts_have_public_names() {
    let capture: Capture = serde_json::from_str(FIXTURE).unwrap();
    let summary: CaptureSummary = capture.summary();
    let counts: &SummaryCounts = &summary.counts;
    let flags: &SummaryFlags = &summary.flags;

    assert_eq!(counts.queries, capture.queries.len() as u64);
    assert_eq!(counts.errors, capture.errors.len() as u64);
    assert_eq!(flags.panicked, capture.flags.panicked);
}
