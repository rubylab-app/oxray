//! The fixture is the capture schema contract.

use oxray::Capture;

#[test]
fn capture_v1_round_trips() {
    let raw = include_str!("fixtures/capture_v1.json");
    let capture: Capture = serde_json::from_str(raw).unwrap();
    assert_eq!(capture.schema_version, 1);
    assert!(!capture.logs.is_empty());
    assert!(!capture.queries.is_empty());
    assert!(!capture.outgoing.is_empty());
    assert!(!capture.errors.is_empty());
    assert!(!capture.n_plus_one.is_empty());
    assert!(!capture.redactions.is_empty());
    assert!(capture.errors[0].backtrace[0].user);
    let encoded = serde_json::to_string(&capture).unwrap();
    let again: Capture = serde_json::from_str(&encoded).unwrap();
    assert_eq!(capture, again);
}
