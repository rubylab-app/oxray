//! Dev-mode access guard.

use std::net::SocketAddr;

use axum::extract::ConnectInfo;
use http::Request;

const NO_CONNECT_INFO: &str = "Oxray dev mode needs ConnectInfo. Serve the app with into_make_service_with_connect_info::<SocketAddr>().";
const NOT_LOOPBACK: &str = "Oxray dev mode only accepts loopback peers.";
const FORWARDED: &str = "Oxray dev mode rejects forwarded headers.";
const BAD_HOST: &str = "Oxray dev mode only accepts Host localhost, 127.0.0.1, or [::1].";
const BAD_ORIGIN: &str = "Oxray dev mode rejects an Origin that does not match Host.";

/// Returns the plain-text 403 reason, or nothing when the request may proceed.
pub fn reject_dev<B>(request: &Request<B>) -> Option<&'static str> {
    let Some(ConnectInfo(peer)) = request.extensions().get::<ConnectInfo<SocketAddr>>() else {
        return Some(NO_CONNECT_INFO);
    };
    if !peer.ip().is_loopback() {
        return Some(NOT_LOOPBACK);
    }
    if forwarded(request) {
        return Some(FORWARDED);
    }
    let Some(host) = host_header(request) else {
        return Some(BAD_HOST);
    };
    if !loopback_host(&host) {
        return Some(BAD_HOST);
    }
    if let Some(origin) = header(request, "origin")
        && !origin_matches(&origin, &host)
    {
        return Some(BAD_ORIGIN);
    }
    None
}

fn forwarded<B>(request: &Request<B>) -> bool {
    request.headers().iter().any(|(name, _)| {
        let name = name.as_str();
        name.eq_ignore_ascii_case("forwarded")
            || name.eq_ignore_ascii_case("x-forwarded-for")
            || name.eq_ignore_ascii_case("x-real-ip")
    })
}

fn header<B>(request: &Request<B>, name: &str) -> Option<String> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn host_header<B>(request: &Request<B>) -> Option<String> {
    header(request, "host").or_else(|| {
        request
            .uri()
            .authority()
            .map(|authority| authority.as_str().to_owned())
    })
}

fn loopback_host(host: &str) -> bool {
    matches!(hostname(host), "localhost" | "127.0.0.1" | "::1")
}

fn hostname(host: &str) -> &str {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    host.split(':').next().unwrap_or(host)
}

fn origin_matches(origin: &str, host: &str) -> bool {
    let Some(origin_host) = origin.split("://").nth(1) else {
        return false;
    };
    let origin_host = origin_host.split('/').next().unwrap_or(origin_host);
    hostname(origin_host).eq_ignore_ascii_case(hostname(host))
}
