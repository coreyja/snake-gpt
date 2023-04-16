use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};

use super::Client;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Message {
    pub content: String,
    role: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct CompletionRequest {
    messages: Vec<Message>,
    model: String,
}

impl CompletionRequest {
    pub(crate) fn gpt_3_5_turbo(prompt: &str) -> Self {
        Self {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![Message {
                content: prompt.to_string(),
                role: "user".to_string(),
            }],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct CompletionChoice {
    finish_reason: String,
    index: i64,
    pub message: Message,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct CompletionUsage {
    completion_tokens: i64,
    prompt_tokens: i64,
    total_tokens: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct CompletionResponse {
    pub(crate) choices: Vec<CompletionChoice>,
    pub(crate) created: i64,
    pub(crate) id: String,
    pub(crate) model: String,
    pub(crate) object: String,
    pub(crate) usage: CompletionUsage,
}

impl Client {
    pub(crate) async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse> {
        let response = self
            .0
            .post("https://api.openai.com/v1/chat/completions")
            .json(&request)
            .send()
            .await
            .into_diagnostic()?;

        let response_body = response.json().await.into_diagnostic()?;

        Ok(response_body)
    }

    pub(crate) async fn split_by_sentences(&self, blob: &str) -> Result<Vec<String>> {
        let started = std::time::Instant::now();

        let prompt = format!(
        "I will paste a block of markdown. I need you to remove all the formatting, and break each sentence onto its own line
        Make sure each sentence has a blank line between it. Code blocks should be considered a single sentence.
        
        {blob}");
        let request = CompletionRequest::gpt_3_5_turbo(&prompt);
        let resp = self.completion(request).await?;

        let message = resp.choices[0].message.content.clone();

        println!("Splitting by sentences took {:?}", started.elapsed());

        Ok(message.split("\n\n").map(|s| s.to_string()).collect())
    }
}
