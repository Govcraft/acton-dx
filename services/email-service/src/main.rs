//! Email service entry point.

use acton_dx_proto::email::v1::email_service_server::EmailServiceServer;
use email_service::{EmailServiceConfig, EmailServiceImpl};
use lettre::message::Mailbox;
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

    info!("Starting email service");

    // Load configuration
    let config = EmailServiceConfig::load()?;

    // Build default from address
    let default_from = match (&config.smtp.from_address, &config.smtp.from_name) {
        (Some(addr), Some(name)) => {
            let email = addr.parse()?;
            Some(Mailbox::new(Some(name.clone()), email))
        }
        (Some(addr), None) => {
            let email = addr.parse()?;
            Some(Mailbox::new(None, email))
        }
        _ => None,
    };

    // Create the service
    let service = EmailServiceImpl::new(
        &config.smtp.host,
        config.smtp.port,
        config.smtp.username.as_deref(),
        config.smtp.password.as_deref(),
        config.smtp.tls,
        default_from,
    )?;

    info!(
        host = %config.smtp.host,
        port = config.smtp.port,
        tls = config.smtp.tls,
        "SMTP transport configured"
    );

    // Build the address
    let addr: SocketAddr = format!("{}:{}", config.service.host, config.service.port).parse()?;

    info!(%addr, "Email service listening");

    // Start the gRPC server
    Server::builder()
        .add_service(EmailServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
