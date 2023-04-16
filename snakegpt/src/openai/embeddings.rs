use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};

use super::Client;

const EMBEDDING_DEFAULT_MODEL: &str = "text-embedding-ada-002";

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingsRequest {
    input: String,
    model: &'static str,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbeddingData {
    pub embedding: Vec<f64>,
    pub index: i64,
    pub object: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbeddingUsage {
    prompt_tokens: i64,
    total_tokens: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmbeddingResponse {
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub object: String,
    pub usage: EmbeddingUsage,
}

impl EmbeddingsRequest {
    pub fn new(input: String) -> Self {
        Self {
            input,
            model: EMBEDDING_DEFAULT_MODEL,
        }
    }
}

impl Client {
    pub async fn embeddings(
        &self,
        request: impl IntoEmbeddingsRequest,
    ) -> Result<EmbeddingResponse> {
        let request: EmbeddingsRequest = request.into();
        let response = self
            .0
            .post("https://api.openai.com/v1/embeddings")
            .json(&request)
            .send()
            .await
            .into_diagnostic()?;

        let response_body = response
            .json::<EmbeddingResponse>()
            .await
            .into_diagnostic()?;

        Ok(response_body)
    }
}

pub trait IntoEmbeddingsRequest {
    fn into(self) -> EmbeddingsRequest;
}

impl IntoEmbeddingsRequest for EmbeddingsRequest {
    fn into(self) -> EmbeddingsRequest {
        self
    }
}

impl<T> IntoEmbeddingsRequest for T
where
    T: Into<String>,
{
    fn into(self) -> EmbeddingsRequest {
        EmbeddingsRequest::new(self.into())
    }
}
