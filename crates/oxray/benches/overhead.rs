//! Local overhead samples for `cargo xtask bench`.

use std::io::Write;

use axum::body::Bytes;
use axum::routing::post;
use axum::{Router, body::Body};
use http::Request;
use tower::ServiceExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let case = std::env::var("OXRAY_BENCH").unwrap_or_else(|_| "baseline".to_owned());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let median = runtime.block_on(sample(&case))?;
    writeln!(std::io::stdout(), "median_ns {median}")?;
    Ok(())
}

async fn sample(case: &str) -> Result<u128, Box<dyn std::error::Error>> {
    let app = match case {
        "baseline" => Router::new().route("/echo", post(echo)),
        "disabled" | "dev" => {
            let oxray = oxray::Oxray::builder().build()?;
            Router::new()
                .route("/echo", post(echo))
                .merge(oxray.router())
                .layer(oxray.layer())
        }
        other => return Err(std::io::Error::other(format!("unknown case {other}")).into()),
    };
    let mut samples = Vec::with_capacity(100);
    for index in 0..120 {
        let started = std::time::Instant::now();
        let response = match app.clone().oneshot(request()?).await {
            Ok(response) => response,
            Err(error) => match error {},
        };
        let _ = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
        if index >= 20 {
            samples.push(started.elapsed().as_nanos());
        }
    }
    samples.sort_unstable();
    Ok(samples[samples.len() / 2])
}

async fn echo(body: Bytes) -> Bytes {
    for index in 0..10 {
        tracing::info!(target: "bench", index, "tick");
    }
    body
}

fn request() -> Result<Request<Body>, http::Error> {
    Request::builder()
        .method("POST")
        .uri("/echo")
        .body(Body::from(vec![1_u8; 1024]))
}
