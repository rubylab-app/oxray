//! A global INFO filter produces the D-017 warning.

#[path = "common/mod.rs"]
mod support;

use std::sync::{Arc, Mutex};

use axum::body::Body;
use oxray::Oxray;
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;

#[tokio::test]
async fn global_info_filter_warns_once() {
    let oxray = Oxray::builder().build().unwrap();
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let layer = Recall(Arc::clone(&seen));
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(oxray.tracing_layer())
        .with(layer)
        .try_init();
    let _ = support::call(
        support::stack(oxray, true),
        support::request("GET", "/only-get", "127.0.0.1", Body::empty()),
    )
    .await;
    let seen = seen.lock().unwrap();
    assert!(
        seen.iter()
            .any(|message| message.contains("oxray=trace,sqlx::query=debug")),
        "{seen:?}"
    );
}

struct Recall(Arc<Mutex<Vec<String>>>);

impl<S> Layer<S> for Recall
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        if *event.metadata().level() != tracing::Level::WARN {
            return;
        }
        let mut message = Message(String::new());
        event.record(&mut message);
        if let Ok(mut guard) = self.0.lock() {
            guard.push(message.0);
        }
    }
}

struct Message(String);

impl tracing::field::Visit for Message {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0 = value.to_owned();
        }
    }
}
