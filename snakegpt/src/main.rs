use std::{
    fs::File,
    io::{Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use aws_sdk_s3::primitives::ByteStream;
use clap::*;
use futures::{stream, StreamExt};

use itertools::Itertools;
use miette::{IntoDiagnostic, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use snakegpt::{
    fetch_embedding, respond_to, setup, Config, OpenAiClient,
    CONCURRENT_REQUESTS, DB_NAME,
};


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
    Download(DownloadArgs),
}

#[derive(Args, Debug)]
struct QueryArgs {
    query: String,
    #[arg(short = 'p', long, default_value = "false")]
    show_prompt: bool,
}

#[derive(Args, Debug)]
struct DownloadArgs {
    #[arg(short, long)]
    project: String,
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    match args.command {
        CliCommand::Prepare(args) => prepare(args).await,
        CliCommand::Query(args) => query(args).await,
        CliCommand::Download(args) => download(args).await,
    }
}

async fn query(args: QueryArgs) -> Result<()> {
    println!("Query: {}", &args.query);

    let conn = setup()?;
    let conn = Arc::new(Mutex::new(conn));
    let ans = respond_to(args.query.to_string(), conn).await?;

    println!("Answer: {}", ans.0);

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

    upload_db(&args).await?;

    Ok(())
}

async fn upload_db(args: &PrepareArgs) -> Result<()> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_s3::Client::new(&config);
    let file = ByteStream::from_path(Path::new(DB_NAME))
        .await
        .into_diagnostic()?;

    let path: PathBuf = args.path.clone();

    let path_name = path
        .file_name()
        .ok_or_else(|| miette::miette!("No file name found for path"))?
        .to_string_lossy()
        .to_string();

    let now = chrono::Utc::now();

    let key = format!("{path_name}/{now}/full.db", now = now.to_rfc2822());

    client
        .put_object()
        .bucket(std::env::var("S3_BUCKET").into_diagnostic()?)
        .key(&key)
        .body(file)
        .send()
        .await
        .into_diagnostic()?;

    //Upload a file called `latest` that points to the latest db
    let key_bytes: Vec<_> = key.as_bytes().to_vec();
    client
        .put_object()
        .bucket(std::env::var("S3_BUCKET").into_diagnostic()?)
        .key(format!("{path_name}/latest"))
        .body(ByteStream::from(key_bytes))
        .send()
        .await
        .into_diagnostic()?;

    Ok(())
}

async fn download(args: DownloadArgs) -> Result<()> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_s3::Client::new(&config);

    let resp = client
        .get_object()
        .bucket(std::env::var("S3_BUCKET").into_diagnostic()?)
        .key(format!("{path_name}/latest", path_name = args.project))
        .send()
        .await
        .into_diagnostic()?;

    let data = resp.body.collect().await.into_diagnostic()?;
    let latest_key = String::from_utf8(data.to_vec()).into_diagnostic()?;

    let resp = client
        .get_object()
        .bucket(std::env::var("S3_BUCKET").into_diagnostic()?)
        .key(latest_key)
        .send()
        .await
        .into_diagnostic()?;
    let data = resp.body.collect().await.into_diagnostic()?;

    let mut file = File::create(DB_NAME).into_diagnostic()?;
    file.write_all(&data.to_vec()).into_diagnostic()?;

    Ok(())
}

async fn embed_sentence(
    conn: &Connection,
    client: &OpenAiClient,
    sentence: &str,
    page_id: i64,
    page_index: usize,
) -> Result<()> {
    let row_id: Option<u64> = conn
        .prepare("SELECT rowid FROM sentences WHERE text = ?")
        .into_diagnostic()?
        .query_row(params![sentence], |row| row.get(0))
        .optional()
        .into_diagnostic()?;

    if row_id.is_none() {
        let embedding = fetch_embedding(client, sentence).await?;
        let embedding_json = serde_json::to_string(&embedding).into_diagnostic()?;

        let mut stmt = conn
            .prepare(
                "
        INSERT INTO
        sentences
        (text, embedding, page_id, page_index)
        VALUES
        (?, vector_to_blob(vector_from_json(?)), ?, ?)",
            )
            .into_diagnostic()?;
        stmt.execute((sentence, embedding_json, page_id, page_index))
            .into_diagnostic()?;
    }

    Ok(())
}
