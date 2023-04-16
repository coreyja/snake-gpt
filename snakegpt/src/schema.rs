use miette::{IntoDiagnostic, Result};
use rusqlite::Connection;

pub fn setup_schema_v0(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS pages (
            path                  TEXT NOT NULL,
            parsed_text           TEXT
        )",
        (),
    )
    .into_diagnostic()?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS uniq_index_pages_path on pages (path);",
        (),
    )
    .into_diagnostic()?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sentences (
                  page_id               INTEGER NOT NULL,
                  page_index            INTEGER NOT NULL,
                  text                  TEXT NOT NULL,
                  embedding             fvector
                  )",
        (),
    )
    .into_diagnostic()?;

    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS uniq_index_sentences_text on sentences (text);",
        (),
    )
    .into_diagnostic()?;

    Ok(())
}
