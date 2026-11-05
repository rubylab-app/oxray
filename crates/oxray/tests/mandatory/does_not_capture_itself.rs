//! Requests to the mount path are not captured.

#[path = "../common/mod.rs"]
mod support;

use axum::body::Body;
use oxray::{Mode, Oxray};

#[tokio::test]
async fn mount_requests_do_not_create_captures() {
    let oxray = Oxray::builder().build().unwrap();
    assert_eq!(oxray.mode(), Mode::Dev);
    let _ = support::call(
        support::stack(oxray.clone(), true),
        support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
    )
    .await;
    for path in [
        "/_oxray/api/meta",
        "/_oxray/api/captures",
        "/_oxray/missing",
    ] {
        let _ = support::call(
            support::stack(oxray.clone(), true),
            support::request("GET", path, "127.0.0.1", Body::empty()),
        )
        .await;
    }
    let (_, _, body) = support::call(
        support::stack(oxray, true),
        support::request("GET", "/_oxray/api/captures", "127.0.0.1", Body::empty()),
    )
    .await;
    let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["path"], "/only-get");
}
