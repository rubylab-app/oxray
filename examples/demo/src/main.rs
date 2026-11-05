//! Minimal Axum app used by later Oxray milestones.
//!
//! `GET /` answers `ok` and the server is built with connection info so the
//! peer address is available to handlers.

#![forbid(unsafe_code)]

use std::io::Write;
use std::net::SocketAddr;

use axum::Router;
use axum::extract::ConnectInfo;
use axum::routing::get;
use tokio::net::TcpListener;

async fn root(ConnectInfo(_peer): ConnectInfo<SocketAddr>) -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    writeln!(std::io::stdout(), "demo listening on http://{addr}")?;
    let app = Router::new().route("/", get(root));
    axum::serve(listener, app).await
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
}
