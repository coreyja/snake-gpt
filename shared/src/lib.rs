use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ChatRequest {
    pub conversation_slug: Uuid,
    pub question: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ConversationResponse {
    pub slug: Uuid,
    pub question: String,
    pub context: Option<String>,
    pub answer: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ConversationError;

mod playground;
