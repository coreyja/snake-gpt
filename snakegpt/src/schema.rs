use miette::{IntoDiagnostic, Result};
use rusqlite::Connection;

pub(crate) fn setup_schema_v0(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sentences (
                  text                  TEXT NOT NULL,
                  embedding             fvector
                  )",
        (),
    )
    .into_diagnostic()?;

    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS uniq_index_text on sentences (text);",
        (),
    )
    .into_diagnostic()?;

    Ok(())
}
