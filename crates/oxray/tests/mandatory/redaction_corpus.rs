//! Redactor positives and negatives on every surface.

#[path = "../common/mod.rs"]
mod support;

use axum::body::Body;
use oxray::{BodyEncoding, Capture, Header, Oxray};

#[test]
fn surfaces_keep_safe_text_and_mask_secrets() {
    let oxray = Oxray::builder().build().unwrap();
    let mut capture: Capture =
        serde_json::from_str(include_str!("../fixtures/capture_v1.json")).unwrap();
    capture.path = "/cards/4242424242424242".to_owned();
    capture.query = "username=ada&password=hunter2-synthetic".to_owned();
    capture.request.headers = vec![
        header("authorization", "Bearer synth-not-real"),
        header("x-request-id", "abc"),
    ];
    capture.request.body.content_type = Some("application/json".to_owned());
    capture.request.body.encoding = BodyEncoding::Utf8;
    capture.request.body.text = Some(
        r#"{"password":"hunter2-synthetic","pinning":"safe","pin":"1234","note":"4242 4242 4242 4242","clabe":"032180000118359719","short":"123456"}"#.to_owned(),
    );
    capture.request.body.truncated = false;
    capture.logs[0].message = "paid 4242424242424242".to_owned();
    capture.logs[0]
        .fields
        .insert("token".to_owned(), serde_json::json!("hunter2-synthetic"));
    capture.logs[0]
        .fields
        .insert("name".to_owned(), serde_json::json!("ada"));
    capture.queries[0].sql = "select 032180000118359719".to_owned();
    capture.outgoing[0].url = "http://127.0.0.1/?pan=4242424242424242".to_owned();
    oxray.redact(&mut capture);

    let stored = serde_json::to_string(&capture).unwrap();
    assert!(!stored.contains("hunter2-synthetic"));
    assert!(!stored.contains("4242424242424242"));
    assert!(!stored.contains("032180000118359719"));
    assert!(!stored.contains("Bearer synth-not-real"));
    assert!(stored.contains("ada"));
    assert!(stored.contains("safe"));
    assert!(stored.contains("123456"));
    assert!(stored.contains("abc"));
    assert!(capture.redactions.iter().any(|item| item.rule == "clabe"));
    assert!(
        capture
            .redactions
            .iter()
            .any(|item| item.rule == "sensitive_header")
    );
    assert!(
        capture
            .redactions
            .iter()
            .any(|item| item.rule == "sensitive_key")
    );
}

#[test]
fn truncated_json_and_edge_digits_are_masked() {
    let oxray = Oxray::builder().build().unwrap();
    let mut capture: Capture =
        serde_json::from_str(include_str!("../fixtures/capture_v1.json")).unwrap();
    capture.request.body.content_type = Some("application/json".to_owned());
    capture.request.body.text = Some(r#"{"password":"hunter2-synthetic","tail":99999"#.to_owned());
    capture.request.body.truncated = true;
    oxray.redact(&mut capture);
    let text = capture.request.body.text.unwrap();
    assert!(!text.contains("hunter2-synthetic"));
    assert!(!text.contains("99999"));
}

#[tokio::test]
async fn live_truncated_body_hides_the_cut_digits() {
    let oxray = Oxray::builder().body_limit(24).build().unwrap();
    let payload = r#"{"password":"hunter2-synthetic","n":"99999"}"#;
    assert!(payload.len() > 24);
    let _ = support::call(
        support::stack(oxray.clone(), true),
        support::request_with(
            "POST",
            "/text",
            "127.0.0.1",
            Some("application/json"),
            &[],
            Body::from(payload),
        ),
    )
    .await;
    let (_, _, body) = support::call(
        support::stack(oxray.clone(), true),
        support::request(
            "GET",
            "/_oxray/api/captures?limit=1",
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = page["items"][0]["id"].as_str().unwrap();
    let (_, _, detail) = support::call(
        support::stack(oxray, true),
        support::request(
            "GET",
            &format!("/_oxray/api/captures/{id}"),
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    let capture: serde_json::Value = serde_json::from_slice(&detail).unwrap();
    let text = capture["request"]["body"]["text"].as_str().unwrap_or("");
    assert!(!text.contains("hunter2-synthetic"));
    assert_eq!(capture["request"]["body"]["truncated"], true);
}

#[test]
fn numbers_errors_and_n_plus_one_are_masked() {
    let oxray = Oxray::builder().build().unwrap();
    let mut capture: Capture =
        serde_json::from_str(include_str!("../fixtures/capture_v1.json")).unwrap();
    capture.request.body.content_type = Some("application/json".to_owned());
    capture.request.body.encoding = BodyEncoding::Utf8;
    capture.request.body.truncated = false;
    capture.request.body.text = Some(
        r#"{"n":4242424242424242,"clabe":123456789012345673,"plain":123456,"nested":[4242424242424242]}"#
            .to_owned(),
    );
    capture.logs[0].fields.insert(
        "trace".to_owned(),
        serde_json::json!(4_242_424_242_424_242_u64),
    );
    capture.errors[0].message = "panic 4242424242424242".to_owned();
    capture.errors[0].backtrace[0].function = "fn 032180000118359719".to_owned();
    capture.n_plus_one[0].sql = "select 4242424242424242".to_owned();
    capture.n_plus_one[0].fingerprint = "select 032180000118359719".to_owned();
    oxray.redact(&mut capture);
    let stored = serde_json::to_string(&capture).unwrap();
    assert!(!stored.contains("4242424242424242"));
    assert!(!stored.contains("123456789012345673"));
    assert!(!stored.contains("032180000118359719"));
    assert!(stored.contains("123456"));
    let text = capture.request.body.text.unwrap();
    assert!(text.contains("•••• 4242"));
    assert!(
        capture
            .redactions
            .iter()
            .any(|item| item.path.starts_with("/errors/"))
    );
    assert!(
        capture
            .redactions
            .iter()
            .any(|item| item.path.starts_with("/n_plus_one/"))
    );
}

fn header(name: &str, value: &str) -> Header {
    oxray::Header {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}
