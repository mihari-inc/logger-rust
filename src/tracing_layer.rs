use crate::client::Mihari;
use crate::entry::{LogEntry, LogLevel};

use tracing::field::{Field, Visit};
use tracing::span;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// A [`tracing_subscriber::Layer`] that forwards tracing events to the mihari
/// log ingestion API.
///
/// # Example
/// ```rust,no_run
/// use mihari_logger::{Mihari, MihariLayer};
/// use tracing_subscriber::prelude::*;
///
/// # async fn example() {
/// let client = Mihari::builder("my-token").build();
///
/// tracing_subscriber::registry()
///     .with(MihariLayer::new(client.clone()))
///     .init();
///
/// tracing::info!(user = "alice", "login succeeded");
/// client.shutdown().await;
/// # }
/// ```
pub struct MihariLayer {
    client: Mihari,
}

impl MihariLayer {
    /// Create a new layer backed by the given mihari client.
    pub fn new(client: Mihari) -> Self {
        Self { client }
    }
}

impl<S> Layer<S> for MihariLayer
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let level = LogLevel::from(*event.metadata().level());

        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        let message = visitor
            .message
            .unwrap_or_else(|| String::from("<no message>"));

        let mut entry = LogEntry::new(level, message);

        // Copy span/event fields as metadata.
        for (key, value) in visitor.fields {
            entry.metadata.insert(key, value);
        }

        // Record the tracing target (module path).
        entry.metadata.insert(
            "target".to_string(),
            serde_json::Value::String(event.metadata().target().to_string()),
        );

        self.client.send(entry);
    }

    fn on_new_span(&self, _attrs: &span::Attributes<'_>, _id: &span::Id, _ctx: Context<'_, S>) {
        // Span tracking is intentionally a no-op; we only forward events.
    }
}

/// Visitor that extracts structured fields from a tracing event.
#[derive(Default)]
struct FieldVisitor {
    message: Option<String>,
    fields: Vec<(String, serde_json::Value)>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let val = format!("{:?}", value);
        if field.name() == "message" {
            self.message = Some(val);
        } else {
            self.fields
                .push((field.name().to_string(), serde_json::Value::String(val)));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.push((
                field.name().to_string(),
                serde_json::Value::String(value.to_string()),
            ));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.push((
            field.name().to_string(),
            serde_json::json!(value),
        ));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.push((
            field.name().to_string(),
            serde_json::json!(value),
        ));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields.push((
            field.name().to_string(),
            serde_json::json!(value),
        ));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.push((
            field.name().to_string(),
            serde_json::json!(value),
        ));
    }
}
