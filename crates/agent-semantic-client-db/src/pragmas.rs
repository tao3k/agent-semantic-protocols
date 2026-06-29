//! SQLite pragma configuration and runtime diagnostics for the client DB.

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;
use serde::Serialize;

const CLIENT_DB_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// SQLite journal mode observed on a local client DB connection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ClientDbJournalMode(String);

impl ClientDbJournalMode {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Runtime SQLite pragma values observed on a local client DB connection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbRuntimePragmas {
    pub journal_mode: ClientDbJournalMode,
    pub synchronous: i64,
    pub busy_timeout_ms: i64,
    pub foreign_keys: bool,
}

pub(crate) fn configure_readable_connection(
    conn: &Connection,
    db_path: &Path,
) -> Result<(), String> {
    conn.busy_timeout(CLIENT_DB_BUSY_TIMEOUT).map_err(|error| {
        format!(
            "failed to configure agent semantic client db busy timeout at {}: {error}",
            db_path.display()
        )
    })?;

    conn.execute_batch(
        "
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        ",
    )
    .map_err(|error| {
        format!(
            "failed to configure agent semantic client db pragmas at {}: {error}",
            db_path.display()
        )
    })?;
    Ok(())
}

pub(crate) fn configure_writable_connection(
    conn: &Connection,
    db_path: &Path,
) -> Result<(), String> {
    configure_readable_connection(conn, db_path)?;

    let journal_mode = query_string_pragma(conn, "journal_mode = WAL", db_path)?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(format!(
            "failed to enable WAL journal mode for agent semantic client db at {}: sqlite returned {journal_mode}",
            db_path.display()
        ));
    }
    Ok(())
}

pub(crate) fn read_runtime_pragmas(
    conn: &Connection,
    db_path: &Path,
) -> Result<ClientDbRuntimePragmas, String> {
    Ok(ClientDbRuntimePragmas {
        journal_mode: ClientDbJournalMode(query_string_pragma(conn, "journal_mode", db_path)?),
        synchronous: query_i64_pragma(conn, "synchronous", db_path)?,
        busy_timeout_ms: query_i64_pragma(conn, "busy_timeout", db_path)?,
        foreign_keys: query_i64_pragma(conn, "foreign_keys", db_path)? != 0,
    })
}

fn query_string_pragma(conn: &Connection, pragma: &str, db_path: &Path) -> Result<String, String> {
    conn.query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))
        .map_err(|error| {
            format!(
                "failed to read agent semantic client db pragma {pragma} at {}: {error}",
                db_path.display()
            )
        })
}

fn query_i64_pragma(conn: &Connection, pragma: &str, db_path: &Path) -> Result<i64, String> {
    conn.query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))
        .map_err(|error| {
            format!(
                "failed to read agent semantic client db pragma {pragma} at {}: {error}",
                db_path.display()
            )
        })
}
