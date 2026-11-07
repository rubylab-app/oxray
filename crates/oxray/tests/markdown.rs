//! Snapshot tests for the "Copy as Markdown" contract.

use oxray::Capture;
use serde_json::Value;

fn fixture() -> Result<Value, serde_json::Error> {
    serde_json::from_str(include_str!("fixtures/capture_v1.json"))
}

fn capture(value: &Value) -> Result<Capture, serde_json::Error> {
    serde_json::from_value(value.clone())
}

#[test]
fn fixture_as_is() {
    let capture = capture(&fixture().unwrap()).unwrap();
    insta::assert_snapshot!("fixture_as_is", capture.markdown());
}

#[test]
fn no_response() {
    let mut value = fixture().unwrap();
    value["status"] = Value::Null;
    value["response"] = Value::Null;
    value["flags"]["panicked"] = Value::Bool(true);
    let capture = capture(&value).unwrap();
    insta::assert_snapshot!("no_response", capture.markdown());
}

#[test]
fn no_route() {
    let mut value = fixture().unwrap();
    value["matched_path"] = Value::Null;
    let capture = capture(&value).unwrap();
    insta::assert_snapshot!("no_route", capture.markdown());
}

#[test]
fn truncated_request_and_binary_omitted_response() {
    let mut value = fixture().unwrap();
    let request_body = &mut value["request"]["body"];
    request_body["truncated"] = Value::Bool(true);
    request_body["captured_bytes"] = serde_json::json!(1331);
    request_body["size_bytes"] = serde_json::json!(65_536);
    let response_body = &mut value["response"]["body"];
    response_body["encoding"] = serde_json::json!("omitted");
    response_body["text"] = Value::Null;
    response_body["truncated"] = Value::Bool(false);
    response_body["omitted_reason"] = serde_json::json!("binary:image/png");
    response_body["captured_bytes"] = serde_json::json!(0);
    response_body["size_bytes"] = serde_json::json!(482);
    let capture = capture(&value).unwrap();
    insta::assert_snapshot!(
        "truncated_request_and_binary_omitted_response",
        capture.markdown()
    );
}

#[test]
fn failed_outgoing_call() {
    let mut value = fixture().unwrap();
    let call = &mut value["outgoing"][0];
    call["status"] = Value::Null;
    call["response"] = Value::Null;
    call["error"] = serde_json::json!("connection refused");
    let capture = capture(&value).unwrap();
    insta::assert_snapshot!("failed_outgoing_call", capture.markdown());
}

#[test]
fn fifty_five_logs() {
    let mut value = fixture().unwrap();
    let logs: Vec<Value> = (0..55_u64)
        .map(|index| {
            serde_json::json!({
                "offset_us": index,
                "level": "INFO",
                "target": "app",
                "message": format!("line {index}"),
                "fields": {},
            })
        })
        .collect();
    value["logs"] = Value::Array(logs);
    let capture = capture(&value).unwrap();
    insta::assert_snapshot!("fifty_five_logs", capture.markdown());
}

#[test]
fn repeated_queries_with_n_plus_one() {
    let mut value = fixture().unwrap();
    let base = value["queries"][0].clone();
    let mut queries = Vec::new();
    for index in 0..3_u64 {
        let mut query = base.clone();
        query["sql"] = serde_json::json!(format!("select {}", index + 1));
        query["duration_us"] = serde_json::json!(30 + index);
        queries.push(query);
    }
    let mut other = base.clone();
    other["sql"] = serde_json::json!("update t set a = 1");
    other["fingerprint"] = serde_json::json!("update ?");
    other["rows_returned"] = Value::Null;
    other["rows_affected"] = serde_json::json!(2);
    queries.push(other);
    value["queries"] = Value::Array(queries);
    let capture = capture(&value).unwrap();
    insta::assert_snapshot!("repeated_queries_with_n_plus_one", capture.markdown());
}

#[test]
fn sql_with_backtick() {
    let mut value = fixture().unwrap();
    value["queries"][0]["sql"] = serde_json::json!("select `a` from t");
    let capture = capture(&value).unwrap();
    insta::assert_snapshot!("sql_with_backtick", capture.markdown());
}

#[test]
fn body_with_backtick_run() {
    let mut value = fixture().unwrap();
    let text = "intro\n```\noutro";
    let body = &mut value["request"]["body"];
    body["content_type"] = serde_json::json!("text/plain");
    body["text"] = serde_json::json!(text);
    body["size_bytes"] = serde_json::json!(text.len());
    body["captured_bytes"] = serde_json::json!(text.len());
    let capture = capture(&value).unwrap();
    let markdown = capture.markdown();
    assert!(
        markdown.contains("````text\nintro\n```\noutro\n````"),
        "{markdown}"
    );
    insta::assert_snapshot!("body_with_backtick_run", markdown);
}
