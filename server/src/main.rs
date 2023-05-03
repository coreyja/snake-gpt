#![feature(async_fn_in_trait)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    body::{boxed, Body},
    extract::{self, FromRef, Path, State},
    http::{self, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get, post},
    Json, Router,
};
use miette::{Context, IntoDiagnostic, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use shared::{
    playground::{api_routes, Api},
    ChatRequest, ConversationResponse,
};
use snakegpt::{get_context, respond_to_with_context, setup, EmbeddingConnection};
use tower::ServiceExt;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};
use uuid::Uuid;

pub mod rpc;

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
                context               TEXT,
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
        .allow_headers([http::header::CONTENT_TYPE])
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

    #[axum_macros::debug_handler(state = AppState)]
    async fn start_chat_inner(
        State(conn): State<EmbeddingConnection>,
        State(app): State<AppConnection>,
        extract::Json(r): Json<ChatRequest>,
    ) -> Response {
        let rpc = rpc::AxumRoutable {
            app,
            embedding: conn,
        };

        let resp = rpc.start_chat(r).await;

        match resp {
            Ok(resp) => Json(resp).into_response(),
            Err(err) => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(boxed(Body::from(format!("error: {err}"))))
                .unwrap(),
        }
    }

    #[axum_macros::debug_handler(state = AppState)]
    async fn get_convo_inner(
        State(app): State<AppConnection>,
        State(embedding): State<EmbeddingConnection>,
        Path(convo_slug): Path<Uuid>,
    ) -> Response {
        let rpc = rpc::AxumRoutable { app, embedding };

        let resp = rpc.get_conversation(convo_slug.to_string()).await;

        match resp {
            Ok(resp) => Json(resp).into_response(),
            Err(err) => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(boxed(Body::from(format!("error: {err}"))))
                .unwrap(),
        }
    }

    // build our application with a single route
    let mut app = Router::new();

    let api_routes = api_routes();

    for r in api_routes {
        let wrapper = match r.method {
            "get" => get,
            "post" => post,
            _ => panic!("Unknown method {}", r.method),
        };
        dbg!(&r);
        app = app.route(
            r.route,
            wrapper(
                |State(app): State<AppConnection>,
                 State(embedding): State<EmbeddingConnection>,
                 Path(params): Path<HashMap<String, String>>,
                 uri: axum::http::Uri| async move {
                    let rpc = rpc::AxumRoutable { app, embedding };

                    // rpc.call(r, params).await.map_err(|e| {
                    //     eprintln!("Error: {}", e);
                    //     e
                    // })
                    todo!("Got stuck here....")
                },
            ),
        );
    }

    let app = app
        // .route("/api/v0/chat", post(start_chat_inner))
        // .route("/api/v0/conversations/:slug", get(get_convo_inner))
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

pub fn convo_resp_from_slug(
    app: &AppConnection,
    convo_slug: Uuid,
) -> Result<Option<ConversationResponse>> {
    let app = app.0.lock().unwrap();
    let convo: Option<ConversationResponse> = app
        .query_row(
            "SELECT question, answer, context FROM conversations WHERE slug = ?",
            params![convo_slug.to_string()],
            |row: &Row| {
                Ok(ConversationResponse {
                    slug: convo_slug,
                    question: row.get(0)?,
                    answer: row.get(1)?,
                    context: row.get(2)?,
                })
            },
        )
        .optional()
        .into_diagnostic()?;

    Ok(convo)
}
