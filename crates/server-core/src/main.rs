use std::net::SocketAddr;
use tracing::info;
use raklib_rs::RakNetListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Starting PocketMine-Rust High Performance Bedrock Server...");
    info!("Initializing Tokio Async Network & Tick Loop Engine...");

    let bind_addr: SocketAddr = "0.0.0.0:19132".parse()?;
    let server_id: u64 = 0x12345678_aabbccdd;
    let listener = RakNetListener::bind(bind_addr, server_id).await?;

    info!("Server listening for Bedrock clients on {}", listener.local_addr()?);
    info!("Ready for parallel worker threads and plugin dispatch!");

    // Keep running
    tokio::signal::ctrl_c().await?;
    info!("Shutting down PocketMine-Rust server gracefully.");
    Ok(())
}
