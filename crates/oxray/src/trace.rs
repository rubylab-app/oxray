//! Tracing layer that attaches logs, SQL, and outgoing calls to a request.

use serde_json::{Map, Value};
use tracing::field::{Field, Visit};
use tracing::{Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use crate::capture::{LogEntry, QueryEntry};
use crate::collect::{self, SharedCollector};
use crate::oxray::{Mode, Oxray};
use crate::sql::fingerprint;

/// Tracing subscriber layer that records events inside `oxray.request`.
#[derive(Clone)]
pub struct OxrayTracingLayer {
    oxray: Oxray,
}

impl OxrayTracingLayer {
    pub(crate) fn new(oxray: Oxray) -> Self {
        Self { oxray }
    }
}

struct Attached(SharedCollector);

impl<S> Layer<S> for OxrayTracingLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn enabled(&self, metadata: &tracing::Metadata<'_>, _ctx: Context<'_, S>) -> bool {
        self.oxray.mode() == Mode::Dev && wants(metadata)
    }

    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        if attrs.metadata().name() != "oxray.request" {
            return;
        }
        let mut request_id = RequestId(None);
        attrs.record(&mut request_id);
        let Some(request_id) = request_id.0 else {
            return;
        };
        let Some(collector) = self.oxray.collector(&request_id) else {
            return;
        };
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Attached(collector));
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        let Some(collector) = collector_for(event, &ctx) else {
            return;
        };
        let target = event.metadata().target();
        if target == "oxray::outgoing" || target.starts_with("oxray::outgoing::") {
            let mut key = KeyVisitor(None);
            event.record(&mut key);
            if let Some(key) = key.0
                && let Some(call) = self.oxray.take_outgoing(key)
            {
                if let Ok(mut guard) = call.lock() {
                    guard.offset_us = collector.offset();
                }
                collect::push_outgoing(&collector, call);
            }
            return;
        }
        let mut fields = FieldVisitor::default();
        event.record(&mut fields);
        let offset = collector.offset();
        if target == "sqlx::query" || target.starts_with("sqlx::query::") {
            if let Some(entry) = query_entry(&fields, offset) {
                collect::push_query(&collector, entry);
            }
            return;
        }
        let level = level_name(event.metadata().level());
        let message = fields.message.clone().unwrap_or_default();
        fields.values.remove("message");
        collect::push_log(
            &collector,
            LogEntry {
                offset_us: offset,
                level: level.to_owned(),
                target: target.to_owned(),
                message,
                fields: fields.values,
            },
            level == "ERROR",
        );
    }
}

fn collector_for<S>(event: &tracing::Event<'_>, ctx: &Context<'_, S>) -> Option<SharedCollector>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    let mut scope = ctx.event_scope(event)?;
    scope.find_map(|span| {
        span.extensions()
            .get::<Attached>()
            .map(|attached| attached.0.clone())
    })
}

fn wants(metadata: &tracing::Metadata<'_>) -> bool {
    let target = metadata.target();
    if target == "oxray" || target == "oxray::outgoing" || target.starts_with("oxray::outgoing::") {
        return *metadata.level() <= Level::TRACE;
    }
    if target.starts_with("oxray::") {
        return false;
    }
    for prefix in ["hyper", "h2", "tokio", "tower", "rustls", "reqwest"] {
        if target == prefix || target.starts_with(&format!("{prefix}::")) {
            return false;
        }
    }
    *metadata.level() <= Level::DEBUG
}

fn level_name(level: &Level) -> &'static str {
    match *level {
        Level::ERROR => "ERROR",
        Level::WARN => "WARN",
        Level::INFO => "INFO",
        Level::DEBUG => "DEBUG",
        Level::TRACE => "TRACE",
    }
}

fn query_entry(fields: &FieldVisitor, event_offset: u64) -> Option<QueryEntry> {
    let statement = text_field(fields, "db.statement");
    let summary = text_field(fields, "summary");
    let sql = if statement.is_empty() {
        summary
    } else {
        statement
    };
    if sql.is_empty() {
        return None;
    }
    let elapsed_secs = fields
        .values
        .get("elapsed_secs")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let duration_us = elapsed_secs.mul_add(1_000_000.0, 0.0).round().max(0.0) as u64;
    Some(QueryEntry {
        offset_us: event_offset.saturating_sub(duration_us),
        duration_us,
        fingerprint: fingerprint(&sql),
        sql,
        rows_affected: json_u64(fields.values.get("rows_affected")),
        rows_returned: json_u64(fields.values.get("rows_returned")),
    })
}

fn text_field(fields: &FieldVisitor, name: &str) -> String {
    fields
        .values
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned()
}

fn json_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(|item| {
        item.as_u64()
            .or_else(|| item.as_i64().and_then(|number| u64::try_from(number).ok()))
            .or_else(|| item.as_f64().map(|number| number.max(0.0) as u64))
    })
}

struct RequestId(Option<String>);

impl Visit for RequestId {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "id" && self.0.is_none() {
            self.0 = Some(format!("{value:?}"));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "id" {
            self.0 = Some(value.to_owned());
        }
    }
}

struct KeyVisitor(Option<u64>);

impl Visit for KeyVisitor {
    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "key" {
            self.0 = Some(value);
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if field.name() == "key"
            && let Ok(value) = u64::try_from(value)
        {
            self.0 = Some(value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "key" && self.0.is_none() {
            let text = format!("{value:?}");
            if let Ok(parsed) = text.parse() {
                self.0 = Some(parsed);
            }
        }
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: Option<String>,
    values: Map<String, Value>,
}

impl Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.store(field, Value::String(value.to_owned()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if self.values.contains_key(field.name()) {
            return;
        }
        self.store(field, Value::String(format!("{value:?}")));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.store(field, Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.store(field, Value::from(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.store(field, Value::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.store(field, Value::from(value));
    }
}

impl FieldVisitor {
    fn store(&mut self, field: &Field, value: Value) {
        if field.name() == "message"
            && let Value::String(text) = &value
        {
            self.message = Some(text.clone());
        }
        self.values.insert(field.name().to_owned(), value);
    }
}

/// Warns once when a global filter hides the request span or sqlx events.
pub(crate) fn warn_if_global_filter_blocks(oxray: &Oxray) {
    if !tracing::dispatcher::has_been_set() || !oxray.claim_filter_warn() {
        return;
    }
    let span_enabled = tracing::enabled!(target: "oxray", tracing::Level::INFO);
    let queries_enabled = tracing::enabled!(target: "sqlx::query", tracing::Level::DEBUG);
    if span_enabled && queries_enabled {
        return;
    }
    tracing::warn!(
        target: "oxray",
        "Oxray's span or sqlx events are disabled by a global filter; add oxray=trace,sqlx::query=debug"
    );
}
