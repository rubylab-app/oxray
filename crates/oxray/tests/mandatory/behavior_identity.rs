//! The app responds the same with Oxray installed.

#[path = "../common/mod.rs"]
mod support;

use std::time::Duration;

use axum::body::Body;
use http_body_util::BodyExt;
use oxray::{Mode, Oxray};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn responses_match_and_the_capture_records_the_exchange() {
    let oxray = Oxray::builder().body_limit(64 * 1024).build().unwrap();
    assert_eq!(oxray.mode(), Mode::Dev);
    use tracing_subscriber::prelude::*;
    let _ = tracing_subscriber::registry()
        .with(oxray.tracing_layer())
        .try_init();

    let payload = r#"{"hello":"world"}"#;
    let (plain_status, plain_headers, plain_body) =
        support::call(support::routes_only(), json_echo(payload)).await;
    let (status, headers, body) =
        support::call(support::stack(oxray.clone(), true), json_echo(payload)).await;
    assert_eq!(status, plain_status);
    assert_eq!(body, plain_body);
    assert_eq!(
        support::without_id(&headers),
        support::without_id(&plain_headers)
    );
    assert!(
        headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("x-oxray-id"))
    );

    let capture = only_capture(&oxray).await.unwrap();
    assert_eq!(capture["matched_path"], "/echo");
    assert_eq!(capture["request"]["body"]["text"], payload);
    assert_eq!(capture["status"], 200);

    let big = vec![b'z'; 5 * 1024 * 1024];
    let (plain_status, _, plain_body) = support::call(
        support::routes_only(),
        support::request("POST", "/len", "127.0.0.1", Body::from(big.clone())),
    )
    .await;
    let (status, _, body) = support::call(
        support::stack(oxray.clone(), true),
        support::request("POST", "/len", "127.0.0.1", Body::from(big)),
    )
    .await;
    assert_eq!(status, plain_status);
    assert_eq!(body, plain_body);
    assert_eq!(body, b"5242880".as_slice());
    let big_capture = latest(&oxray).await.unwrap();
    assert_eq!(big_capture["request"]["body"]["truncated"], true);
    assert_eq!(big_capture["request"]["body"]["captured_bytes"], 64 * 1024);
    assert_eq!(
        big_capture["request"]["body"]["size_bytes"],
        5 * 1024 * 1024
    );

    assert_first_chunk_before_delay(&support::routes_only()).await;
    assert_first_chunk_before_delay(&support::stack(oxray.clone(), true)).await;

    let (plain_status, _, plain_body) = support::call(
        support::routes_only(),
        support::request("GET", "/events", "127.0.0.1", Body::empty()),
    )
    .await;
    let (status, _, body) = support::call(
        support::stack(oxray.clone(), true),
        support::request("GET", "/events", "127.0.0.1", Body::empty()),
    )
    .await;
    assert_eq!(status, plain_status);
    assert_eq!(body, plain_body);
    let sse = latest(&oxray).await.unwrap();
    assert_eq!(sse["response"]["body"]["encoding"], "omitted");
    assert_eq!(sse["response"]["body"]["omitted_reason"], "event_stream");

    assert_same(&oxray, "GET", "/missing", Body::empty()).await;
    let missing = latest(&oxray).await.unwrap();
    assert_eq!(missing["status"], 404);
    assert!(missing["matched_path"].is_null());

    assert_same(&oxray, "POST", "/only-get", Body::empty()).await;
    assert_eq!(latest(&oxray).await.unwrap()["status"], 405);

    let (status, _, _) = support::call(
        support::stack(oxray.clone(), true),
        support::request_with(
            "POST",
            "/json",
            "127.0.0.1",
            Some("text/plain"),
            &[],
            Body::from("visible-body"),
        ),
    )
    .await;
    assert_eq!(status, 415);
    assert_eq!(
        latest(&oxray).await.unwrap()["request"]["body"]["text"],
        "visible-body"
    );

    assert_same(&oxray, "GET", "/fail", Body::empty()).await;
    assert_eq!(latest(&oxray).await.unwrap()["status"], 500);
    assert_eq!(latest(&oxray).await.unwrap()["flags"]["has_error"], true);
}

fn json_echo(payload: &str) -> http::Request<Body> {
    support::request_with(
        "POST",
        "/echo",
        "127.0.0.1",
        Some("application/json"),
        &[],
        Body::from(payload.to_owned()),
    )
}

async fn assert_same(oxray: &Oxray, method: &str, path: &str, body: Body) {
    let plain = support::request(method, path, "127.0.0.1", Body::empty());
    let (plain_status, plain_headers, plain_body) =
        support::call(support::routes_only(), plain).await;
    let (status, headers, response_body) = support::call(
        support::stack(oxray.clone(), true),
        support::request(method, path, "127.0.0.1", body),
    )
    .await;
    assert_eq!(status, plain_status);
    assert_eq!(response_body, plain_body);
    assert_eq!(
        support::without_id(&headers),
        support::without_id(&plain_headers)
    );
}

async fn assert_first_chunk_before_delay(app: &axum::Router) {
    let response = match app
        .clone()
        .oneshot(support::request(
            "GET",
            "/stream",
            "127.0.0.1",
            Body::empty(),
        ))
        .await
    {
        Ok(response) => response,
        Err(error) => match error {},
    };
    let mut body = response.into_body();
    let timed = tokio::time::timeout(Duration::from_millis(200), body.frame()).await;
    assert!(matches!(&timed, Ok(Some(Ok(_)))));
    if let Ok(Some(Ok(frame))) = timed {
        let data = frame.into_data();
        assert!(matches!(data, Ok(bytes) if bytes.as_ref() == b"a"));
    }
}

async fn only_capture(oxray: &Oxray) -> Option<Value> {
    let page = list(oxray, "").await?;
    if page["items"].as_array()?.len() == 1 {
        latest(oxray).await
    } else {
        None
    }
}

async fn latest(oxray: &Oxray) -> Option<Value> {
    let page = list(oxray, "?limit=1").await?;
    let id = page["items"][0]["id"].as_str()?;
    let (_, _, body) = support::call(
        support::stack(oxray.clone(), true),
        support::request(
            "GET",
            &format!("/_oxray/api/captures/{id}"),
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    serde_json::from_slice(&body).ok()
}

async fn list(oxray: &Oxray, query: &str) -> Option<Value> {
    let (_, _, body) = support::call(
        support::stack(oxray.clone(), true),
        support::request(
            "GET",
            &format!("/_oxray/api/captures{query}"),
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    serde_json::from_slice(&body).ok()
}
