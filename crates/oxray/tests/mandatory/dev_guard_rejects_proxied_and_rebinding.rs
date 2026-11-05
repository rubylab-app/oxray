//! Dev mode rejects tunnels, proxies, and rebinding.

#[path = "../common/mod.rs"]
mod support;

use axum::body::Body;
use oxray::{Mode, Oxray};

#[tokio::test]
async fn proxied_and_rebinding_requests_are_forbidden() {
    let oxray = Oxray::builder().build().unwrap();
    assert_eq!(oxray.mode(), Mode::Dev);
    let (status, _, _) = support::call(
        support::stack(oxray.clone(), true),
        support::request("GET", "/_oxray/api/meta", "127.0.0.1", Body::empty()),
    )
    .await;
    assert_eq!(status, 200);

    let (status, _, _) = support::call(
        support::stack(oxray.clone(), true),
        support::request("GET", "/_oxray/api/meta", "evil.example", Body::empty()),
    )
    .await;
    assert_eq!(status, 403);

    let (status, _, _) = support::call(
        support::stack(oxray.clone(), true),
        support::request_with(
            "GET",
            "/_oxray/api/meta",
            "127.0.0.1",
            None,
            &[("x-forwarded-for", "203.0.113.5")],
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(status, 403);

    let (status, _, _) = support::call(
        support::stack(oxray.clone(), true),
        support::request_with(
            "GET",
            "/_oxray/api/meta",
            "127.0.0.1",
            None,
            &[("origin", "http://evil.example")],
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(status, 403);

    let (status, _, body) = support::call(
        support::stack(oxray, false),
        support::request("GET", "/_oxray/api/meta", "127.0.0.1", Body::empty()),
    )
    .await;
    assert_eq!(status, 403);
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("into_make_service_with_connect_info"));
}
