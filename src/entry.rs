use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
            LogLevel::Fatal => "fatal",
        };
        f.write_str(s)
    }
}

impl From<tracing::Level> for LogLevel {
    fn from(level: tracing::Level) -> Self {
        match level {
            tracing::Level::TRACE | tracing::Level::DEBUG => LogLevel::Debug,
            tracing::Level::INFO => LogLevel::Info,
            tracing::Level::WARN => LogLevel::Warn,
            tracing::Level::ERROR => LogLevel::Error,
        }
    }
}

/// A single log entry to be sent to the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// ISO 8601 timestamp.
    pub dt: DateTime<Utc>,
    /// Log severity level.
    pub level: LogLevel,
    /// Log message body.
    pub message: String,
    /// Additional metadata key-value pairs (flattened into the JSON object).
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl LogEntry {
    /// Create a new log entry with the given level and message, timestamped to now.
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            dt: Utc::now(),
            level,
            message: message.into(),
            metadata: HashMap::new(),
        }
    }

    /// Attach a metadata key-value pair to this entry.
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Response returned by the log ingestion API on success.
#[derive(Debug, Clone, Deserialize)]
pub struct IngestResponse {
    pub status: String,
    pub count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_entry_serialises_correctly() {
        let entry = LogEntry::new(LogLevel::Info, "hello world")
            .with_meta("service", "test-svc");

        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["level"], "info");
        assert_eq!(json["message"], "hello world");
        assert_eq!(json["service"], "test-svc");
        assert!(json["dt"].is_string());
    }

    #[test]
    fn log_level_display() {
        assert_eq!(LogLevel::Debug.to_string(), "debug");
        assert_eq!(LogLevel::Fatal.to_string(), "fatal");
    }

    #[test]
    fn log_level_from_tracing() {
        assert_eq!(LogLevel::from(tracing::Level::INFO), LogLevel::Info);
        assert_eq!(LogLevel::from(tracing::Level::ERROR), LogLevel::Error);
        assert_eq!(LogLevel::from(tracing::Level::TRACE), LogLevel::Debug);
    }
}
