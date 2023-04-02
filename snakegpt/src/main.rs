use miette::{IntoDiagnostic, Result};

use crate::openai::{
    embeddings::{EmbeddingResponse, EmbeddingsRequest},
    Config,
};

mod openai;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;

    let client = config.client()?;

    let request = EmbeddingsRequest::new("Hello world!".to_string());

    let response_body = client.embeddings(request).await?;

    dbg!(&response_body);

    Ok(())
}
