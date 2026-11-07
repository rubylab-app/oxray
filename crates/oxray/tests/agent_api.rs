//! Agent-facing API: the markdown route and the capture list filters.

#[path = "common/mod.rs"]
mod support;

use std::panic::AssertUnwindSafe;

use axum::body::Body;
use futures_util::FutureExt;
use http::StatusCode;
use oxray::Oxray;
use serde_json::Value;
use tower::ServiceExt;

/// Sends a request through the real router and returns the capture id.
async fn record(app: &axum::Router, method: &str, path: &str) -> Option<String> {
    let _ = support::call(
        app.clone(),
        support::request(method, path, "127.0.0.1", Body::empty()),
    )
    .await;
    newest_id(app).await
}

async fn newest_id(app: &axum::Router) -> Option<String> {
    let page = list(app, "/api/captures?limit=1").await?;
    Some(page["items"][0]["id"].as_str()?.to_owned())
}

async fn list(app: &axum::Router, query: &str) -> Option<Value> {
    let (_, _, body) = support::call(
        app.clone(),
        support::request(
            "GET",
            &format!("/_oxray{query}"),
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    serde_json::from_slice(&body).ok()
}

fn ids(page: &Value) -> Option<Vec<String>> {
    page["items"]
        .as_array()?
        .iter()
        .map(|item| item["id"].as_str().map(str::to_owned))
        .collect()
}

fn content_type(headers: &[(String, String)]) -> String {
    headers
        .iter()
        .find(|(name, _)| name == "content-type")
        .map(|(_, value)| value.clone())
        .unwrap_or_default()
}

#[tokio::test]
async fn markdown_route_and_list_filters() {
    let oxray = Oxray::builder().build().unwrap();
    let app = support::stack(oxray.clone(), true);

    let ok_id = record(&app, "GET", "/only-get").await.unwrap();
    let missing_id = record(&app, "GET", "/missing").await.unwrap();
    let fail_id = record(&app, "GET", "/fail").await.unwrap();
    let boom_app = support::stack(oxray.clone(), true);
    let _ = AssertUnwindSafe(boom_app.oneshot(support::request(
        "GET",
        "/boom",
        "127.0.0.1",
        Body::empty(),
    )))
    .catch_unwind()
    .await;
    let boom_id = newest_id(&app).await.unwrap();
    assert_ne!(boom_id, fail_id);

    // The markdown route serves text/markdown starting with the header.
    let (status, headers, body) = support::call(
        app.clone(),
        support::request(
            "GET",
            &format!("/_oxray/api/captures/{fail_id}/markdown"),
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type(&headers), "text/markdown; charset=utf-8");
    let markdown = String::from_utf8_lossy(&body);
    assert!(markdown.starts_with("## "), "{markdown}");
    assert!(
        markdown.ends_with("\n") && !markdown.ends_with("\n\n"),
        "{markdown}"
    );
    assert!(markdown.contains("→ 500 in "), "{markdown}");

    // An unknown id gets the same 404 message as the JSON route.
    let unknown = "0193f2c4-0000-7000-8000-000000000000";
    let (status, headers, body) = support::call(
        app.clone(),
        support::request(
            "GET",
            &format!("/_oxray/api/captures/{unknown}/markdown"),
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type(&headers), "text/plain; charset=utf-8");
    assert_eq!(
        String::from_utf8_lossy(&body),
        format!("Capture {unknown} is not in the store")
    );

    // Status filters.
    assert_eq!(
        ids(&list(&app, "/api/captures?status=5xx").await.unwrap()).unwrap(),
        vec![fail_id.clone()]
    );
    assert_eq!(
        ids(&list(&app, "/api/captures?status=200").await.unwrap()).unwrap(),
        vec![ok_id.clone()]
    );
    // Failed filters: has_error or a null status.
    let mut failed = ids(&list(&app, "/api/captures?failed=true").await.unwrap()).unwrap();
    failed.sort();
    let mut expected_failed = vec![fail_id.clone(), boom_id.clone()];
    expected_failed.sort();
    assert_eq!(failed, expected_failed);
    let mut not_failed = ids(&list(&app, "/api/captures?failed=false").await.unwrap()).unwrap();
    not_failed.sort();
    let mut expected_ok = vec![ok_id.clone(), missing_id.clone()];
    expected_ok.sort();
    assert_eq!(not_failed, expected_ok);
    // After keeps only newer ids.
    assert_eq!(
        ids(&list(&app, &format!("/api/captures?after={missing_id}"))
            .await
            .unwrap())
        .unwrap(),
        vec![boom_id.clone(), fail_id.clone()]
    );
    // An invalid status is a 400 with the documented message.
    let (status, headers, body) = support::call(
        app.clone(),
        support::request(
            "GET",
            "/_oxray/api/captures?status=abc",
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(content_type(&headers), "text/plain; charset=utf-8");
    assert_eq!(
        String::from_utf8_lossy(&body),
        "status must be a code like 500 or a class like 5xx"
    );

    // A filter plus limit=1 pages through the matching captures.
    let page = list(&app, "/api/captures?failed=false&limit=1")
        .await
        .unwrap();
    assert_eq!(ids(&page).unwrap(), vec![missing_id.clone()]);
    let cursor = page["next_cursor"].as_str().unwrap().to_owned();
    assert_eq!(cursor, missing_id);
    let next = list(
        &app,
        &format!("/api/captures?failed=false&limit=1&cursor={cursor}"),
    )
    .await
    .unwrap();
    assert_eq!(ids(&next).unwrap(), vec![ok_id.clone()]);
    assert!(next["next_cursor"].is_null());
}

#[tokio::test]
async fn markdown_of_a_redacted_capture_has_no_corpus_values() {
    let corpus: Value =
        serde_json::from_str(include_str!("fixtures/sensitive_corpus.json")).unwrap();
    let values: Vec<&str> = corpus["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    let oxray = Oxray::builder().build().unwrap();
    let app = support::stack(oxray.clone(), true);
    let payload = serde_json::json!({
        "password": values[3],
        "note": values[1],
        "clabe": values[2],
        "pan": values[0],
    })
    .to_string();
    let _ = support::call(
        app.clone(),
        support::request_with(
            "POST",
            &format!("/echo?token={}", values[3]),
            "127.0.0.1",
            Some("application/json"),
            &[("authorization", values[4])],
            Body::from(payload),
        ),
    )
    .await;
    let id = newest_id(&app).await.unwrap();
    let (status, _, body) = support::call(
        app.clone(),
        support::request(
            "GET",
            &format!("/_oxray/api/captures/{id}/markdown"),
            "127.0.0.1",
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let markdown = String::from_utf8_lossy(&body).into_owned();
    for secret in &values {
        assert!(
            !markdown.contains(secret),
            "leaked {secret} in:\n{markdown}"
        );
    }
    // Normalized form: spaces and hyphens removed, digits-only corpus values.
    let normalized: String = markdown
        .chars()
        .filter(|ch| *ch != ' ' && *ch != '-')
        .collect();
    for secret in &values {
        let digits: String = secret.chars().filter(|ch| ch.is_ascii_digit()).collect();
        let non_digits = secret.chars().filter(|ch| !ch.is_ascii_digit()).count();
        if !digits.is_empty() && digits.len() > non_digits {
            assert!(
                !normalized.contains(&digits),
                "leaked digits {digits} of {secret} in:\n{markdown}"
            );
        }
    }
}
