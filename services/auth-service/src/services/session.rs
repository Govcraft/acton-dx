//! gRPC Session Service implementation.

use crate::agents::session_manager::{
    AddFlash, CreateSession, DeleteSession, LoadSession, TakeFlashes, UpdateSession,
};
use crate::{FlashMessage, SessionData};
use acton_dx_proto::auth::v1::{
    session_service_server::SessionService, AddFlashMessageRequest, AddFlashMessageResponse,
    CreateSessionRequest, CreateSessionResponse, DestroySessionRequest, DestroySessionResponse,
    FlashMessage as ProtoFlashMessage, GetFlashMessagesRequest, GetFlashMessagesResponse,
    Session as ProtoSession, UpdateSessionRequest, UpdateSessionResponse, ValidateSessionRequest,
    ValidateSessionResponse,
};
use acton_reactive::prelude::{ActorHandle, ActorHandleInterface};
use std::time::Duration;
use tonic::{Request, Response, Status};

/// gRPC Session Service implementation.
#[derive(Debug, Clone)]
pub struct SessionServiceImpl {
    session_agent: ActorHandle,
}

impl SessionServiceImpl {
    /// Create a new session service implementation.
    #[must_use]
    pub const fn new(session_agent: ActorHandle) -> Self {
        Self { session_agent }
    }
}

fn session_data_to_proto(session: &SessionData) -> ProtoSession {
    ProtoSession {
        session_id: session.session_id.clone(),
        user_id: session.user_id,
        user_email: session.user_email.clone(),
        user_name: session.user_name.clone(),
        data: session.data.clone(),
        csrf_token: session.csrf_token.clone(),
        created_at: session.created_at.timestamp(),
        expires_at: session.expires_at.timestamp(),
    }
}

fn flash_to_proto(flash: &FlashMessage) -> ProtoFlashMessage {
    ProtoFlashMessage {
        level: flash.level.clone(),
        message: flash.message.clone(),
    }
}

#[tonic::async_trait]
impl SessionService for SessionServiceImpl {
    async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> Result<Response<CreateSessionResponse>, Status> {
        let req = request.into_inner();
        let ttl_seconds = u64::try_from(req.ttl_seconds).unwrap_or(3600);

        let (msg, rx) = CreateSession::with_response(req.user_id, ttl_seconds);
        self.session_agent.send(msg).await;

        let session = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .map_err(|_| Status::deadline_exceeded("Session creation timed out"))?
            .map_err(|_| Status::internal("Session agent channel closed"))?;

        Ok(Response::new(CreateSessionResponse {
            session: Some(session_data_to_proto(&session)),
        }))
    }

    async fn validate_session(
        &self,
        request: Request<ValidateSessionRequest>,
    ) -> Result<Response<ValidateSessionResponse>, Status> {
        let req = request.into_inner();

        let (msg, rx) = LoadSession::with_response(req.session_id);
        self.session_agent.send(msg).await;

        let session = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .map_err(|_| Status::deadline_exceeded("Session validation timed out"))?
            .map_err(|_| Status::internal("Session agent channel closed"))?;

        match session {
            Some(s) if !s.is_expired() => Ok(Response::new(ValidateSessionResponse {
                valid: true,
                session: Some(session_data_to_proto(&s)),
            })),
            _ => Ok(Response::new(ValidateSessionResponse {
                valid: false,
                session: None,
            })),
        }
    }

    async fn update_session(
        &self,
        request: Request<UpdateSessionRequest>,
    ) -> Result<Response<UpdateSessionResponse>, Status> {
        let req = request.into_inner();

        let (msg, rx) = UpdateSession::with_response(req.session_id, req.data, req.user_id);
        self.session_agent.send(msg).await;

        let session = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .map_err(|_| Status::deadline_exceeded("Session update timed out"))?
            .map_err(|_| Status::internal("Session agent channel closed"))?;

        session.map_or_else(
            || {
                Ok(Response::new(UpdateSessionResponse {
                    success: false,
                    session: None,
                }))
            },
            |s| {
                Ok(Response::new(UpdateSessionResponse {
                    success: true,
                    session: Some(session_data_to_proto(&s)),
                }))
            },
        )
    }

    async fn destroy_session(
        &self,
        request: Request<DestroySessionRequest>,
    ) -> Result<Response<DestroySessionResponse>, Status> {
        let req = request.into_inner();

        let (msg, rx) = DeleteSession::with_response(req.session_id);
        self.session_agent.send(msg).await;

        let deleted = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .map_err(|_| Status::deadline_exceeded("Session destruction timed out"))?
            .map_err(|_| Status::internal("Session agent channel closed"))?;

        Ok(Response::new(DestroySessionResponse { success: deleted }))
    }

    async fn add_flash_message(
        &self,
        request: Request<AddFlashMessageRequest>,
    ) -> Result<Response<AddFlashMessageResponse>, Status> {
        let req = request.into_inner();
        let flash = req
            .flash
            .ok_or_else(|| Status::invalid_argument("flash message is required"))?;

        let (msg, rx) = AddFlash::with_response(
            req.session_id,
            FlashMessage {
                level: flash.level,
                message: flash.message,
            },
        );
        self.session_agent.send(msg).await;

        let success = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .map_err(|_| Status::deadline_exceeded("Add flash timed out"))?
            .map_err(|_| Status::internal("Session agent channel closed"))?;

        Ok(Response::new(AddFlashMessageResponse { success }))
    }

    async fn get_and_clear_flash_messages(
        &self,
        request: Request<GetFlashMessagesRequest>,
    ) -> Result<Response<GetFlashMessagesResponse>, Status> {
        let req = request.into_inner();

        let (msg, rx) = TakeFlashes::with_response(req.session_id);
        self.session_agent.send(msg).await;

        let flashes = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .map_err(|_| Status::deadline_exceeded("Get flashes timed out"))?
            .map_err(|_| Status::internal("Session agent channel closed"))?;

        let messages = flashes.iter().map(flash_to_proto).collect();

        Ok(Response::new(GetFlashMessagesResponse { messages }))
    }
}
