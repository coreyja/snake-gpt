use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AnswerResp {
    pub answer: String,
    pub context: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ChatRequest {
    pub question: String,
}
