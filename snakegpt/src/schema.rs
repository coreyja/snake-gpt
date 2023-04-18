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

    conn.execute_batch(
        "
  DROP TABLE IF EXISTS vss_sentences;
  create virtual table vss_sentences using vss0(
      embedding(1536),
    );
  ",
    )
    .into_diagnostic()?;

    conn.execute(
        "insert into vss_sentences(rowid, embedding)
  select rowid, embedding from sentences;",
        (),
    )
    .into_diagnostic()?;

    Ok(())
}
