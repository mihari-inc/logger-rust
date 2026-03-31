use mihari::Mihari;

#[tokio::main]
async fn main() {
    let client = Mihari::builder("your-api-token-here")
        .endpoint("https://api.mihari.io/v1/logs")
        .with_meta("service", "my-app")
        .with_meta("env", "production")
        .build();

    client.info("application started").await;
    client.debug("loading configuration").await;
    client.warn("cache miss ratio above threshold").await;
    client.error("failed to connect to payment gateway").await;

    // Custom log entry with extra fields.
    let entry = mihari::LogEntry::new(mihari::LogLevel::Info, "user signed in")
        .with_meta("user_id", "usr_42")
        .with_meta("ip", "203.0.113.42");
    client.send(entry);

    // Ensure all logs are delivered before exiting.
    client.shutdown().await;

    println!("All logs sent.");
}
