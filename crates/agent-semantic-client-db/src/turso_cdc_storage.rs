//! Non-MVCC Turso Change Data Capture authority.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const TURSO_CDC_PAGE_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.client-db.turso-cdc-page-receipt.v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TursoCdcCaptureMode {
    Id,
    Before,
    After,
    Full,
}

impl TursoCdcCaptureMode {
    fn as_pragma_value(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Before => "before",
            Self::After => "after",
            Self::Full => "full",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoCdcProfileConfig {
    pub path: PathBuf,
    pub mode: TursoCdcCaptureMode,
    pub table_name: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TursoCdcChangeKind {
    Delete,
    Update,
    Insert,
    Commit,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoCdcChange {
    pub change_id: i64,
    pub change_time: i64,
    pub change_txn_id: i64,
    pub kind: TursoCdcChangeKind,
    pub raw_change_type: i64,
    pub table_name: Option<String>,
    pub row_id: Option<String>,
    pub before: Option<Vec<u8>>,
    pub after: Option<Vec<u8>>,
    pub updates: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoCdcPageReceipt {
    pub schema_id: String,
    pub profile: String,
    pub capture_mode: TursoCdcCaptureMode,
    pub after_change_id: Option<i64>,
    pub limit: usize,
    pub has_more: bool,
    pub next_change_id: Option<i64>,
    pub changes: Vec<TursoCdcChange>,
}

pub struct TursoCdcStorage {
    _database: turso::Database,
    connection: turso::Connection,
    mode: TursoCdcCaptureMode,
    table_name: String,
}

impl TursoCdcStorage {
    pub async fn open(config: TursoCdcProfileConfig) -> Result<Self, String> {
        validate_table_name(&config.table_name)?;
        let path = config.path.to_string_lossy();
        let database = turso::Builder::new_local(path.as_ref())
            .experimental_multiprocess_wal(true)
            .build()
            .await
            .map_err(|error| format!("failed to open non-MVCC CDC database: {error}"))?;
        let connection = database
            .connect()
            .map_err(|error| format!("failed to connect non-MVCC CDC database: {error}"))?;
        let pragma = format!(
            "PRAGMA capture_data_changes_conn('{},{}')",
            config.mode.as_pragma_value(),
            config.table_name
        );
        connection
            .execute_batch(&pragma)
            .await
            .map_err(|error| format!("failed to enable Turso CDC profile: {error}"))?;
        Ok(Self {
            _database: database,
            connection,
            mode: config.mode,
            table_name: config.table_name,
        })
    }

    pub fn connection(&self) -> turso::Connection {
        self.connection.clone()
    }

    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    pub async fn read_page(
        &self,
        after_change_id: Option<i64>,
        limit: usize,
    ) -> Result<TursoCdcPageReceipt, String> {
        if !(1..=1_000).contains(&limit) {
            return Err("CDC page limit must be between 1 and 1000".to_owned());
        }
        let query = format!(
            r#"SELECT change_id, change_time, change_txn_id, change_type, table_name,
                       CAST(id AS TEXT), "before", "after", "updates"
                FROM {}
                WHERE change_id > ?1
                ORDER BY change_id ASC
                LIMIT ?2"#,
            self.table_name
        );
        let mut rows = self
            .connection
            .query(&query, (after_change_id.unwrap_or(0), (limit + 1) as i64))
            .await
            .map_err(|error| format!("failed to query Turso CDC page: {error}"))?;
        let mut changes = Vec::with_capacity(limit + 1);
        while let Some(row) = rows
            .next()
            .await
            .map_err(|error| format!("failed to advance Turso CDC page: {error}"))?
        {
            let raw_change_type = row
                .get::<i64>(3)
                .map_err(|error| format!("failed to decode CDC change type: {error}"))?;
            changes.push(TursoCdcChange {
                change_id: row
                    .get(0)
                    .map_err(|error| format!("failed to decode CDC change ID: {error}"))?,
                change_time: row
                    .get(1)
                    .map_err(|error| format!("failed to decode CDC change time: {error}"))?,
                change_txn_id: row
                    .get(2)
                    .map_err(|error| format!("failed to decode CDC transaction ID: {error}"))?,
                kind: match raw_change_type {
                    -1 => TursoCdcChangeKind::Delete,
                    0 => TursoCdcChangeKind::Update,
                    1 => TursoCdcChangeKind::Insert,
                    2 => TursoCdcChangeKind::Commit,
                    _ => TursoCdcChangeKind::Unknown,
                },
                raw_change_type,
                table_name: row
                    .get(4)
                    .map_err(|error| format!("failed to decode CDC table name: {error}"))?,
                row_id: row
                    .get(5)
                    .map_err(|error| format!("failed to decode CDC row ID: {error}"))?,
                before: row
                    .get(6)
                    .map_err(|error| format!("failed to decode CDC before image: {error}"))?,
                after: row
                    .get(7)
                    .map_err(|error| format!("failed to decode CDC after image: {error}"))?,
                updates: row
                    .get(8)
                    .map_err(|error| format!("failed to decode CDC update image: {error}"))?,
            });
        }
        let has_more = changes.len() > limit;
        if has_more {
            changes.truncate(limit);
        }
        let next_change_id = changes.last().map(|change| change.change_id);
        Ok(TursoCdcPageReceipt {
            schema_id: TURSO_CDC_PAGE_RECEIPT_SCHEMA_ID.to_owned(),
            profile: "cdc-non-mvcc".to_owned(),
            capture_mode: self.mode,
            after_change_id,
            limit,
            has_more,
            next_change_id,
            changes,
        })
    }
}

fn validate_table_name(table_name: &str) -> Result<(), String> {
    let valid = !table_name.is_empty()
        && table_name.len() <= 64
        && table_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if valid {
        Ok(())
    } else {
        Err("CDC table name must contain only ASCII letters, digits, and underscore".to_owned())
    }
}
