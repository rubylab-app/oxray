//! A 415 response still records the request body.

#[path = "common/mod.rs"]
mod support;

use axum::body::Body;
use oxray::Oxray;

#[tokio::test]
async fn unsupported_media_type_keeps_the_request_body() {
    let oxray = Oxray::builder().body_limit(64 * 1024).build().unwrap();
    let sent = "{\"note\":\"visible-body\"}";
    let (status, _, _) = support::call(
        support::stack(oxray.clone(), true),
        support::request_with(
            "POST",
            "/json",
            "127.0.0.1",
            Some("text/plain"),
            &[],
            Body::from(sent),
        ),
    )
    .await;
    assert_eq!(status, 415);
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
    assert_eq!(capture["request"]["body"]["text"], sent);
}
