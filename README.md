# mihari

A lightweight, async log collection and transport library for Rust.

Mihari batches log entries and ships them to an HTTP ingestion API with gzip compression, automatic retries, and graceful shutdown. It integrates with the `tracing` ecosystem via a subscriber layer.

## Features

- **Batched transport** -- entries are buffered and flushed in configurable batches (default: 10)
- **Automatic flush interval** -- buffered entries are shipped every N seconds (default: 5)
- **Gzip compression** -- request bodies are compressed with `flate2` (opt-out available)
- **Retry with exponential backoff** -- failed requests are retried up to N times (default: 3)
- **Graceful shutdown** -- `shutdown().await` flushes remaining entries before stopping
- **`tracing` integration** -- `MihariLayer` forwards tracing events to the API
- **Auto-captured metadata** -- hostname, PID, and Rust version are attached automatically
- **Thread-safe** -- `Mihari` is `Clone + Send + Sync`; clones share one transport
- **Builder pattern** -- fluent configuration API with sensible defaults

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
mihari = "0.1"
```

## Quick start

### Direct API

```rust
use mihari::Mihari;

#[tokio::main]
async fn main() {
    let client = Mihari::builder("your-api-token")
        .endpoint("https://api.mihari.io/v1/logs")
        .with_meta("service", "my-app")
        .with_meta("env", "production")
        .build();

    client.info("application started").await;
    client.warn("disk usage above 80%").await;
    client.error("connection to database lost").await;

    // Custom entry with extra fields
    let entry = mihari::LogEntry::new(mihari::LogLevel::Info, "user login")
        .with_meta("user_id", "usr_42")
        .with_meta("ip", "203.0.113.42");
    client.send(entry);

    client.shutdown().await;
}
```

### With the `tracing` ecosystem

```rust
use mihari::{Mihari, MihariLayer};
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() {
    let client = Mihari::builder("your-api-token").build();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(MihariLayer::new(client.clone()))
        .init();

    tracing::info!(user = "alice", "login succeeded");
    tracing::warn!(latency_ms = 1200, "slow query");

    client.shutdown().await;
}
```

## Configuration

| Method            | Default                              | Description                                |
|-------------------|--------------------------------------|--------------------------------------------|
| `endpoint()`      | `https://api.mihari.io/v1/logs`      | Ingestion API URL                          |
| `batch_size()`    | `10`                                 | Entries buffered before auto-flush         |
| `flush_interval()`| `5s`                                 | Time between periodic flushes              |
| `max_retries()`   | `3`                                  | Retry attempts for failed requests         |
| `timeout()`       | `30s`                                | HTTP request timeout                       |
| `gzip()`          | `true`                               | Enable gzip compression                   |
| `with_meta(k, v)` | --                                   | Default metadata on every entry            |

## API protocol

Entries are POST-ed as a JSON array:

```
POST /v1/logs
Authorization: Bearer <token>
Content-Type: application/json
Content-Encoding: gzip

[
  {
    "dt": "2024-01-15T10:30:00.000Z",
    "level": "info",
    "message": "request handled",
    "_hostname": "web-01",
    "_pid": 12345,
    "service": "my-app"
  }
]
```

Expected response (202):

```json
{ "status": "accepted", "count": 1 }
```

## Log levels

| Level   | Method       | Description                      |
|---------|-------------|----------------------------------|
| `debug` | `.debug()`  | Verbose diagnostic information   |
| `info`  | `.info()`   | General operational events       |
| `warn`  | `.warn()`   | Potential issues                 |
| `error` | `.error()`  | Errors that need attention       |
| `fatal` | `.fatal()`  | Unrecoverable failures           |

## Running examples

```bash
cargo run --example basic
cargo run --example with_tracing
```

## Running tests

```bash
cargo test
```

## License

MIT -- see [LICENSE](LICENSE).
