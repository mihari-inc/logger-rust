//! # mihari-logger
//!
//! A lightweight, async log collection and transport library for Rust.
//!
//! Mihari batches log entries and ships them to an HTTP ingestion API with
//! gzip compression, automatic retries, and graceful shutdown.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use mihari_logger::Mihari;
//!
//! #[tokio::main]
//! async fn main() {
//!     let client = Mihari::builder("my-api-token")
//!         .endpoint("https://api.example.com/v1/logs")
//!         .build();
//!
//!     client.info("application started").await;
//!     client.warn("disk usage above 80%").await;
//!
//!     client.shutdown().await;
//! }
//! ```
//!
//! ## With the `tracing` ecosystem
//!
//! ```rust,no_run
//! use mihari_logger::{Mihari, MihariLayer};
//! use tracing_subscriber::prelude::*;
//!
//! #[tokio::main]
//! async fn main() {
//!     let client = Mihari::builder("my-api-token").build();
//!
//!     tracing_subscriber::registry()
//!         .with(MihariLayer::new(client.clone()))
//!         .init();
//!
//!     tracing::info!(user = "alice", "login succeeded");
//!
//!     client.shutdown().await;
//! }
//! ```

mod client;
mod config;
mod entry;
mod tracing_layer;
mod transport;

// ── Public API re-exports ───────────────────────────────────────────
pub use client::{Mihari, MihariBuilder};
pub use config::{Config, ConfigBuilder};
pub use entry::{IngestResponse, LogEntry, LogLevel};
pub use tracing_layer::MihariLayer;
