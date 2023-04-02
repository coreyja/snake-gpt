use miette::{IntoDiagnostic, Result};
use rusqlite::Connection;

use crate::openai::{
    embeddings::{EmbeddingResponse, EmbeddingsRequest},
    Config,
};

mod openai;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[tokio::main]
async fn main() -> Result<()> {
    // let config = Config::from_env()?;

    // let client = config.client()?;

    // let response_body = client.embeddings("Hello world!").await?;

    // dbg!(&response_body);

    // Ok(())

    let conn = Connection::open_in_memory().into_diagnostic()?;

    load_my_extension(&conn)?;

    conn.query_row("select sqlite_version()", (), |result| {
        dbg!(&result);
        Ok(())
    })
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
