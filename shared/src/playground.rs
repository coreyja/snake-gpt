use std::fmt::Debug;

use super::*;

#[async_trait::async_trait]
trait Api {
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

#[async_trait::async_trait]
trait ClientTransport {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn send_request(
        &self,
        method: &str,
        route: &str,
        body: Option<serde_json::Value>,
    ) -> Result<Result<serde_json::Value, serde_json::Value>, Self::Error>;
}

/// The goal is for this to be auto-generated by a Macro
#[async_trait::async_trait]
impl<Transport> Api for Transport
where
    Transport: ClientTransport + Sync,
    // TODO: I don't know what this line does the compiler told me to do it
    ClientError<ConversationError>: From<Transport::Error>,
{
    type ErrorWrapper<InnerError: Debug + for<'a> Deserialize<'a>> = ClientError<InnerError>;

    async fn start_chat(
        &self,
        body: ChatRequest,
    ) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>> {
        let body = serde_json::to_value(body)?;
        let resp = self
            .send_request(
                Self::API_START_CHAT_METHOD,
                Self::API_START_CHAT_ROUTE,
                Some(body),
            )
            .await?;
        match resp {
            Ok(resp) => {
                let resp = serde_json::from_value(resp)?;
                Ok(resp)
            }
            Err(resp) => {
                let resp = serde_json::from_value(resp)?;
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
            .await?;
        match resp {
            Ok(resp) => {
                let resp = serde_json::from_value(resp)?;
                Ok(resp)
            }
            Err(resp) => {
                let resp = serde_json::from_value(resp)?;
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
enum ClientError<T>
where
    T: std::fmt::Debug,
{
    #[error(transparent)]
    Transport(#[from] GlooErrorStandin),
    #[error(transparent)]
    Deserialization(#[from] serde_json::Error),
    #[error("Api Error")]
    Api(T),
}
