use mihari_logger::{LogEntry, LogLevel, Mihari, MihariLayer};
use std::time::Duration;
use tracing_subscriber::prelude::*;
use wiremock::matchers::{bearer_token, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper: build a client pointed at the given mock server.
fn test_client(server: &MockServer) -> Mihari {
    Mihari::builder("test-token-xyz")
        .endpoint(format!("{}/v1/logs", server.uri()))
        .batch_size(2)
        .flush_interval(Duration::from_secs(60)) // long interval; we flush manually
        .gzip(false) // disable gzip so we can inspect raw bodies easily
        .max_retries(0)
        .build()
}

/// Mount a standard 202 mock that accepts JSON log batches.
async fn mount_accepted(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/v1/logs"))
        .and(bearer_token("test-token-xyz"))
        .and(header("Content-Type", "application/json"))
        .respond_with(ResponseTemplate::new(202).set_body_json(serde_json::json!({
            "status": "accepted",
            "count": 2
        })))
        .expect(1..)
        .mount(server)
        .await;
}

#[tokio::test]
async fn sends_batch_when_batch_size_reached() {
    let server = MockServer::start().await;
    mount_accepted(&server).await;

    let client = test_client(&server);

    // Send exactly batch_size (2) entries to trigger an automatic flush.
    client.info("first message").await;
    client.warn("second message").await;

    // Allow background task to deliver.
    tokio::time::sleep(Duration::from_millis(200)).await;
    client.shutdown().await;

    // Verify the mock received the request.
    let received = server.received_requests().await.unwrap();
    assert!(
        !received.is_empty(),
        "expected at least one request to the mock server"
    );

    // Parse the body of the first request.
    let body: Vec<serde_json::Value> = serde_json::from_slice(&received[0].body).unwrap();
    assert_eq!(body.len(), 2);
    assert_eq!(body[0]["level"], "info");
    assert_eq!(body[0]["message"], "first message");
    assert_eq!(body[1]["level"], "warn");
    assert_eq!(body[1]["message"], "second message");
}

#[tokio::test]
async fn manual_flush_sends_partial_batch() {
    let server = MockServer::start().await;
    mount_accepted(&server).await;

    let client = test_client(&server);

    client.error("only one entry").await;
    client.flush().await;

    // Give the transport a moment.
    tokio::time::sleep(Duration::from_millis(200)).await;
    client.shutdown().await;

    let received = server.received_requests().await.unwrap();
    assert!(
        !received.is_empty(),
        "flush should have triggered a request"
    );

    let body: Vec<serde_json::Value> = serde_json::from_slice(&received[0].body).unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["level"], "error");
}

#[tokio::test]
async fn log_entry_contains_auto_metadata() {
    let server = MockServer::start().await;
    mount_accepted(&server).await;

    let client = test_client(&server);
    client.debug("metadata check").await;
    client.flush().await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    client.shutdown().await;

    let received = server.received_requests().await.unwrap();
    assert!(!received.is_empty());

    let body: Vec<serde_json::Value> = serde_json::from_slice(&received[0].body).unwrap();
    let entry = &body[0];

    // Auto-captured metadata.
    assert!(entry["_hostname"].is_string());
    assert!(entry["_pid"].is_number());
    assert!(entry["dt"].is_string());
}

#[tokio::test]
async fn custom_metadata_attached() {
    let server = MockServer::start().await;
    mount_accepted(&server).await;

    let client = Mihari::builder("test-token-xyz")
        .endpoint(format!("{}/v1/logs", server.uri()))
        .batch_size(1)
        .flush_interval(Duration::from_secs(60))
        .gzip(false)
        .max_retries(0)
        .with_meta("service", "my-api")
        .with_meta("env", "test")
        .build();

    client.info("with custom meta").await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    client.shutdown().await;

    let received = server.received_requests().await.unwrap();
    let body: Vec<serde_json::Value> = serde_json::from_slice(&received[0].body).unwrap();
    assert_eq!(body[0]["service"], "my-api");
    assert_eq!(body[0]["env"], "test");
}

#[tokio::test]
async fn log_entry_serialisation() {
    let entry =
        LogEntry::new(LogLevel::Fatal, "something broke").with_meta("request_id", "abc-123");

    let json = serde_json::to_value(&entry).unwrap();
    assert_eq!(json["level"], "fatal");
    assert_eq!(json["message"], "something broke");
    assert_eq!(json["request_id"], "abc-123");
    assert!(json["dt"].is_string());
}

#[tokio::test]
async fn tracing_layer_forwards_events() {
    let server = MockServer::start().await;
    mount_accepted(&server).await;

    let client = Mihari::builder("test-token-xyz")
        .endpoint(format!("{}/v1/logs", server.uri()))
        .batch_size(1)
        .flush_interval(Duration::from_secs(60))
        .gzip(false)
        .max_retries(0)
        .build();

    // Install the layer on a non-global subscriber so we don't interfere
    // with other tests.
    let subscriber = tracing_subscriber::registry().with(MihariLayer::new(client.clone()));

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(request_id = "r-42", "handled request");
    });

    tokio::time::sleep(Duration::from_millis(300)).await;
    client.shutdown().await;

    let received = server.received_requests().await.unwrap();
    assert!(
        !received.is_empty(),
        "tracing layer should have forwarded the event"
    );

    let body: Vec<serde_json::Value> = serde_json::from_slice(&received[0].body).unwrap();
    assert_eq!(body[0]["level"], "info");
    assert!(body[0]["message"]
        .as_str()
        .unwrap()
        .contains("handled request"));
    assert_eq!(body[0]["request_id"], "r-42");
}

#[tokio::test]
async fn graceful_shutdown_flushes_remaining() {
    let server = MockServer::start().await;
    mount_accepted(&server).await;

    let client = test_client(&server);

    // Send one entry (below batch_size), then shutdown.
    client.fatal("final message").await;
    client.shutdown().await;

    let received = server.received_requests().await.unwrap();
    assert!(
        !received.is_empty(),
        "shutdown should flush remaining entries"
    );
}
