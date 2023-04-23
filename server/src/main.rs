use std::sync::{Arc, Mutex};

use axum::{
    body::{boxed, Body},
    extract::{self, FromRef, State},
    http::{self, Method, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use miette::{IntoDiagnostic, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use snakegpt::{respond_to, setup, EmbeddingConnection};
use tower::ServiceExt;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

#[derive(Clone, Debug)]
pub struct AppConnection(pub Arc<Mutex<Connection>>);

#[derive(Deserialize, Serialize, Debug, Clone)]
struct AnswerResp {
    answer: String,
    context: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct ChatRequest {
    question: String,
}

#[derive(Debug, Clone)]
struct AppState {
    embedding_connection: EmbeddingConnection,
    app_connection: AppConnection,
}

impl FromRef<AppState> for AppConnection {
    fn from_ref(conn: &AppState) -> Self {
        conn.app_connection.clone()
    }
}

impl FromRef<AppState> for EmbeddingConnection {
    fn from_ref(conn: &AppState) -> Self {
        conn.embedding_connection.clone()
    }
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
    let conn = EmbeddingConnection(conn);

    let app_conn = Connection::open_in_memory().into_diagnostic()?;

    let state = AppState {
        embedding_connection: conn,
        app_connection: AppConnection(Arc::new(Mutex::new(app_conn))),
    };

    // build our application with a single route
    let app =
        Router::new()
            .route(
                "/api/v0/chat",
                post(
                    |State(conn): State<EmbeddingConnection>,
                     extract::Json(r): Json<ChatRequest>| async {
                        let question = r.question;
                        let resp = respond_to(question.clone(), conn);
                        let (answer, context) = resp.await.unwrap();

                        Json(AnswerResp { answer, context })
                    },
                ),
            )
            .with_state(state)
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
