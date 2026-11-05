//! Internal capture failures leave the application response intact.

#[cfg(feature = "__fault-injection")]
#[path = "../common/mod.rs"]
mod support;

#[cfg(feature = "__fault-injection")]
use axum::body::Body;
#[cfg(feature = "__fault-injection")]
use oxray::{Mode, Oxray};

#[cfg(feature = "__fault-injection")]
#[tokio::test]
async fn store_and_redactor_failures_warn_once() {
    let healthy = Oxray::builder().build().unwrap();
    assert_eq!(healthy.mode(), Mode::Dev);
    let (healthy_status, healthy_headers, healthy_body) = support::call(
        support::stack(healthy, true),
        support::request_with(
            "POST",
            "/text",
            "127.0.0.1",
            Some("text/plain"),
            &[],
            Body::from("hello"),
        ),
    )
    .await;

    for (arm_store, warns) in [(true, "store"), (false, "redact")] {
        let oxray = Oxray::builder().build().unwrap();
        if arm_store {
            oxray.fault_fail_store();
        } else {
            oxray.fault_fail_redact();
        }
        for _ in 0..2 {
            let (status, headers, body) = support::call(
                support::stack(oxray.clone(), true),
                support::request_with(
                    "POST",
                    "/text",
                    "127.0.0.1",
                    Some("text/plain"),
                    &[],
                    Body::from("hello"),
                ),
            )
            .await;
            assert_eq!(status, healthy_status);
            assert_eq!(body, healthy_body);
            assert_eq!(
                support::without_id(&headers),
                support::without_id(&healthy_headers)
            );
        }
        if arm_store {
            assert_eq!(oxray.fault_store_warns(), 1, "{warns}");
        } else {
            assert_eq!(oxray.fault_redact_warns(), 1, "{warns}");
        }
    }
}
