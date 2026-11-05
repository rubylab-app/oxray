//! Mount validation, disabled routing, and content-type case.

#[path = "common/mod.rs"]
mod support;

use std::net::SocketAddr;
use std::process::Command;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::get;
use http::{Request, StatusCode};
use oxray::{Error, Mode, Oxray};

#[test]
fn build_rejects_root_and_trailing_slash() {
    assert!(matches!(
        Oxray::builder().mount("/").build(),
        Err(Error::MountPath)
    ));
    assert!(matches!(
        Oxray::builder().mount("/_oxray/").build(),
        Err(Error::MountPath)
    ));
    assert!(matches!(
        Oxray::builder().mount("oxray").build(),
        Err(Error::MountPath)
    ));
    assert!(Oxray::builder().mount("/_oxray").build().is_ok());
    assert!(Oxray::builder().mount("/debug/oxray").build().is_ok());
}

#[tokio::test]
async fn dev_router_merges_when_the_app_already_has_a_fallback() {
    let oxray = Oxray::builder().build().unwrap();
    assert_eq!(oxray.mode(), Mode::Dev);
    let app = Router::new()
        .route("/only-get", get(|| async { "get" }))
        .fallback(|| async { (StatusCode::NOT_FOUND, "app-fallback") })
        .merge(oxray.router())
        .layer(oxray.layer())
        .layer(axum::middleware::from_fn(insert_peer));
    let (status, _, body) = support::call(
        app.clone(),
        support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_ref(), b"get");
    let (status, _, body) = support::call(
        app.clone(),
        support::request("GET", "/missing", "127.0.0.1", Body::empty()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body.as_ref(), b"app-fallback");
    let (status, _, _) = support::call(
        app,
        support::request("GET", "/_oxray/api/meta", "127.0.0.1", Body::empty()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[test]
fn disabled_router_404s_only_under_the_mount() {
    if std::env::var_os("OXRAY_DISABLED_ROUTER_CHILD").is_some() {
        let oxray = Oxray::builder().build().unwrap();
        assert_eq!(oxray.mode(), Mode::Disabled);
        let app = Router::new()
            .route("/only-get", get(|| async { "get" }))
            .fallback(|| async { (StatusCode::NOT_FOUND, "app-fallback") })
            .merge(oxray.router());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (status, _, body) = support::call(
                app.clone(),
                support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body.as_ref(), b"get");
            let (status, _, body) = support::call(
                app.clone(),
                support::request("GET", "/missing", "127.0.0.1", Body::empty()),
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert_eq!(body.as_ref(), b"app-fallback");
            let (status, _, body) = support::call(
                app,
                support::request("GET", "/_oxray/api/captures", "127.0.0.1", Body::empty()),
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert!(body.is_empty());
        });
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("disabled_router_404s_only_under_the_mount")
        .env("OXRAY", "off")
        .env("OXRAY_DISABLED_ROUTER_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn mixed_case_media_types_follow_the_redactor() {
    let oxray = Oxray::builder().build().unwrap();
    let _ = support::call(
        support::stack(oxray.clone(), true),
        support::request_with(
            "POST",
            "/text",
            "127.0.0.1",
            Some("Application/JSON; charset=UTF-8"),
            &[],
            Body::from(
                r#"{"password":"hunter2-synthetic","n":4242424242424242,"clabe":123456789012345673}"#,
            ),
        ),
    )
    .await;
    let capture = only_capture(&oxray).await.unwrap();
    let text = capture["request"]["body"]["text"].as_str().unwrap();
    assert!(!text.contains("hunter2-synthetic"));
    assert!(!text.contains("4242424242424242"));
    assert!(!text.contains("123456789012345673"));
    assert_eq!(capture["request"]["body"]["encoding"], "utf8");

    let oxray = Oxray::builder().build().unwrap();
    let _ = support::call(
        support::stack(oxray.clone(), true),
        support::request_with(
            "POST",
            "/sink",
            "127.0.0.1",
            Some("Multipart/Form-Data; boundary=oxray"),
            &[],
            Body::from("4242424242424242"),
        ),
    )
    .await;
    let capture = only_capture(&oxray).await.unwrap();
    assert_eq!(capture["request"]["body"]["omitted_reason"], "multipart");
    assert!(capture["request"]["body"]["text"].is_null());
    assert!(!capture.to_string().contains("4242424242424242"));

    let oxray = Oxray::builder().build().unwrap();
    let _ = support::call(
        support::stack(oxray.clone(), true),
        support::request_with(
            "POST",
            "/sink",
            "127.0.0.1",
            Some("Text/Event-Stream"),
            &[],
            Body::from("4242424242424242"),
        ),
    )
    .await;
    let capture = only_capture(&oxray).await.unwrap();
    assert_eq!(capture["request"]["body"]["omitted_reason"], "event_stream");
    assert!(!capture.to_string().contains("4242424242424242"));
}

async fn only_capture(oxray: &Oxray) -> Option<serde_json::Value> {
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
    let page: serde_json::Value = serde_json::from_slice(&body).ok()?;
    let id = page["items"][0]["id"].as_str()?;
    let (_, _, detail) = support::call(
        support::stack(oxray.clone(), true),
        support::request(
            "GET",
            &format!("/_oxray/api/captures/{id}"),
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    serde_json::from_slice(&detail).ok()
}

async fn insert_peer(mut request: Request<Body>, next: Next) -> Response {
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 9))));
    next.run(request).await
}
