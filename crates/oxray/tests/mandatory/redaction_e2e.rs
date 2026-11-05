//! Sensitive values do not survive in the store or the API.

#[path = "../common/mod.rs"]
mod support;

use axum::body::Body;
use oxray::{Mode, Oxray};

#[tokio::test]
async fn corpus_is_absent_from_store_and_api() {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/sensitive_corpus.json")).unwrap();
    assert!(corpus.values.len() >= 5);
    let oxray = Oxray::builder().body_limit(64 * 1024).build().unwrap();
    assert_eq!(oxray.mode(), Mode::Dev);
    let json = format!(
        "{{\"password\":\"{}\",\"note\":\"{}\",\"clabe\":\"{}\"}}",
        corpus.values[3], corpus.values[1], corpus.values[2]
    );
    let _ = support::call(
        support::stack(oxray.clone(), true),
        support::request_with(
            "POST",
            "/sink?q=4242424242424242",
            "127.0.0.1",
            Some("application/json"),
            &[("authorization", corpus.values[4].as_str())],
            Body::from(json),
        ),
    )
    .await;
    let form = format!("secret={}", corpus.values[3]);
    let _ = support::call(
        support::stack(oxray.clone(), true),
        support::request_with(
            "POST",
            "/sink",
            "127.0.0.1",
            Some("application/x-www-form-urlencoded"),
            &[],
            Body::from(form),
        ),
    )
    .await;
    let (_, _, list_body) = support::call(
        support::stack(oxray.clone(), true),
        support::request("GET", "/_oxray/api/captures", "127.0.0.1", Body::empty()),
    )
    .await;
    let page: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
    let items = page["items"].as_array().unwrap();
    assert!(items.len() >= 2);
    let mut artifacts = String::from_utf8(list_body.to_vec()).unwrap();
    for item in items {
        let id = item["id"].as_str().unwrap();
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
        artifacts.push_str(&String::from_utf8_lossy(&detail));
    }
    assert!(
        artifacts.contains("sensitive_key")
            || artifacts.contains("[redacted]")
            || artifacts.contains("clabe")
    );
    let normalized = normalize(&artifacts);
    for value in &corpus.values {
        let secret = normalize(value);
        assert!(!normalized.contains(&secret), "{secret}");
    }
}

#[derive(serde::Deserialize)]
struct Corpus {
    values: Vec<String>,
}

fn normalize(input: &str) -> String {
    let mut text = input.to_owned();
    for _ in 0..3 {
        text = percent_encoding::percent_decode_str(&text)
            .decode_utf8_lossy()
            .into_owned();
    }
    text = unescape_json(&text);
    let chars: Vec<char> = text.chars().collect();
    let mut stripped = String::new();
    for (index, ch) in chars.iter().enumerate() {
        if (*ch == ' ' || *ch == '-')
            && index > 0
            && index + 1 < chars.len()
            && chars[index - 1].is_ascii_digit()
            && chars[index + 1].is_ascii_digit()
        {
            continue;
        }
        stripped.push(*ch);
    }
    stripped.to_lowercase()
}

fn unescape_json(input: &str) -> String {
    let mut chars = input.chars().peekable();
    let mut output = String::new();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }
        match chars.next() {
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Ok(code) = u32::from_str_radix(&hex, 16)
                    && let Some(decoded) = char::from_u32(code)
                {
                    output.push(decoded);
                    continue;
                }
            }
            Some(other) => output.push(other),
            None => {}
        }
    }
    output
}
