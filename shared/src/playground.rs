use std::fmt::Debug;

use super::*;

#[macros::rpc]
pub trait Api {
    type ErrorWrapper<T: for<'a> Deserialize<'a> + Debug>;

    const START_CHAT_ROUTE: &'static str = "/v0/chat";
    const START_CHAT_METHOD: &'static str = "post";

    #[rpc(route = "/v0/chat", method = "post")]
    async fn start_chat(
        &self,
        body: ChatRequest,
    ) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>>;

    const GET_CONVERSATION_ROUTE: &'static str = "/v0/conversations/{conversation_slug}";
    const GET_CONVERSATION_METHOD: &'static str = "get";

    #[rpc(route = "/v0/conversations/{conversation_slug}", method = "get")]
    async fn get_conversation(
        &self,
        conversation_slug: String,
    ) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>>;
}

pub trait ClientTransport: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn send_request(
        &self,
        method: &str,
        route: &str,
        body: Option<serde_json::Value>,
    ) -> Result<Result<serde_json::Value, serde_json::Value>, Self::Error>;
}

use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum ClientError<ApiError, TransportError>
where
    ApiError: std::fmt::Debug,
    TransportError: std::fmt::Debug + std::error::Error,
{
    #[error(transparent)]
    Transport(TransportError),
    #[error(transparent)]
    Deserialization(serde_json::Error),
    #[error(transparent)]
    Serialization(serde_json::Error),
    #[error("Api Error")]
    Api(ApiError),
}

#[derive(Error, Diagnostic, Debug)]
pub enum ServerError<InternalError>
where
    InternalError: std::fmt::Debug,
{
    #[error(transparent)]
    Deserialization(serde_json::Error),
    #[error(transparent)]
    Serialization(serde_json::Error),
    #[error("Internal Error")]
    Api(InternalError),
}
