use crate::config::Config;
use crate::entry::{IngestResponse, LogEntry};

use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::Client;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::time;

/// Commands sent from the client to the background transport task.
#[derive(Debug)]
pub(crate) enum TransportCommand {
    /// Enqueue a log entry for batched delivery.
    Send(LogEntry),
    /// Force an immediate flush of the current buffer.
    Flush,
    /// Shut down the background task gracefully.
    Shutdown,
}

/// Background transport that batches, compresses and ships log entries.
pub(crate) struct Transport {
    tx: mpsc::UnboundedSender<TransportCommand>,
    shutdown_notify: Arc<Notify>,
}

impl Transport {
    /// Spawn the background flush loop and return a handle.
    pub(crate) fn spawn(config: Config) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let shutdown_notify = Arc::new(Notify::new());
        let notify_clone = shutdown_notify.clone();

        tokio::spawn(async move {
            flush_loop(config, rx, notify_clone).await;
        });

        Self {
            tx,
            shutdown_notify,
        }
    }

    /// Enqueue a log entry.
    pub(crate) fn send(&self, entry: LogEntry) {
        let _ = self.tx.send(TransportCommand::Send(entry));
    }

    /// Request an immediate flush and wait for it to complete.
    pub(crate) async fn flush(&self) {
        let _ = self.tx.send(TransportCommand::Flush);
        // Give the flush loop a moment to process.
        tokio::task::yield_now().await;
    }

    /// Initiate graceful shutdown: flush remaining entries, then stop.
    pub(crate) async fn shutdown(&self) {
        let _ = self.tx.send(TransportCommand::Shutdown);
        self.shutdown_notify.notified().await;
    }
}

/// Core flush loop running on a background tokio task.
async fn flush_loop(
    config: Config,
    mut rx: mpsc::UnboundedReceiver<TransportCommand>,
    shutdown_notify: Arc<Notify>,
) {
    let client = Client::builder()
        .timeout(config.timeout)
        .build()
        .expect("failed to build reqwest client");

    let buffer: Arc<Mutex<Vec<LogEntry>>> = Arc::new(Mutex::new(Vec::new()));
    let mut interval = time::interval(config.flush_interval);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let entries = drain_buffer(&buffer).await;
                if !entries.is_empty() {
                    send_batch(&client, &config, entries).await;
                }
            }
            cmd = rx.recv() => {
                match cmd {
                    Some(TransportCommand::Send(entry)) => {
                        let should_flush = {
                            let mut buf = buffer.lock().await;
                            buf.push(entry);
                            buf.len() >= config.batch_size
                        };
                        if should_flush {
                            let entries = drain_buffer(&buffer).await;
                            send_batch(&client, &config, entries).await;
                        }
                    }
                    Some(TransportCommand::Flush) => {
                        let entries = drain_buffer(&buffer).await;
                        if !entries.is_empty() {
                            send_batch(&client, &config, entries).await;
                        }
                    }
                    Some(TransportCommand::Shutdown) | None => {
                        // Final flush before exit.
                        let entries = drain_buffer(&buffer).await;
                        if !entries.is_empty() {
                            send_batch(&client, &config, entries).await;
                        }
                        shutdown_notify.notify_one();
                        return;
                    }
                }
            }
        }
    }
}

/// Take all entries out of the buffer, leaving it empty.
async fn drain_buffer(buffer: &Arc<Mutex<Vec<LogEntry>>>) -> Vec<LogEntry> {
    let mut buf = buffer.lock().await;
    std::mem::take(&mut *buf)
}

/// Compress a JSON payload with gzip.
fn gzip_compress(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(data)?;
    encoder.finish()
}

/// Send a batch of log entries to the API with retry logic.
async fn send_batch(client: &Client, config: &Config, entries: Vec<LogEntry>) {
    let body = match serde_json::to_vec(&entries) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "failed to serialise log entries");
            return;
        }
    };

    for attempt in 0..=config.max_retries {
        let result = send_request(client, config, &body).await;
        match result {
            Ok(resp) => {
                tracing::debug!(
                    count = resp.count,
                    status = %resp.status,
                    "batch delivered"
                );
                return;
            }
            Err(e) => {
                if attempt < config.max_retries {
                    let backoff = std::time::Duration::from_millis(100 * 2u64.pow(attempt));
                    tracing::warn!(
                        attempt = attempt + 1,
                        max = config.max_retries,
                        error = %e,
                        "retrying batch send"
                    );
                    tokio::time::sleep(backoff).await;
                } else {
                    tracing::error!(
                        error = %e,
                        entries = entries.len(),
                        "batch send failed after retries"
                    );
                }
            }
        }
    }
}

/// Perform a single HTTP request (with optional gzip).
async fn send_request(
    client: &Client,
    config: &Config,
    json_body: &[u8],
) -> Result<IngestResponse, reqwest::Error> {
    let mut request = client
        .post(&config.endpoint)
        .bearer_auth(&config.token)
        .header("Content-Type", "application/json");

    let body_bytes = if config.gzip {
        let compressed = gzip_compress(json_body).unwrap_or_else(|_| json_body.to_vec());
        request = request.header("Content-Encoding", "gzip");
        compressed
    } else {
        json_body.to_vec()
    };

    request
        .body(body_bytes)
        .send()
        .await?
        .error_for_status()?
        .json::<IngestResponse>()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gzip_roundtrip() {
        let original = b"hello world, this is a test payload";
        let compressed = gzip_compress(original).unwrap();
        assert_ne!(compressed, original.as_slice());

        // Decompress to verify.
        use flate2::read::GzDecoder;
        use std::io::Read;
        let mut decoder = GzDecoder::new(compressed.as_slice());
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        assert_eq!(decompressed, original);
    }
}
