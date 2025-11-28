//! Auth service binary entry point.

use acton_dx_proto::auth::v1::{
    csrf_service_server::CsrfServiceServer, password_service_server::PasswordServiceServer,
    session_service_server::SessionServiceServer,
};
use acton_reactive::prelude::ActonApp;
use auth_service::{
    AuthServiceConfig, CsrfServiceImpl, PasswordServiceImpl, SessionManagerAgent,
    SessionServiceImpl,
};
use std::net::SocketAddr;
use tonic::transport::Server;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "auth_service=info,tonic=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting auth-service");

    // Load configuration
    let config = AuthServiceConfig::load().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config, using defaults: {}", *e);
        AuthServiceConfig::default()
    });

    // Initialize acton-reactive runtime
    let mut runtime = ActonApp::launch();

    // Spawn session manager agent
    let session_agent = SessionManagerAgent::spawn(
        &mut runtime,
        config.session.cleanup_interval_seconds,
    )
    .await?;

    tracing::info!("Session manager agent started");

    // Create gRPC services
    let session_service = SessionServiceImpl::new(session_agent);
    let password_service = PasswordServiceImpl::with_params(
        config.password.memory_cost,
        config.password.time_cost,
        config.password.parallelism,
        Some(config.password.hash_length),
    );
    let csrf_service = CsrfServiceImpl::with_config(
        config.csrf.token_ttl_seconds,
        config.csrf.token_bytes,
    );

    // Build server address
    let addr: SocketAddr = format!("{}:{}", config.service.host, config.service.port).parse()?;

    tracing::info!("Listening on {addr}");

    // Start gRPC server
    Server::builder()
        .add_service(SessionServiceServer::new(session_service))
        .add_service(PasswordServiceServer::new(password_service))
        .add_service(CsrfServiceServer::new(csrf_service))
        .serve(addr)
        .await?;

    // Shutdown runtime
    runtime.shutdown_all().await?;

    Ok(())
}
