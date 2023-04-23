use std::sync::{Arc, Mutex};

use axum::{
    body::{boxed, Body},
    extract::{self, FromRef, Path, State},
    http::{self, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use miette::{Context, IntoDiagnostic, Result};
use rusqlite::{params, Connection, Row};
use shared::{AnswerResp, ChatRequest, ConversationResponse};
use snakegpt::{respond_to, setup, EmbeddingConnection};
use tower::ServiceExt;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct AppConnection(pub Arc<Mutex<Connection>>);

impl AppConnection {
    pub fn setup_schema_v0(&self) -> Result<()> {
        let conn = self.0.lock().unwrap();

        conn.execute(
            "
            CREATE TABLE IF NOT EXISTS conversations (
                slug                  TEXT NOT NULL,
                question              TEXT NOT NULL,
                answer                TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS uniq_index_conversations_slugs on conversations (slug);
            ",
            (),
        )
        .into_diagnostic()?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS actions (
                conversation_id     TEXT NOT NULL,
                action_type         TEXT NOT NULL,
                action_data         TEXT NOT NULL
            )",
            (),
        )
        .into_diagnostic()?;

        Ok(())
    }
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
    let app_conn = Mutex::new(app_conn);
    let app_conn = Arc::new(app_conn);
    let app_conn = AppConnection(app_conn);

    app_conn
        .setup_schema_v0()
        .wrap_err("Couldn't setup app DB schema ")?;

    let state = AppState {
        embedding_connection: conn,
        app_connection: app_conn,
    };

    // build our application with a single route
    let app = Router::new()
        .route("/api/v0/chat", post(start_chat))
        .route("/api/v0/conversations/:slug", get(get_convo))
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

async fn start_chat(
    State(conn): State<EmbeddingConnection>,
    State(app): State<AppConnection>,
    extract::Json(r): Json<ChatRequest>,
) -> impl IntoResponse {
    let question = r.question;

    let conversation_id = {
        let app = app.0.lock().unwrap();
        app.query_row(
            "INSERT OR IGNORE INTO conversations (slug, question) VALUES (?, ?) returning rowid",
            params![r.conversation_slug.to_string(), question],
            |row: &Row| -> Result<i64, _> { row.get(0) },
        )
        .unwrap()
    };

    // Create a new conversation in the DB with the question
    let resp = respond_to(question.clone(), conn);
    let (answer, context) = resp.await.unwrap();

    {
        let app = app.0.lock().unwrap();
        app.execute(
            "UPDATE conversations SET answer = ? WHERE rowid = ?",
            params![answer, conversation_id],
        )
        .unwrap();
    }

    Json(AnswerResp { answer, context })
}

async fn get_convo(
    State(app): State<AppConnection>,
    Path(convo_slug): Path<Uuid>,
) -> impl IntoResponse {
    let app = app.0.lock().unwrap();
    let convo: (Result<String>, Result<Option<String>>) = app
        .query_row(
            "SELECT question, answer FROM conversations WHERE slug = ?",
            params![convo_slug.to_string()],
            |row: &Row| Ok((row.get(0).into_diagnostic(), row.get(1).into_diagnostic())),
        )
        .unwrap();

    Json(ConversationResponse {
        slug: convo_slug,
        question: convo.0.unwrap(),
        answer: convo.1.unwrap(),
    })
}
