use std::time::Duration;

/// Default API endpoint.
const DEFAULT_ENDPOINT: &str = "https://api.mihari.io/v1/logs";
/// Default batch size before auto-flush.
const DEFAULT_BATCH_SIZE: usize = 10;
/// Default flush interval.
const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_secs(5);
/// Default maximum retry attempts.
const DEFAULT_MAX_RETRIES: u32 = 3;
/// Default request timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Configuration for a [`crate::Mihari`] client.
#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) token: String,
    pub(crate) endpoint: String,
    pub(crate) batch_size: usize,
    pub(crate) flush_interval: Duration,
    pub(crate) max_retries: u32,
    pub(crate) timeout: Duration,
    pub(crate) gzip: bool,
    pub(crate) default_metadata: Vec<(String, serde_json::Value)>,
}

/// Fluent builder for [`Config`].
///
/// # Example
/// ```rust,no_run
/// use mihari::ConfigBuilder;
///
/// let config = ConfigBuilder::new("my-token")
///     .endpoint("https://custom.endpoint/v1/logs")
///     .batch_size(20)
///     .build();
/// ```
#[derive(Debug)]
pub struct ConfigBuilder {
    token: String,
    endpoint: String,
    batch_size: usize,
    flush_interval: Duration,
    max_retries: u32,
    timeout: Duration,
    gzip: bool,
    default_metadata: Vec<(String, serde_json::Value)>,
}

impl ConfigBuilder {
    /// Start building a config with the required API bearer token.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            batch_size: DEFAULT_BATCH_SIZE,
            flush_interval: DEFAULT_FLUSH_INTERVAL,
            max_retries: DEFAULT_MAX_RETRIES,
            timeout: DEFAULT_TIMEOUT,
            gzip: true,
            default_metadata: Vec::new(),
        }
    }

    /// Override the ingestion endpoint URL.
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Number of entries to buffer before triggering a flush (default: 10).
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Time between automatic flushes (default: 5 s).
    pub fn flush_interval(mut self, interval: Duration) -> Self {
        self.flush_interval = interval;
        self
    }

    /// Maximum retry attempts for failed HTTP requests (default: 3).
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// HTTP request timeout (default: 30 s).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Enable or disable gzip compression (default: enabled).
    pub fn gzip(mut self, enabled: bool) -> Self {
        self.gzip = enabled;
        self
    }

    /// Attach a default metadata field to every log entry.
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.default_metadata.push((key.into(), value.into()));
        self
    }

    /// Consume the builder and produce a [`Config`].
    pub fn build(self) -> Config {
        Config {
            token: self.token,
            endpoint: self.endpoint,
            batch_size: self.batch_size,
            flush_interval: self.flush_interval,
            max_retries: self.max_retries,
            timeout: self.timeout,
            gzip: self.gzip,
            default_metadata: self.default_metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults() {
        let cfg = ConfigBuilder::new("tok_abc").build();
        assert_eq!(cfg.token, "tok_abc");
        assert_eq!(cfg.endpoint, DEFAULT_ENDPOINT);
        assert_eq!(cfg.batch_size, DEFAULT_BATCH_SIZE);
        assert_eq!(cfg.flush_interval, DEFAULT_FLUSH_INTERVAL);
        assert_eq!(cfg.max_retries, DEFAULT_MAX_RETRIES);
        assert!(cfg.gzip);
    }

    #[test]
    fn builder_overrides() {
        let cfg = ConfigBuilder::new("tok")
            .endpoint("http://localhost:8080")
            .batch_size(50)
            .gzip(false)
            .max_retries(1)
            .with_meta("env", "staging")
            .build();

        assert_eq!(cfg.endpoint, "http://localhost:8080");
        assert_eq!(cfg.batch_size, 50);
        assert!(!cfg.gzip);
        assert_eq!(cfg.max_retries, 1);
        assert_eq!(cfg.default_metadata.len(), 1);
    }
}
