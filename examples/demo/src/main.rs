//! Minimal Axum app that exercises Oxray in dev.
//!
//! `GET /` answers `ok`. `POST /webhooks/{psp}` records a signed payment
//! webhook. `POST /corpus` runs every sensitive surface through the redactor.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use hmac::Mac;
use oxray::Oxray;
use serde::Deserialize;
use sha2::Sha256;
use sqlx::sqlite::SqlitePool;
use tokio::net::TcpListener;
use tower_http::catch_panic::CatchPanicLayer;

/// Public demo key. It only signs the synthetic webhook in this example.
const DEMO_KEY: &[u8] = b"demo-secret";

type HmacSha256 = hmac::Hmac<Sha256>;
type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
    processor: String,
    http: reqwest_middleware::ClientWithMiddleware,
}

#[derive(Clone)]
struct ProcessorState {
    calls: std::sync::Arc<AtomicU32>,
}

#[derive(Deserialize)]
struct CorpusFile {
    values: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("seed") => seed_main(args).await,
        Some("serve") => serve_main(args).await,
        Some(other) => {
            writeln!(std::io::stderr(), "unknown command {other}")?;
            std::process::exit(2);
        }
        None => serve(None, None).await,
    }
}

async fn serve_main(mut args: impl Iterator<Item = String>) -> Result<(), BoxError> {
    let mut mount = None;
    let mut port = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--mount" => {
                mount = Some(
                    args.next()
                        .ok_or_else(|| string_err("usage: demo serve --mount <path>"))?,
                );
            }
            "--port" => {
                let raw = args
                    .next()
                    .ok_or_else(|| string_err("usage: demo serve --port <port>"))?;
                port = Some(
                    raw.parse::<u16>()
                        .map_err(|_| string_err("port must be a number"))?,
                );
            }
            _ => {
                return Err(string_err(
                    "usage: demo serve [--mount <path>] [--port <port>]",
                ));
            }
        }
    }
    serve(mount, port).await
}

async fn serve(mount: Option<String>, port: Option<u16>) -> Result<(), BoxError> {
    let processor = spawn_processor().await?;
    let mut builder = Oxray::builder();
    if let Some(mount) = mount {
        builder = builder.mount(mount);
    }
    let oxray = builder.build()?;
    let state = app_state(&oxray, &processor).await?;
    install_tracing(&oxray)?;
    let listener = match port {
        Some(port) => TcpListener::bind(("127.0.0.1", port)).await?,
        None => TcpListener::bind("127.0.0.1:0").await?,
    };
    let addr = listener.local_addr()?;
    writeln!(std::io::stdout(), "demo listening on http://{addr}")?;
    axum::serve(
        listener,
        router(oxray, state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

async fn seed_main(mut args: impl Iterator<Item = String>) -> Result<(), BoxError> {
    let flag = args
        .next()
        .ok_or_else(|| string_err("usage: demo seed --url <url>"))?;
    if flag != "--url" {
        return Err(string_err("usage: demo seed --url <url>"));
    }
    let url = args
        .next()
        .ok_or_else(|| string_err("usage: demo seed --url <url>"))?;
    if args.next().is_some() {
        return Err(string_err("unexpected extra argument"));
    }
    seed(&url).await
}

async fn seed(base: &str) -> Result<(), BoxError> {
    let base = base.trim_end_matches('/');
    let client = reqwest::Client::builder().build()?;
    client.get(format!("{base}/")).send().await?;
    let body = webhook_body();
    let signature = sign(body.as_bytes())?;
    client
        .post(format!("{base}/webhooks/acme"))
        .header("x-oxray-signature", signature)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await?;
    Ok(())
}

fn router(oxray: Oxray, state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/webhooks/{psp}", post(webhook))
        .route("/corpus", post(corpus))
        .with_state(state)
        .merge(oxray.router())
        .layer(CatchPanicLayer::new())
        .layer(oxray.layer())
}

async fn root(ConnectInfo(_peer): ConnectInfo<SocketAddr>) -> &'static str {
    "ok"
}

async fn webhook(
    State(state): State<AppState>,
    Path(_psp): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    let provided = headers
        .get("x-oxray-signature")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let Ok(expected) = sign(&body) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if provided != expected {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let insert = sqlx::query("insert into payments (note) values (?)")
        .bind("ok")
        .execute(&state.pool)
        .await;
    if insert.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    for id in 0..14 {
        let query = sqlx::query("select note from payments where id = ?")
            .bind(id)
            .fetch_optional(&state.pool)
            .await;
        if query.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    let url = format!("{}/charge", state.processor);
    let _ = state.http.post(url).body(body.to_vec()).send().await;
    (StatusCode::INTERNAL_SERVER_ERROR, "processor failed").into_response()
}

async fn corpus(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(values) = corpus_values() else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };
    let pan = values.first().map(String::as_str).unwrap_or("");
    let password = values.get(3).map(String::as_str).unwrap_or("");
    tracing::info!(target: "demo", password, "corpus field");
    let _ = sqlx::query("select '4242424242424242'")
        .fetch_optional(&state.pool)
        .await;
    let url = format!("{}/echo?pan={pan}", state.processor);
    let payload = format!("{{\"password\":\"{password}\"}}");
    let _ = state
        .http
        .post(url)
        .header("content-type", "application/json")
        .body(payload)
        .send()
        .await;
    let _ = (query, headers, body);
    StatusCode::OK
}

async fn charge(State(state): State<ProcessorState>) -> StatusCode {
    let call = state.calls.fetch_add(1, Ordering::Relaxed) + 1;
    if call % 3 == 0 {
        StatusCode::BAD_GATEWAY
    } else {
        StatusCode::OK
    }
}

async fn echo() -> &'static str {
    "ok"
}

async fn spawn_processor() -> Result<String, BoxError> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let state = ProcessorState {
        calls: std::sync::Arc::new(AtomicU32::new(0)),
    };
    let app = Router::new()
        .route("/charge", post(charge))
        .route("/echo", post(echo))
        .with_state(state);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(format!("http://{addr}"))
}

async fn app_state(oxray: &Oxray, processor: &str) -> Result<AppState, BoxError> {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    sqlx::query("create table payments (id integer primary key, note text)")
        .execute(&pool)
        .await?;
    let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::builder().build()?)
        .with(oxray.reqwest_middleware())
        .build();
    Ok(AppState {
        pool,
        processor: processor.to_owned(),
        http,
    })
}

fn install_tracing(oxray: &Oxray) -> Result<(), BoxError> {
    use tracing_subscriber::prelude::*;
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_filter(tracing_subscriber::EnvFilter::new("info")),
        )
        .with(oxray.tracing_layer())
        .try_init()
        .map_err(|err| string_err(err.to_string()))?;
    Ok(())
}

fn webhook_body() -> String {
    "{\"card\":\"4242 4242 4242 4242\"}".to_owned()
}

fn sign(body: &[u8]) -> Result<String, hmac::digest::InvalidLength> {
    let mut mac = HmacSha256::new_from_slice(DEMO_KEY)?;
    mac.update(body);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn corpus_values() -> Option<Vec<String>> {
    let file: CorpusFile = serde_json::from_str(include_str!(
        "../../../crates/oxray/tests/fixtures/sensitive_corpus.json"
    ))
    .ok()?;
    Some(file.values)
}

fn string_err(message: impl Into<String>) -> BoxError {
    Box::new(std::io::Error::other(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn root_answers_ok_for_a_loopback_peer() {
        let peer = SocketAddr::from(([127, 0, 0, 1], 9));
        let body = root(ConnectInfo(peer)).await;
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn corpus_route_redacts_every_surface() {
        let (base, _processor) = boot().await;
        let client = reqwest::Client::builder().build().unwrap();
        let body = webhook_body();
        let signature = sign(body.as_bytes()).unwrap();
        let status = client
            .post(format!("{base}/webhooks/acme"))
            .header("x-oxray-signature", signature)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .unwrap()
            .status();
        assert_eq!(status, 500);
        let page = captures(&client, &base).await;
        let latest = &page["items"][0];
        assert_eq!(latest["status"], 500);
        assert_eq!(latest["flags"]["n_plus_one"], true);
        let values = corpus_values().unwrap();
        let json = format!(
            "{{\"password\":\"{}\",\"note\":\"{}\",\"clabe\":\"{}\"}}",
            values[3], values[1], values[2]
        );
        client
            .post(format!(
                "{base}/corpus?card={}",
                values[0].replace(' ', "%20")
            ))
            .header("authorization", &values[4])
            .header("content-type", "application/json")
            .body(json)
            .send()
            .await
            .unwrap();
        client
            .post(format!("{base}/corpus"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("secret={}", values[3]))
            .send()
            .await
            .unwrap();
        let page = captures(&client, &base).await;
        let mut artifacts = page.to_string();
        for item in page["items"].as_array().unwrap() {
            artifacts.push_str(
                &capture(&client, &base, item["id"].as_str().unwrap())
                    .await
                    .to_string(),
            );
        }
        let digits = digits_only(&artifacts);
        for value in &values {
            assert!(!artifacts.contains(value), "{value}");
            let secret_digits: String = value.chars().filter(|ch| ch.is_ascii_digit()).collect();
            if secret_digits.len() >= 13 {
                assert!(!digits.contains(&secret_digits), "{secret_digits}");
            }
        }
    }

    #[tokio::test]
    async fn processor_fails_every_third_call() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = ProcessorState {
            calls: std::sync::Arc::new(AtomicU32::new(0)),
        };
        let app = Router::new()
            .route("/charge", post(charge))
            .with_state(state);
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let client = reqwest::Client::builder().build().unwrap();
        let mut statuses = Vec::new();
        for _ in 0..3 {
            let status = client
                .post(format!("http://{addr}/charge"))
                .send()
                .await
                .unwrap()
                .status()
                .as_u16();
            statuses.push(status);
        }
        assert_eq!(statuses, vec![200, 200, 502]);
    }

    async fn boot() -> (String, String) {
        let processor = spawn_processor().await.unwrap();
        let oxray = Oxray::builder().build().unwrap();
        let state = app_state(&oxray, &processor).await.unwrap();
        let _ = install_tracing(&oxray);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = router(oxray, state).into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://{addr}"), processor)
    }

    async fn captures(client: &reqwest::Client, base: &str) -> serde_json::Value {
        let body = client
            .get(format!("{base}/_oxray/api/captures"))
            .header("host", "127.0.0.1")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        serde_json::from_str(&body).unwrap()
    }

    async fn capture(client: &reqwest::Client, base: &str, id: &str) -> serde_json::Value {
        let body = client
            .get(format!("{base}/_oxray/api/captures/{id}"))
            .header("host", "127.0.0.1")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        serde_json::from_str(&body).unwrap()
    }

    fn digits_only(input: &str) -> String {
        input.chars().filter(|ch| ch.is_ascii_digit()).collect()
    }
}
