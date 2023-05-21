use std::collections::HashMap;

use rusqlite::{params, Row};
use serde::Serialize;
use shared::{
    playground::{Api, ServerError},
    ChatRequest, ConversationError, ConversationResponse,
};
use snakegpt::{get_context, respond_to_with_context, EmbeddingConnection};

use crate::{convo_resp_from_slug, AppConnection};

pub struct AxumRoutable {
    pub app: AppConnection,
    pub embedding: EmbeddingConnection,
}

impl Api for AxumRoutable {
    type ErrorWrapper<InnerError: std::fmt::Debug + for<'a> serde::Deserialize<'a> + Serialize> =
        ServerError<InnerError>;

    async fn start_chat(
        &self,
        body: ChatRequest,
    ) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>> {
        let question = body.question;

        let conversation_id = {
            let app = self.app.0.lock().unwrap();
            app.query_row(
            "INSERT OR IGNORE INTO conversations (slug, question) VALUES (?, ?) returning rowid",
            params![body.conversation_slug.to_string(), question],
            |row: &Row| -> Result<i64, _> { row.get(0) },
        )
        .unwrap()
        };

        let convo_resp = convo_resp_from_slug(&self.app, body.conversation_slug).unwrap();

        let conversation_id = conversation_id;
        let app_for_spawn = self.app.clone();
        let embedding_for_spawn = self.embedding.clone();
        tokio::spawn(async move {
            let (context, question) = get_context(question.clone(), embedding_for_spawn)
                .await
                .unwrap();
            {
                let app = app_for_spawn.0.lock().unwrap();
                app.execute(
                    "UPDATE conversations SET context = ? WHERE rowid = ?",
                    params![context, conversation_id],
                )
                .unwrap();
            }

            // Create a new conversation in the DB with the question
            let resp = respond_to_with_context(context, question);
            let (answer, _context) = resp.await.unwrap();

            {
                let app = app_for_spawn.0.lock().unwrap();
                app.execute(
                    "UPDATE conversations SET answer = ? WHERE rowid = ?",
                    params![answer, conversation_id],
                )
                .unwrap();
            }
        });

        Ok(convo_resp.unwrap())
    }

    async fn get_conversation(
        &self,
        conversation_slug: String,
    ) -> Result<ConversationResponse, Self::ErrorWrapper<ConversationError>> {
        Ok(
            convo_resp_from_slug(&self.app, conversation_slug.parse().unwrap())
                .map(|x| x.unwrap())
                .unwrap(),
        )
    }
}
