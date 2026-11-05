# Oxray

An X-ray for every request in your Axum app.

This README is a draft for review. Oxray is not published.

## Development

Rust 1.96.0 comes from `rust-toolchain.toml`. Node is 24. Visual checks need Docker.

```sh
cargo xtask verify
```

`format_duration` renders a `std::time::Duration` for the interface: `74 µs`, `1.2 ms`, `812 ms`, `1.24 s`, `1 min 4 s`.
