//! Cache service entry point.

use acton_dx_proto::cache::v1::cache_service_server::CacheServiceServer;
use cache_service::{CacheServiceConfig, CacheServiceImpl};
use redis::Client;
use std::net::SocketAddr;
use tonic::transport::Server;
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env_lossy(),
        )
        .init();

    info!("Starting cache service");

    // Load configuration
    let config = CacheServiceConfig::load()?;

    // Connect to Redis
    let client = Client::open(config.redis.url.as_str())?;
    let conn = client.get_connection_manager().await?;

    info!(url = %config.redis.url, "Connected to Redis");

    // Create the service
    let service = CacheServiceImpl::new(conn);

    // Build the address
    let addr: SocketAddr = format!("{}:{}", config.service.host, config.service.port).parse()?;

    info!(%addr, "Cache service listening");

    // Start the gRPC server
    Server::builder()
        .add_service(CacheServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
