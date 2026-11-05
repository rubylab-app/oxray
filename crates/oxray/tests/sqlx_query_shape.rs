//! sqlx 0.9 emits the field shape Oxray reads.

#[path = "common/mod.rs"]
mod support;

use axum::body::Body;
use oxray::Oxray;
use tracing_subscriber::prelude::*;

#[tokio::test]
async fn sqlx_query_event_fills_the_capture() {
    let oxray = Oxray::builder().build().unwrap();
    let _ = tracing_subscriber::registry()
        .with(oxray.tracing_layer())
        .try_init();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let app = axum::Router::new()
        .route(
            "/sql",
            axum::routing::get(move || {
                let pool = pool.clone();
                async move {
                    let _ = sqlx::query("select 1").fetch_optional(&pool).await;
                    "ok"
                }
            }),
        )
        .layer(oxray.layer());
    let _ = support::call(
        app,
        support::request("GET", "/sql", "127.0.0.1", Body::empty()),
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
    let query = &capture["queries"][0];
    assert_eq!(query["sql"], "select 1");
    assert_eq!(query["fingerprint"], "select ?");
    assert!(query["elapsed_secs"].is_null() || query["duration_us"].is_number());
    assert!(query["duration_us"].as_u64().is_some());
}
