//! Outgoing calls keep the reqwest response and attach to a child task.

#![cfg(feature = "reqwest")]

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::routing::get;
use http::Request;
use oxray::Oxray;
use tower::ServiceExt;
use tracing::Instrument;
use tracing_subscriber::prelude::*;

#[tokio::test]
async fn outgoing_response_matches_the_upstream() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            Router::new().route("/data", get(|| async { "payload" })),
        )
        .await;
    });
    let url = format!("http://{addr}/data");
    let plain = reqwest::Client::builder().build().unwrap();
    let oxray = Oxray::builder().build().unwrap();
    let wrapped =
        reqwest_middleware::ClientBuilder::new(reqwest::Client::builder().build().unwrap())
            .with(oxray.reqwest_middleware())
            .build();
    let left = plain.get(&url).send().await.unwrap();
    let right = wrapped.get(&url).send().await.unwrap();
    assert_eq!(left.status(), right.status());
    assert_eq!(left.version(), right.version());
    assert_eq!(left.headers(), right.headers());
    assert_eq!(left.url(), right.url());
    assert_eq!(left.remote_addr(), right.remote_addr());
    let left_body = left.bytes().await.unwrap();
    let right_body = right.bytes().await.unwrap();
    assert_eq!(left_body, right_body);
}

#[tokio::test]
async fn child_task_outgoing_is_attached_to_the_request() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            Router::new().route("/data", get(|| async { "payload" })),
        )
        .await;
    });
    let oxray = Oxray::builder().build().unwrap();
    let _ = tracing_subscriber::registry()
        .with(oxray.tracing_layer())
        .try_init();
    let target = format!("http://{addr}/data");
    let routed = oxray.clone();
    let app = Router::new()
        .route(
            "/work",
            get(move || {
                let target = target.clone();
                let oxray = routed.clone();
                async move {
                    let span = tracing::Span::current();
                    let oxray = oxray.clone();
                    tokio::spawn(
                        async move {
                            let client = reqwest_middleware::ClientBuilder::new(
                                reqwest::Client::builder().build().unwrap(),
                            )
                            .with(oxray.reqwest_middleware())
                            .build();
                            let _ = client.get(target).send().await;
                        }
                        .instrument(span),
                    )
                    .await
                    .unwrap();
                    "ok"
                }
            }),
        )
        .merge(oxray.router())
        .layer(oxray.layer())
        .layer(axum::middleware::from_fn(insert_peer));
    let response = app.oneshot(request("GET", "/work")).await.unwrap();
    assert_eq!(response.status(), 200);
    let _ = axum::body::to_bytes(response.into_body(), usize::MAX).await;
    let app = Router::new()
        .merge(oxray.router())
        .layer(axum::middleware::from_fn(insert_peer));
    let list = app
        .oneshot(request("GET", "/_oxray/api/captures?limit=1"))
        .await
        .unwrap();
    let body = axum::body::to_bytes(list.into_body(), usize::MAX)
        .await
        .unwrap();
    let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = page["items"][0]["id"].as_str().unwrap();
    let app = Router::new()
        .merge(oxray.router())
        .layer(axum::middleware::from_fn(insert_peer));
    let detail = app
        .oneshot(request("GET", &format!("/_oxray/api/captures/{id}")))
        .await
        .unwrap();
    let body = axum::body::to_bytes(detail.into_body(), usize::MAX)
        .await
        .unwrap();
    let capture: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        capture["outgoing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|call| call["url"].as_str().unwrap_or("").contains("/data")),
        "{capture}"
    );
}

fn request(method: &str, path: &str) -> Request<Body> {
    match Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1")
        .body(Body::empty())
    {
        Ok(request) => request,
        Err(_) => Request::new(Body::empty()),
    }
}

async fn insert_peer(
    mut request: Request<Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 9))));
    next.run(request).await
}
