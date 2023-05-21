use std::{collections::HashMap, fmt::Debug};

use super::*;

#[macros::rpc]
pub trait Api {
    type ErrorWrapper<T: for<'a> Deserialize<'a> + Debug + Serialize>;

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
            route: "/api/v0/conversations/:conversation_slug",
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

pub trait RpcCallable<Inner> {
    async fn call(
        &self,
        route: String,
        params: HashMap<String, String>,
        body: Option<String>,
    ) -> Result<serde_json::Value, serde_json::Value>;
}

impl<Inner> RpcCallable<Inner> for Inner
where
    Inner: Api,
{
    async fn call(
        &self,
        route: String,
        params: HashMap<String, String>,
        body: Option<String>,
    ) -> Result<serde_json::Value, serde_json::Value> {
        match route.as_str() {
            "/api/v0/chat" => {
                let body = body.unwrap();
                let body = serde_json::from_str(&body).unwrap();
                let resp = self.start_chat(body).await;

                match resp {
                    Ok(resp) => Ok(serde_json::to_value(resp).unwrap()),
                    Err(_err) => panic!("Error needs to be serializable come back to this"),
                }
            }
            "/api/v0/conversations/:conversation_slug" => {
                let conversation_slug = params.get("conversation_slug").unwrap();

                let resp = self.get_conversation(conversation_slug.clone()).await;

                match resp {
                    Ok(resp) => Ok(serde_json::to_value(resp).unwrap()),
                    Err(_err) => panic!("Error needs to be serializable come back to this"),
                }
            }
            _ => panic!("Unknown route: {}", route),
        }
    }
}
