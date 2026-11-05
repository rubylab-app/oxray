//! The mounted shell injects its path and a strict content security policy.

#[path = "common/mod.rs"]
mod support;

use axum::body::Body;
use http::StatusCode;

#[tokio::test]
async fn shell_injects_the_mount_and_serves_hashed_assets() {
    let oxray = oxray::Oxray::builder().build().unwrap();
    let (status, headers, body) = support::call(
        support::stack(oxray.clone(), true),
        support::request("GET", "/_oxray", "127.0.0.1", Body::empty()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (slash_status, _, slash_body) = support::call(
        support::stack(oxray.clone(), true),
        support::request("GET", "/_oxray/", "127.0.0.1", Body::empty()),
    )
    .await;
    assert_eq!(slash_status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&slash_body).contains("oxray-mount"));
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("<base href=\"/_oxray/\">"));
    assert!(html.contains("<meta name=\"oxray-mount\" content=\"/_oxray\">"));
    assert!(headers.iter().any(|(name, value)| {
        name == "content-security-policy"
            && value == "default-src 'self'; base-uri 'self'; frame-ancestors 'none'"
    }));

    let (deep_status, _, deep_body) = support::call(
        support::stack(oxray.clone(), true),
        support::request(
            "GET",
            "/_oxray/r/0193f2c4-7a1e-7c3d-9f42-6b1d2e8a51c7",
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(deep_status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&deep_body).contains("oxray-mount"));

    let (asset_status, asset_headers, asset_body) = support::call(
        support::stack(oxray, true),
        support::request(
            "GET",
            "/_oxray/assets/theme-init.js",
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(asset_status, StatusCode::OK);
    assert!(!asset_body.is_empty());
    assert!(
        asset_headers.iter().any(|(name, value)| {
            name == "content-type" && value.starts_with("text/javascript")
        })
    );
}

#[tokio::test]
async fn dotted_mount_slash_serves_the_shell() {
    let oxray = oxray::Oxray::builder()
        .mount("/v1.0/oxray")
        .build()
        .unwrap();
    let (status, _, body) = support::call(
        support::stack(oxray.clone(), true),
        support::request("GET", "/v1.0/oxray/", "127.0.0.1", Body::empty()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("<base href=\"/v1.0/oxray/\">"));
    assert!(html.contains("<meta name=\"oxray-mount\" content=\"/v1.0/oxray\">"));

    let (asset_status, asset_headers, asset_body) = support::call(
        support::stack(oxray.clone(), true),
        support::request(
            "GET",
            "/v1.0/oxray/assets/theme-init.js",
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(asset_status, StatusCode::OK);
    assert!(!asset_body.is_empty());
    assert!(
        asset_headers.iter().any(|(name, value)| {
            name == "content-type" && value.starts_with("text/javascript")
        })
    );

    let (missing_status, _, _) = support::call(
        support::stack(oxray, true),
        support::request(
            "GET",
            "/v1.0/oxray/assets/missing.js",
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(missing_status, StatusCode::NOT_FOUND);
}
