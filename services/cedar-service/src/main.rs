//! Cedar authorization service entry point.

use acton_dx_proto::cedar::v1::cedar_service_server::CedarServiceServer;
use cedar_service::{CedarServiceConfig, CedarServiceImpl};
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

    info!("Starting Cedar authorization service");

    // Load configuration
    let config = CedarServiceConfig::load()?;

    // Create the service
    let service = CedarServiceImpl::new(&config.policies.path)?;

    // Build the address
    let addr: SocketAddr = format!("{}:{}", config.service.host, config.service.port).parse()?;

    info!(%addr, "Cedar service listening");

    // Start the gRPC server
    Server::builder()
        .add_service(CedarServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
