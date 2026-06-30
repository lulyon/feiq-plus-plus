//! feiq-relay — lightweight WebSocket message relay server.
//!
//! Clients join named rooms and exchange IPMSG datagrams through
//! the relay. Offline messages are queued in-memory and delivered
//! on rejoin.

pub mod server;

use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "feiq-relay", version, about = "Feiq++ relay server")]
struct Cli {
    /// Address to bind to
    #[arg(long, default_value = "0.0.0.0")]
    bind: String,

    /// TCP port for WebSocket
    #[arg(long, default_value = "2426")]
    port: u16,

    /// Offline message TTL in seconds (default: 86400 = 24h)
    #[arg(long, default_value = "86400")]
    history_ttl: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cli = Cli::parse();
    tracing::info!(
        "feiq-relay v{} starting on {}:{} (history_ttl={}s)",
        env!("CARGO_PKG_VERSION"),
        cli.bind,
        cli.port,
        cli.history_ttl,
    );

    server::run(&cli.bind, cli.port, cli.history_ttl).await?;
    Ok(())
}
