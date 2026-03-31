use crate::config::{Config, ConfigBuilder};
use crate::entry::{LogEntry, LogLevel};
use crate::transport::Transport;

use std::sync::Arc;

/// Auto-captured environment metadata keys.
const META_HOSTNAME: &str = "_hostname";
const META_PID: &str = "_pid";
const META_RUST_VERSION: &str = "_rust_version";

/// The main mihari client.
///
/// Thread-safe (`Clone` is cheap — all clones share the same transport).
///
/// # Example
/// ```rust,no_run
/// # async fn example() {
/// use mihari::Mihari;
///
/// let client = Mihari::builder("my-token").build();
/// client.info("server started").await;
/// client.shutdown().await;
/// # }
/// ```
#[derive(Clone)]
pub struct Mihari {
    inner: Arc<MihariInner>,
}

struct MihariInner {
    config: Config,
    transport: Transport,
    hostname: String,
    pid: u32,
}

impl Mihari {
    /// Start building a client with the given bearer token.
    pub fn builder(token: impl Into<String>) -> MihariBuilder {
        MihariBuilder {
            config_builder: ConfigBuilder::new(token),
        }
    }

    /// Build directly from a [`Config`].
    pub fn from_config(config: Config) -> Self {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown".to_string());
        let pid = std::process::id();
        let transport = Transport::spawn(config.clone());

        Self {
            inner: Arc::new(MihariInner {
                config,
                transport,
                hostname,
                pid,
            }),
        }
    }

    // ── Convenience log methods ─────────────────────────────────────

    /// Log at DEBUG level.
    pub async fn debug(&self, message: impl Into<String>) {
        self.log(LogLevel::Debug, message).await;
    }

    /// Log at INFO level.
    pub async fn info(&self, message: impl Into<String>) {
        self.log(LogLevel::Info, message).await;
    }

    /// Log at WARN level.
    pub async fn warn(&self, message: impl Into<String>) {
        self.log(LogLevel::Warn, message).await;
    }

    /// Log at ERROR level.
    pub async fn error(&self, message: impl Into<String>) {
        self.log(LogLevel::Error, message).await;
    }

    /// Log at FATAL level.
    pub async fn fatal(&self, message: impl Into<String>) {
        self.log(LogLevel::Fatal, message).await;
    }

    /// Log an entry at the given level.
    pub async fn log(&self, level: LogLevel, message: impl Into<String>) {
        let entry = self.build_entry(level, message.into());
        self.send(entry);
    }

    /// Send a fully constructed [`LogEntry`].
    pub fn send(&self, entry: LogEntry) {
        self.inner.transport.send(entry);
    }

    /// Force an immediate flush of buffered entries.
    pub async fn flush(&self) {
        self.inner.transport.flush().await;
    }

    /// Gracefully shut down: flush all remaining entries, then stop the
    /// background transport task.
    pub async fn shutdown(&self) {
        self.inner.transport.shutdown().await;
    }

    /// Access the client configuration.
    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    // ── Internal helpers ────────────────────────────────────────────

    fn build_entry(&self, level: LogLevel, message: String) -> LogEntry {
        let mut entry = LogEntry::new(level, message)
            .with_meta(META_HOSTNAME, self.inner.hostname.clone())
            .with_meta(META_PID, self.inner.pid)
            .with_meta(META_RUST_VERSION, option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown"));

        // Attach default metadata from config.
        for (key, value) in &self.inner.config.default_metadata {
            entry.metadata.insert(key.clone(), value.clone());
        }

        entry
    }
}

impl Drop for Mihari {
    fn drop(&mut self) {
        // If this is the last Arc reference, attempt a best-effort sync flush.
        if Arc::strong_count(&self.inner) == 1 {
            // We cannot do async work inside Drop, but we can send a
            // shutdown command which the transport will process.
            self.inner.transport.request_shutdown();
        }
    }
}

/// Fluent builder that wraps [`ConfigBuilder`] and produces a [`Mihari`] client.
pub struct MihariBuilder {
    config_builder: ConfigBuilder,
}

impl MihariBuilder {
    /// Override the ingestion endpoint URL.
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config_builder = self.config_builder.endpoint(endpoint);
        self
    }

    /// Set batch size.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.config_builder = self.config_builder.batch_size(size);
        self
    }

    /// Set flush interval.
    pub fn flush_interval(mut self, interval: std::time::Duration) -> Self {
        self.config_builder = self.config_builder.flush_interval(interval);
        self
    }

    /// Set max retries.
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.config_builder = self.config_builder.max_retries(retries);
        self
    }

    /// Set request timeout.
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config_builder = self.config_builder.timeout(timeout);
        self
    }

    /// Enable or disable gzip compression.
    pub fn gzip(mut self, enabled: bool) -> Self {
        self.config_builder = self.config_builder.gzip(enabled);
        self
    }

    /// Add a default metadata field to every log entry.
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.config_builder = self.config_builder.with_meta(key, value);
        self
    }

    /// Build and return the [`Mihari`] client.
    ///
    /// **Must be called inside a tokio runtime** because the background flush
    /// task is spawned here.
    pub fn build(self) -> Mihari {
        Mihari::from_config(self.config_builder.build())
    }
}
