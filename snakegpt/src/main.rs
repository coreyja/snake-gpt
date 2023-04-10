use clap::Parser;
use futures::{stream, StreamExt};
use itertools::Itertools;
use std::path::PathBuf;

use miette::{IntoDiagnostic, Result};
use openai::Client;
use rusqlite::{params, Connection, Row};

use crate::openai::{embeddings::EmbeddingsRequest, Config};

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

const CONCURRENT_REQUESTS: usize = 5;

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

    let pages = walkdir::WalkDir::new(&args.path)
        .into_iter()
        .filter(|entry| {
            let entry = entry.as_ref().unwrap();
            let path = entry.path();

            entry.file_type().is_file()
                && !path.to_string_lossy().contains("node_modules")
                && path
                    .extension()
                    .map(|ext| ext.to_str() == Some("md"))
                    .unwrap_or(false)
        })
        .collect_vec();

    println!("Found {} pages", pages.len());

    let bodies = stream::iter(pages)
        .map(|page| {
            let client = &client;
            let conn = &conn;
            let page = page.unwrap();
            let path: PathBuf = page.path().into();
            async move {
                println!("About to Process Path: {}", path.display());

                let display_path = path.display().to_string();

                let page_id = conn.query_row(
                    "INSERT OR IGNORE INTO pages (path) VALUES (?) returning rowid",
                    params![display_path],
                    |row: &Row| -> Result<i64, _> { row.get(0) },
                );

                let (page_id, parsed_text) = match page_id {
                    Ok(id) => (id, None),
                    Err(_) => conn
                        .query_row(
                            "SELECT rowid, parsed_text FROM pages WHERE path = ?",
                            params![display_path],
                            |row: &Row| -> Result<(i64, Option<String>), _> {
                                Ok((row.get(0)?, row.get(1)?))
                            },
                        )
                        .into_diagnostic()?,
                };

                let parsed_text = if let Some(parsed_text) = parsed_text {
                    parsed_text
                } else {
                    let content = std::fs::read_to_string(path).into_diagnostic()?;
                    let sentences = client.split_by_sentences(&content).await?;

                    let parsed_text = sentences.join("\n\n");

                    conn.execute(
                        "
                    UPDATE pages SET parsed_text = ? where rowid = ?",
                        (&parsed_text, page_id),
                    )
                    .into_diagnostic()?;

                    parsed_text
                };

                Result::<_>::Ok((page_id, parsed_text))
            }
        })
        .buffer_unordered(CONCURRENT_REQUESTS);

    bodies
        .for_each(|b| async {
            match b {
                Ok((pid, _parsed_text)) => println!("Processed page with id {pid}"),
                Err(e) => eprintln!("Got an error: {}", e),
            }
        })
        .await;

    let mut st = conn
        .prepare("SELECT rowid, parsed_text FROM pages")
        .into_diagnostic()?;
    let rows = st
        .query_map(params![], |row| {
            let id: i64 = row.get(0)?;
            let parsed_text: String = row.get(1)?;
            Ok((id, parsed_text))
        })
        .into_diagnostic()?;

    stream::iter(rows)
        .map(|row| {
            let client = &client;
            let conn = &conn;

            async move {
                let (page_id, text) = row.into_diagnostic()?;
                for (i, sentence) in text.split("\n\n").enumerate() {
                    // TODO: Skip if already embedded
                    embed_sentence(conn, client, sentence, page_id, i).await?;
                }

                Result::<_>::Ok(page_id)
            }
        })
        .buffer_unordered(CONCURRENT_REQUESTS)
        .for_each(|b| async {
            match b {
                Ok(pid) => println!("Embedded page with id {pid}"),
                Err(e) => eprintln!("Got an error: {}", e),
            }
        })
        .await;

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
