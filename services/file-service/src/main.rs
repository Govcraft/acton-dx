//! File service entry point.

use acton_dx_proto::file::v1::file_service_server::FileServiceServer;
use file_service::{FileServiceConfig, FileServiceImpl};
use std::net::SocketAddr;
use std::path::PathBuf;
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

    info!("Starting file service");

    // Load configuration
    let config = FileServiceConfig::load()?;

    // Create the service
    let service = FileServiceImpl::new(
        PathBuf::from(&config.storage.base_path),
        config.urls.public_base_url,
        config.urls.signing_key,
        config.storage.chunk_size,
    )
    .await?;

    info!(
        path = %config.storage.base_path,
        max_size = config.storage.max_file_size,
        chunk_size = config.storage.chunk_size,
        "File storage configured"
    );

    // Build the address
    let addr: SocketAddr = format!("{}:{}", config.service.host, config.service.port).parse()?;

    info!(%addr, "File service listening");

    // Start the gRPC server
    Server::builder()
        .add_service(FileServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
