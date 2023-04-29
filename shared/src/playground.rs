use std::fmt::Debug;

use super::*;
pub trait Api {
    type ErrorWrapper<T: for<'a> Deserialize<'a> + Debug>;

    const API_START_CHAT_ROUTE: &'static str = "/v0/chat";
    const API_START_CHAT_METHOD: &'static str = "post";
    async fn start_chat(
        &self,
        body: ChatRequest,
    ) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>>;

    const API_GET_CONVERSATION_ROUTE: &'static str = "/v0/conversations/{conversation_slug}";
    const API_GET_CONVERSATION_METHOD: &'static str = "get";
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

/// The goal is for this to be auto-generated by a Macro
impl<Transport> Api for Transport
where
    Transport: ClientTransport,
{
    type ErrorWrapper<InnerError: Debug + for<'a> Deserialize<'a>> =
        ClientError<InnerError, Transport::Error>;

    async fn start_chat(
        &self,
        body: ChatRequest,
    ) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>> {
        let body = serde_json::to_value(body).map_err(ClientError::Serialization)?;
        let resp = self
            .send_request(
                Self::API_START_CHAT_METHOD,
                Self::API_START_CHAT_ROUTE,
                Some(body),
            )
            .await
            .map_err(ClientError::Transport)?;
        match resp {
            Ok(resp) => {
                let resp = serde_json::from_value(resp).map_err(ClientError::Deserialization)?;
                Ok(resp)
            }
            Err(resp) => {
                let resp = serde_json::from_value(resp).map_err(ClientError::Deserialization)?;
                Err(ClientError::Api(resp))
            }
        }
    }

    async fn get_conversation(
        &self,
        conversation_slug: String,
    ) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>> {
        let route =
            Self::API_GET_CONVERSATION_ROUTE.replace("{conversation_slug}", &conversation_slug);
        let resp = self
            .send_request(Self::API_GET_CONVERSATION_METHOD, &route, None)
            .await
            .map_err(ClientError::Transport)?;
        match resp {
            Ok(resp) => {
                let resp = serde_json::from_value(resp).map_err(ClientError::Deserialization)?;
                Ok(resp)
            }
            Err(resp) => {
                let resp = serde_json::from_value(resp).map_err(ClientError::Deserialization)?;
                Err(ClientError::Api(resp))
            }
        }
    }
}

#[derive(Error, Debug)]
#[error("oops!")]
// #[diagnostic()]
struct GlooErrorStandin;

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
