use miette::{IntoDiagnostic, Result};
use openai::Client;
use rusqlite::Connection;

use crate::openai::{
    embeddings::{EmbeddingResponse, EmbeddingsRequest},
    Config,
};

mod openai;
mod schema;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[tokio::main]
async fn main() -> Result<()> {
    let conn = Connection::open("sample.v0.db").into_diagnostic()?;

    load_my_extension(&conn)?;

    conn.query_row("select vss_version()", (), |result| {
        dbg!(&result);
        Ok(())
    })
    .into_diagnostic()?;

    schema::setup_schema_v0(&conn)?;

    let config = Config::from_env()?;
    let client = config.client()?;

    embed_sentence(&conn, &client, "Hello, world!").await?;

    Ok(())
}

async fn embed_sentence(conn: &Connection, client: &Client, sentence: &str) -> Result<()> {
    let embedding_resp = client
        .embeddings(EmbeddingsRequest::new(sentence.to_string()))
        .await?;
    let embedding = embedding_resp.data[0].embedding.clone();
    let embedding_json = serde_json::to_string(&embedding).into_diagnostic()?;

    let mut stmt = conn
        .prepare("INSERT OR IGNORE INTO sentences (text, embedding) VALUES (?, vector_to_blob(vector_from_json(?)))")
        .into_diagnostic()?;

    stmt.execute((sentence, embedding_json)).into_diagnostic()?;

    Ok(())
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
