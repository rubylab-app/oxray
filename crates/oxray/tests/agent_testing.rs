//! Reading back the capture of a request from a test, so a failing assert
//! can print it.

#[path = "common/mod.rs"]
mod support;

use std::process::Command;

use axum::body::Body;
use axum::http::StatusCode;
use bytes::Bytes;
use oxray::Oxray;
use tower::ServiceExt;

/// Sends `request` through `app`, reads the response body to the end, and
/// returns the status, the `x-oxray-id` header, and the body bytes.
async fn send(
    app: &axum::Router,
    request: http::Request<Body>,
) -> (StatusCode, Option<String>, Bytes) {
    let response = match app.clone().oneshot(request).await {
        Ok(response) => response,
        Err(error) => match error {},
    };
    let status = response.status();
    let id = response
        .headers()
        .get("x-oxray-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let Ok(body) = axum::body::to_bytes(response.into_body(), usize::MAX).await else {
        return (status, id, Bytes::new());
    };
    (status, id, body)
}

#[tokio::test]
async fn capture_by_id_matches_the_request() {
    let oxray = Oxray::builder().build().unwrap();
    let app = support::stack(oxray.clone(), true);
    let (status, id, _body) = send(
        &app,
        support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = id.unwrap();
    let capture = oxray.capture(&id).unwrap();
    assert_eq!(capture.id, id);
    assert_eq!(capture.method, "GET");
    assert_eq!(capture.path, "/only-get");
    assert_eq!(capture.status, Some(200));
}

#[tokio::test]
async fn latest_capture_is_the_newer_of_two_requests() {
    let oxray = Oxray::builder().build().unwrap();
    let app = support::stack(oxray.clone(), true);
    let (_, first, _) = send(
        &app,
        support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
    )
    .await;
    let (_, second, _) = send(
        &app,
        support::request("POST", "/sink", "127.0.0.1", Body::from("ping")),
    )
    .await;
    let first = first.unwrap();
    let second = second.unwrap();
    assert_ne!(first, second);
    let latest = oxray.latest_capture().unwrap();
    assert_eq!(latest.id, second);
    assert_ne!(latest.id, first);
    assert_eq!(latest.method, "POST");
    assert_eq!(latest.path, "/sink");
}

#[tokio::test]
async fn unknown_and_evicted_ids_return_none() {
    let oxray = Oxray::builder().build().unwrap();
    let app = support::stack(oxray.clone(), true);
    let _ = send(
        &app,
        support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
    )
    .await;
    assert!(
        oxray
            .capture("0193f2c4-0000-7000-8000-000000000000")
            .is_none()
    );

    let oxray = Oxray::builder().max_captures(1).build().unwrap();
    let app = support::stack(oxray.clone(), true);
    let (_, first, _) = send(
        &app,
        support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
    )
    .await;
    let (_, second, _) = send(
        &app,
        support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
    )
    .await;
    let first = first.unwrap();
    let second = second.unwrap();
    assert!(oxray.capture(&first).is_none());
    assert_eq!(oxray.capture(&second).unwrap().id, second);
    assert_eq!(oxray.latest_capture().unwrap().id, second);
}

#[tokio::test]
async fn a_failing_assert_can_print_the_markdown() {
    let oxray = Oxray::builder().build().unwrap();
    let app = support::stack(oxray.clone(), true);
    let (status, id, _body) = send(
        &app,
        support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
    )
    .await;
    let capture = oxray.capture(&id.unwrap()).unwrap();
    assert_eq!(status, StatusCode::OK, "{}", capture.markdown());
}

#[test]
fn disabled_mode_stores_and_returns_nothing() {
    if std::env::var_os("OXRAY_AGENT_TESTING_CHILD").is_some() {
        let oxray = Oxray::builder().build().unwrap();
        assert_eq!(oxray.mode(), oxray::Mode::Disabled);
        let app = support::stack(oxray.clone(), true);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (status, id, _body) = send(
                &app,
                support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert!(id.is_none());
            assert!(
                oxray
                    .capture("0193f2c4-0000-7000-8000-000000000000")
                    .is_none()
            );
            assert!(oxray.latest_capture().is_none());
        });
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("disabled_mode_stores_and_returns_nothing")
        .env("OXRAY", "off")
        .env("OXRAY_AGENT_TESTING_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
