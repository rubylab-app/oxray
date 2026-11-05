# Oxray

An X-ray for every request in your Axum app.

This README is a draft for review. Oxray is not published.

## Development

Rust 1.96.0 comes from `rust-toolchain.toml`. Node is 24. Visual checks need Docker.

```sh
cargo xtask verify
```

`format_duration` renders a `std::time::Duration` for the interface: `74 µs`, `1.2 ms`, `812 ms`, `1.24 s`, `1 min 4 s`.

## Capture

Debug builds record requests (`Mode::Dev`). `OXRAY=off` or a release profile leaves the handle disabled. The mounted routes stay loopback-only in Dev and answer 404 when disabled.

```rust
let oxray = oxray::Oxray::builder().build()?;
let app = axum::Router::new()
    .merge(oxray.router())
    .layer(oxray.layer());
```

Serve the app with `into_make_service_with_connect_info::<SocketAddr>()`. The default mount is `/_oxray`. `GET /api/captures`, `GET /api/captures/{id}`, and `GET /api/meta` live under that mount. Responses can include `X-Oxray-Id`.
