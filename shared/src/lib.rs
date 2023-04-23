use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AnswerResp {
    pub answer: String,
    pub context: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ChatRequest {
    pub conversation_slug: Uuid,
    pub question: String,
}


#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ConversationResponse {
    pub slug: Uuid,
    pub question: String,
    pub answer: Option<String>,
}
