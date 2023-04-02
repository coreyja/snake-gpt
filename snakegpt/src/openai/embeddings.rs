use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};

use super::Client;

const EMBEDDING_DEFAULT_MODEL: &str = "text-embedding-ada-002";

#[derive(Debug, Clone, Serialize)]
pub(crate) struct EmbeddingsRequest {
    input: String,
    model: &'static str,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct EmbeddingData {
    embedding: Vec<f64>,
    index: i64,
    object: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct EmbeddingUsage {
    prompt_tokens: i64,
    total_tokens: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
    model: String,
    object: String,
    usage: EmbeddingUsage,
}

impl EmbeddingsRequest {
    pub(crate) fn new(input: String) -> Self {
        Self {
            input,
            model: EMBEDDING_DEFAULT_MODEL,
        }
    }
}

impl Client {
    pub(crate) async fn embeddings(&self, request: EmbeddingsRequest) -> Result<EmbeddingResponse> {
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
