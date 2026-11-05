//! The store stays inside its configured caps.

#[path = "../common/mod.rs"]
mod support;

use axum::body::Body;
use oxray::{Mode, Oxray};

#[tokio::test]
async fn ten_thousand_large_bodies_stay_inside_the_caps() {
    let body_limit = 1024;
    let max_captures = 4;
    let max_bytes = 1_000_000;
    let oxray = Oxray::builder()
        .body_limit(body_limit)
        .max_captures(max_captures)
        .max_bytes(max_bytes)
        .build()
        .unwrap();
    assert_eq!(oxray.mode(), Mode::Dev);
    let payload = vec![b'x'; 1024 * 1024];
    assert!(payload.len() > body_limit);
    for _ in 0..10_000 {
        let (status, _, body) = support::call(
            support::stack(oxray.clone(), true),
            support::request("POST", "/len", "127.0.0.1", Body::from(payload.clone())),
        )
        .await;
        assert_eq!(status, 200);
        assert_eq!(body, b"1048576".as_slice());
    }
    let (_, _, meta_body) = support::call(
        support::stack(oxray, true),
        support::request("GET", "/_oxray/api/meta", "127.0.0.1", Body::empty()),
    )
    .await;
    let meta: serde_json::Value = serde_json::from_slice(&meta_body).unwrap();
    let captures = meta["store"]["captures"].as_u64().unwrap();
    let bytes = meta["store"]["bytes"].as_u64().unwrap();
    assert!(captures <= max_captures as u64);
    assert!(bytes <= max_bytes as u64);
    assert!(captures > 0);
}

#[cfg(feature = "reqwest")]
#[tokio::test]
async fn side_table_stays_within_one_thousand() {
    let oxray = Oxray::builder().build().unwrap();
    assert_eq!(oxray.mode(), Mode::Dev);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            axum::Router::new().route("/x", axum::routing::get(|| async { "ok" })),
        )
        .await;
    });
    let client =
        reqwest_middleware::ClientBuilder::new(reqwest::Client::builder().build().unwrap())
            .with(oxray.reqwest_middleware())
            .build();
    for _ in 0..1_100 {
        let response = client.get(format!("http://{addr}/x")).send().await;
        assert!(response.is_ok());
    }
    let pending = oxray.pending_outgoing();
    assert!(pending <= 1_000);
    assert!(pending > 0);
}
