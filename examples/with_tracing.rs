use mihari_logger::{Mihari, MihariLayer};
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() {
    let client = Mihari::builder("your-api-token-here")
        .endpoint("https://api.mihari.io/v1/logs")
        .with_meta("service", "tracing-demo")
        .build();

    // Compose a subscriber that writes to stdout AND ships to mihari.
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(MihariLayer::new(client.clone()))
        .init();

    tracing::info!("application started");
    tracing::debug!(component = "cache", "warming up cache");
    tracing::warn!(latency_ms = 1200, "slow response detected");
    tracing::error!(
        error = "connection refused",
        host = "db.internal",
        "database unreachable"
    );

    // Ensure all logs are delivered before exiting.
    client.shutdown().await;

    println!("All logs sent via tracing integration.");
}
