//! Data service binary entry point.

use acton_dx_proto::data::v1::data_service_server::DataServiceServer;
use data_service::{DataServiceConfig, DataServiceImpl};
use sqlx::any::AnyPoolOptions;
use std::net::SocketAddr;
use std::time::Duration;
use tonic::transport::Server;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "data_service=info,sqlx=warn,tonic=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting data-service");

    // Load configuration
    let config = DataServiceConfig::load().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config, using defaults: {}", e);
        // Return a minimal default config
        DataServiceConfig {
            database: data_service::DatabaseConfig {
                url: "sqlite::memory:".to_string(),
                max_connections: 10,
                min_connections: 1,
                connect_timeout_seconds: 30,
            },
            service: data_service::ServiceConfig::default(),
        }
    });

    // Install the SQLx Any driver
    sqlx::any::install_default_drivers();

    // Create database connection pool
    let pool = AnyPoolOptions::new()
        .max_connections(config.database.max_connections)
        .min_connections(config.database.min_connections)
        .acquire_timeout(Duration::from_secs(config.database.connect_timeout_seconds))
        .connect(&config.database.url)
        .await?;

    tracing::info!("Database connection pool established");

    // Create gRPC service
    let data_service = DataServiceImpl::new(pool);

    // Build server address
    let addr: SocketAddr = format!("{}:{}", config.service.host, config.service.port).parse()?;

    tracing::info!("Listening on {addr}");

    // Start gRPC server
    Server::builder()
        .add_service(DataServiceServer::new(data_service))
        .serve(addr)
        .await?;

    Ok(())
}
