//! Release builds stay off without an explicit opt-in.

#[path = "../common/mod.rs"]
mod support;

use axum::body::Body;
use oxray::{Mode, Oxray};

#[tokio::test]
async fn release_without_opt_in_does_not_capture() {
    let oxray = Oxray::builder().build().unwrap();
    if cfg!(oxray_release_profile) {
        assert_eq!(oxray.mode(), Mode::Disabled);
        let (status, headers, _) = support::call(
            support::stack(oxray.clone(), true),
            support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
        )
        .await;
        assert_eq!(status, 200);
        assert!(
            headers
                .iter()
                .all(|(name, _)| !name.eq_ignore_ascii_case("x-oxray-id"))
        );
        let (api_status, _, _) = support::call(
            support::stack(oxray, true),
            support::request("GET", "/_oxray/api/captures", "127.0.0.1", Body::empty()),
        )
        .await;
        assert_eq!(api_status, 404);
    } else {
        assert_eq!(oxray.mode(), Mode::Dev);
        let (status, _, _) = support::call(
            support::stack(oxray.clone(), true),
            support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
        )
        .await;
        assert_eq!(status, 200);
        let (api_status, _, body) = support::call(
            support::stack(oxray, true),
            support::request("GET", "/_oxray/api/captures", "127.0.0.1", Body::empty()),
        )
        .await;
        assert_eq!(api_status, 200);
        let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(page["items"].as_array().unwrap().len(), 1);
    }
}
