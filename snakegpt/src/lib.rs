use clap::{Args, Parser, Subcommand};
use futures::{stream, StreamExt};
use indoc::formatdoc;
use itertools::Itertools;
use std::sync::{Arc, Mutex};
use std::{os::unix::prelude::PermissionsExt, path::PathBuf};

use miette::{IntoDiagnostic, Result};
use openai::{embeddings::EmbeddingsRequest, Client};
use rusqlite::{params, Connection, Row};

pub use crate::openai::completion::CompletionRequest;
pub use crate::openai::{Client as OpenAiClient, Config};

mod openai;
mod schema;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

pub const CONCURRENT_REQUESTS: usize = 5;
pub const DB_NAME: &str = "sample.v0.db";

pub fn setup() -> Result<Connection> {
    let conn = Connection::open(DB_NAME).into_diagnostic()?;
    load_my_extension(&conn)?;
    schema::setup_schema_v0(&conn)?;
    Ok(conn)
}

pub async fn respond_to(query: String, conn: Arc<Mutex<Connection>>) -> Result<(String, String)> {
    let config = Config::from_env()?;
    let client = config.client()?;

    let question = &query;
    let embedding = fetch_embedding(&client, question).await?;
    let embedding_json = serde_json::to_string(&embedding).into_diagnostic()?;

    let nearest_embeddings = {
        let conn = conn.lock().unwrap();
        let mut st = conn
            .prepare(
                "select rowid, distance
  from vss_sentences
  where vss_search(
    embedding,
    vector_from_json(?1)
  )
  limit 5;",
            )
            .into_diagnostic()?;
        let nearest_embeddings: Vec<Result<(u32, f64), _>> = st
            .query_map(params![&embedding_json], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .into_diagnostic()?
            .collect_vec();

        let nearest_embeddings: Vec<(String, f64)> = nearest_embeddings
            .into_iter()
            .map(|result| {
                let (rowid, distance) = result.into_diagnostic()?;
                let mut stmt = conn
                    .prepare("select text from sentences where rowid = ?1")
                    .into_diagnostic()?;
                let text: String = stmt
                    .query_row(params![rowid], |row| row.get(0))
                    .into_diagnostic()?;

                Ok((text, distance))
            })
            .collect::<Result<Vec<_>>>()?;

        nearest_embeddings
    };

    let context = nearest_embeddings
        .iter()
        .map(|(text, _)| format!("- {}", text.trim()))
        .join("\n");

    let prompt = formatdoc!(
        "
      You are a helpful chatbot Answering questions about Battlesnake.
      Battlesnake is an online competitve programming game.
      The goal of a battlesnake developer is to build a snake that can survive
      on the board the longest.

      Your job is to answer the users questions about Battlesnake as accurately as possible.
      

      Below is some context about the Users qustion. Use it to help you answer the question.
      After the context will be dashes like this: ----
      Below the dashes is the users question that you should answer.

      Context:
      {context}

      --------------------------------------

      {question}
      "
    );

    let completion_request = CompletionRequest::gpt_3_5_turbo(&prompt);
    let answer = client.completion(completion_request).await?;

    let first_choice = answer.choices.first().unwrap().message.content.clone();

    Ok((first_choice, context))
}

fn load_my_extension(conn: &Connection) -> Result<()> {
    // Safety: We fully trust the loaded extension and execute no untrusted SQL
    // while extension loading is enabled.
    unsafe {
        conn.load_extension_enable().into_diagnostic()?;
        conn.load_extension("./vendor/vector0", None)
            .into_diagnostic()?;
        conn.load_extension("./vendor/vss0", None)
            .into_diagnostic()?;
        conn.load_extension_disable().into_diagnostic()?;

        Ok(())
    }
}

pub async fn fetch_embedding(
    client: &Client,
    sentence: &str,
) -> Result<Vec<f64>, miette::ErrReport> {
    let embedding_resp = client
        .embeddings(EmbeddingsRequest::new(sentence.to_string()))
        .await?;
    let embedding = embedding_resp.data[0].embedding.clone();
    Ok(embedding)
}
