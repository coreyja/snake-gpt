use clap::{Args, Parser, Subcommand};
use futures::{stream, StreamExt};
use indoc::formatdoc;
use itertools::Itertools;
use std::{os::unix::prelude::PermissionsExt, path::PathBuf};

use miette::{IntoDiagnostic, Result};
use openai::Client;
use rusqlite::{params, Connection, Row};

use crate::openai::{completion::CompletionRequest, embeddings::EmbeddingsRequest, Config};

mod openai;
mod schema;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Args, Debug)]

struct PrepareArgs {
    /// Path to search for Markdown files
    #[arg(short, long)]
    path: PathBuf,
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    Prepare(PrepareArgs),
    Query(QueryArgs),
}

#[derive(Args, Debug)]
struct QueryArgs {
    query: String,
    #[arg(short = 'p', long, default_value = "false")]
    show_prompt: bool,
}

/// SnakeGPT
///
/// A chatbot that uses the Battlesnake Docs site and related content
/// to generate responses to questions about Battlesnake.
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct CliArgs {
    #[clap(subcommand)]
    command: CliCommand,
}

const CONCURRENT_REQUESTS: usize = 5;

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    match args.command {
        CliCommand::Prepare(args) => prepare(args).await,
        CliCommand::Query(args) => query(args).await,
    }
}

const DB_NAME: &str = "sample.v0.db";

async fn query(args: QueryArgs) -> Result<()> {
    let conn = setup()?;

    let config = Config::from_env()?;
    let client = config.client()?;

    println!("Query: {}", &args.query);
    println!("About to fetch Embeddings for query");

    let question = &args.query;
    let embedding = fetch_embedding(&client, question).await?;
    let embedding_json = serde_json::to_string(&embedding).into_diagnostic()?;

    println!("Retrieved Embeddings. Finding related content");

    println!("About to make VSS Table");
    conn.execute_batch(
        "
        DROP TABLE IF EXISTS vss_sentences;
        create virtual table vss_sentences using vss0(
            embedding(1536),
          );
        ",
    )
    .into_diagnostic()?;

    println!("About to populate VSS Table");
    conn.execute(
        "insert into vss_sentences(rowid, embedding)
        select rowid, embedding from sentences;",
        (),
    )
    .into_diagnostic()?;

    println!("About to query VSS Table");

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

    println!("Found related content, creating Prompt");

    let context_strings = nearest_embeddings
        .iter()
        .map(|(text, _)| format!("- {}", text.trim()))
        .join("\n");
    let context_section = format!("### Context\n{context_strings}");

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
        {context_section}

        --------------------------------------

        {question}
        "
    );

    if args.show_prompt {
        println!("Prompt:\n{}", &prompt);
    }

    let completion_request = CompletionRequest::gpt_3_5_turbo(&prompt);
    let answer = client.completion(completion_request).await?;

    let first_choice = &answer.choices.first().unwrap().message.content;

    println!("Answer: {}", first_choice);

    Ok(())
}

async fn prepare(args: PrepareArgs) -> Result<()> {
    let conn = setup()?;

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

fn setup() -> Result<Connection> {
    let conn = Connection::open(DB_NAME).into_diagnostic()?;
    load_my_extension(&conn)?;
    conn.query_row("select vss_version()", (), |result| {
        dbg!(&result);
        Ok(())
    })
    .into_diagnostic()?;
    schema::setup_schema_v0(&conn)?;
    Ok(conn)
}

async fn embed_sentence(
    conn: &Connection,
    client: &Client,
    sentence: &str,
    page_id: i64,
    page_index: usize,
) -> Result<()> {
    let embedding = fetch_embedding(client, sentence).await?;
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

async fn fetch_embedding(client: &Client, sentence: &str) -> Result<Vec<f64>, miette::ErrReport> {
    let embedding_resp = client
        .embeddings(EmbeddingsRequest::new(sentence.to_string()))
        .await?;
    let embedding = embedding_resp.data[0].embedding.clone();
    Ok(embedding)
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
