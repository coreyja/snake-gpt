use std::fmt::Debug;

use super::*;

#[macros::rpc]
pub trait Api {
    type ErrorWrapper<T: for<'a> Deserialize<'a> + Debug>;

    #[rpc(route = "/v0/chat", method = "post")]
    async fn start_chat(
        &self,
        body: ChatRequest,
    ) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>>;

    #[rpc(route = "/v0/conversations/:conversation_slug", method = "get")]
    async fn get_conversation(
        &self,
        conversation_slug: String,
    ) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>>;
}

#[derive(Debug, Clone)]
pub struct RouteInfo {
    pub route: &'static str,
    pub method: &'static str,
    pub sig: syn::Signature,
}

pub fn api_routes() -> Vec<RouteInfo> {
    vec![
        RouteInfo {
            route: "/api/v0/chat",
            method: "post",
            sig: syn::parse_str(
                "async fn start_chat(&self, body: ChatRequest) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>>",
            ).unwrap(),
        },
        RouteInfo {
            route: "/api/v0/conversations/{conversation_slug}",
            method: "get",
            sig: syn::parse_str(
                "async fn get_conversation(&self, conversation_slug: String) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>>",
            ).unwrap(),
        },
    ]
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
