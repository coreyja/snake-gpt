use axum::{
    extract,
    http::{self, Method},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use snakegpt::respond_to;
use tower_http::cors::{Any, CorsLayer};

#[derive(Deserialize, Serialize, Debug, Clone)]
struct AnswerResp {
    answer: String,
    prompt: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct ChatRequest {
    question: String,
}

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(vec![http::header::CONTENT_TYPE])
        // allow requests from any origin
        .allow_origin(Any);

    // build our application with a single route
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route(
            "/api/v0/chat",
            post(|extract::Json(r): Json<ChatRequest>| async {
                let question = r.question;
                let resp = respond_to(question.clone()).await.unwrap();
                let (answer, prompt) = resp;

                Json(AnswerResp { answer, prompt })
            }),
        )
        .layer(cors);

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
