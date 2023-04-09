use clap::Parser;
use std::{io::Write, path::PathBuf};

use bstr::{BStr, ByteSlice};
use miette::{IntoDiagnostic, Result};
use openai::Client;
use rusqlite::{params, Connection, Row};

use crate::openai::{
    completion::CompletionRequest,
    embeddings::{EmbeddingResponse, EmbeddingsRequest},
    Config,
};

mod openai;
mod schema;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// SnakeGPT
///
/// A chatbot that uses the Battlesnake Docs site and related content
/// to generate responses to questions about Battlesnake.
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Path to search for Markdown files
    #[arg(short, long)]
    path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

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

    for entry in walkdir::WalkDir::new(&args.path) {
        let entry = entry.into_diagnostic()?;
        if entry.file_type().is_file() {
            let path = entry.path();

            if path.to_string_lossy().contains("node_modules") {
                continue;
            }

            if let Some(ext) = path.extension() {
                if ext.to_str() == Some("md") {
                    println!("About to Process Path: {}", path.display());

                    let display_path = path.display().to_string();

                    let page_id = conn.query_row(
                        "INSERT OR IGNORE INTO pages (path) VALUES (?) returning rowid",
                        params![display_path],
                        |row: &Row| -> Result<i64, _> { Ok(row.get(0)?) },
                    );

                    let page_id = match page_id {
                        Ok(id) => id,
                        Err(e) => conn
                            .query_row(
                                "SELECT rowid FROM pages WHERE path = ?",
                                params![display_path],
                                |row: &Row| -> Result<i64, _> { Ok(row.get(0)?) },
                            )
                            .into_diagnostic()?,
                    };

                    let content = std::fs::read_to_string(path).into_diagnostic()?;
                    let sentences = client.split_by_sentences(&content).await?;

                    for (i, s) in sentences.into_iter().enumerate() {
                        print!(".");
                        std::io::stdout().flush().unwrap();

                        embed_sentence(&conn, &client, &s, page_id, i).await?;
                    }
                    println!();
                    // todo!();
                }
            }
        }
    }

    Ok(())
}

async fn embed_sentence(
    conn: &Connection,
    client: &Client,
    sentence: &str,
    page_id: i64,
    page_index: usize,
) -> Result<()> {
    let embedding_resp = client
        .embeddings(EmbeddingsRequest::new(sentence.to_string()))
        .await?;
    let embedding = embedding_resp.data[0].embedding.clone();
    let embedding_json = serde_json::to_string(&embedding).into_diagnostic()?;

    let mut stmt = conn
        .prepare(
            "
        INSERT OR IGNORE INTO
        sentences
        (text, embedding, page_id, page_index)
        VALUES
        (?, vector_to_blob(vector_from_json(?)), ?, ?)",
        )
        .into_diagnostic()?;
    stmt.execute((sentence, embedding_json, page_id, page_index))
        .into_diagnostic()?;

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
