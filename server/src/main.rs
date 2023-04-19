use std::sync::{Arc, Mutex};

use axum::{
    body::{boxed, Body},
    extract::{self, State},
    http::{self, Method, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use miette::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use snakegpt::{respond_to, setup};
use tower::ServiceExt;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

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
async fn main() -> Result<()> {
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(vec![http::header::CONTENT_TYPE])
        // allow requests from any origin
        .allow_origin(Any);

    let conn = setup()?;
    let conn = Mutex::new(conn);
    let conn = Arc::new(conn);

    // build our application with a single route
    let app = Router::new()
        .route(
            "/api/v0/chat",
            post(
                |State(conn): State<Arc<Mutex<Connection>>>,
                 extract::Json(r): Json<ChatRequest>| async {
                    let question = r.question;
                    let resp = respond_to(question.clone(), conn);
                    let (answer, prompt) = resp.await.unwrap();

                    Json(AnswerResp { answer, prompt })
                },
            ),
        )
        .with_state(conn)
        .fallback_service(get(|req| async move {
            match ServeDir::new("./dist").oneshot(req).await {
                Ok(res) => res.map(boxed),
                Err(err) => Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(boxed(Body::from(format!("error: {err}"))))
                    .expect("error response"),
            }
        }))
        .layer(cors);

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
