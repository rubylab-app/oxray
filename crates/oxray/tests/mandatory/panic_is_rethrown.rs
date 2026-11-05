//! Handler panics reach the caller.

#[path = "../common/mod.rs"]
mod support;

use std::panic::AssertUnwindSafe;

use axum::body::Body;
use futures_util::FutureExt;
use oxray::{Mode, Oxray};
use tower::ServiceExt;

#[tokio::test]
async fn panic_reaches_the_caller() {
    let oxray = Oxray::builder().build().unwrap();
    assert_eq!(oxray.mode(), Mode::Dev);
    let plain = AssertUnwindSafe(support::routes_only().oneshot(support::request(
        "GET",
        "/boom",
        "127.0.0.1",
        Body::empty(),
    )))
    .catch_unwind()
    .await;
    let recorded = AssertUnwindSafe(support::stack(oxray, true).oneshot(support::request(
        "GET",
        "/boom",
        "127.0.0.1",
        Body::empty(),
    )))
    .catch_unwind()
    .await;
    let plain_text = panic_text(plain);
    let recorded_text = panic_text(recorded);
    assert_eq!(plain_text, recorded_text);
    assert!(recorded_text.contains("boom-marker"));
}

fn panic_text(
    result: Result<
        Result<axum::response::Response, std::convert::Infallible>,
        Box<dyn std::any::Any + Send>,
    >,
) -> String {
    let Err(payload) = result else {
        return String::new();
    };
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_owned();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    String::new()
}
