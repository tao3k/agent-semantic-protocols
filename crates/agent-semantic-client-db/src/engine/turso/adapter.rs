// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Turso DB Engine adapter for `client.turso` state.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::engine::contract::{
    ClientDbBackend, ClientDbEngineBackend, ClientDbEngineDurability, ClientDbEngineFeatures,
};
use crate::engine::turso_statement::{execute_turso_statement, run_turso_operation};

use super::pool::{
    TursoConnectionLease, build_turso_database, shared_turso_read_only_connection,
    shared_turso_write_connection,
};

pub(super) const TURSO_CLIENT_DB_FILE: &str = "facts.turso";
pub(super) const TURSO_SEARCH_PROJECTION_DB_FILE: &str = "search-projection.turso";
const TURSO_CLIENT_DB_SCHEMA_VERSION: i64 = 1;
const TURSO_CLIENT_DB_PHYSICAL_FORMAT_ID: &str = "turso-0.7-native";
const TURSO_CLIENT_DB_FORMAT_RECEIPT_FILE: &str = "facts.turso.format.v1.json";
const TURSO_SEARCH_PROJECTION_DB_FORMAT_RECEIPT_FILE: &str =
    "search-projection.turso.format.v1.json";
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_PENDING: &str = "pending-cutover";
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY: &str = "ready";
pub(super) const TURSO_CLIENT_DB_INDEX_METHOD: bool = true;
pub(super) const TURSO_CLIENT_DB_MVCC_ENABLED: bool = true;
const TURSO_CLIENT_DB_BEGIN_CONCURRENT_ENABLED: bool = false;

/// Bootstrap metadata table used to record the Turso DB Engine schema version.
pub const TURSO_BOOTSTRAP_TABLE: &str = "asp_db_engine_bootstrap";

/// Diagnostic report for the Turso DB Engine backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoClientDbEngineReport {
    pub backend: &'static str,
    pub status: &'static str,
    pub db_file_name: &'static str,
    pub schema_version: i64,
    pub schema_bootstrap: &'static str,
    pub durability: &'static str,
    pub features: ClientDbEngineFeatures,
    pub db_path: PathBuf,
    pub reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::engine) struct TursoClientDbEngineBackend;

impl ClientDbEngineBackend for TursoClientDbEngineBackend {
    type Connection = ();
    type Report = TursoClientDbEngineReport;

    fn backend(&self) -> ClientDbBackend {
        ClientDbBackend::Turso
    }

    fn db_file_name(&self) -> &'static str {
        TURSO_CLIENT_DB_FILE
    }

    fn schema_version(&self) -> i64 {
        TURSO_CLIENT_DB_SCHEMA_VERSION
    }

    fn durability(&self) -> ClientDbEngineDurability {
        ClientDbEngineDurability::TursoLocalFile
    }

    fn features(&self) -> ClientDbEngineFeatures {
        ClientDbEngineFeatures {
            async_io: true,
            concurrent_writes: true,
            fts: true,
            fts_index_method: TURSO_CLIENT_DB_INDEX_METHOD,
            vector: false,
            overlay_search: true,
            sync: false,
            encryption: false,
            mvcc: TURSO_CLIENT_DB_MVCC_ENABLED,
            begin_concurrent: TURSO_CLIENT_DB_BEGIN_CONCURRENT_ENABLED,
        }
    }

    fn inspect(&self, db_path: &Path) -> TursoClientDbEngineReport {
        let active_db_path = db_path.with_file_name(self.db_file_name());
        let (status, schema_bootstrap, reason) = if active_db_path.exists() {
            ("ready", TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY, None)
        } else {
            ("missing", TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_PENDING, None)
        };
        TursoClientDbEngineReport {
            backend: self.backend().as_str(),
            status,
            db_file_name: self.db_file_name(),
            schema_version: self.schema_version(),
            schema_bootstrap,
            durability: self.durability().as_str(),
            features: self.features(),
            db_path: active_db_path,
            reason,
        }
    }
}

async fn turso_physical_format_is_current(
    connection: &mrr_data_backend::turso_driver::Connection,
) -> Result<bool, String> {
    let mut rows = connection
        .query(
            "SELECT 1
             FROM asp_db_engine_format
             WHERE format_id = ?1
               AND turso_version = ?2
               AND logical_schema_version = ?3
             LIMIT 1",
            (
                TURSO_CLIENT_DB_PHYSICAL_FORMAT_ID,
                "0.7",
                TURSO_CLIENT_DB_SCHEMA_VERSION,
            ),
        )
        .await
        .map_err(|error| format!("failed to inspect Turso physical-format authority: {error}"))?;
    rows.next()
        .await
        .map(|row| row.is_some())
        .map_err(|error| format!("failed to read Turso physical-format authority: {error}"))
}

pub(in crate::engine) fn prepare_turso_client_db_path(db_path: &Path) -> Result<PathBuf, String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create Turso client DB dir: {error}"))?;
    }
    let turso_path = db_path.with_file_name(TURSO_CLIENT_DB_FILE);
    ensure_no_active_turso_migration(&turso_path)?;
    ensure_turso_0_7_format_receipt(&turso_path)?;
    Ok(turso_path)
}

pub(in crate::engine) fn turso_0_7_format_receipt_path(turso_path: &Path) -> PathBuf {
    let receipt_file = if turso_path.file_name().and_then(|name| name.to_str())
        == Some(TURSO_SEARCH_PROJECTION_DB_FILE)
    {
        TURSO_SEARCH_PROJECTION_DB_FORMAT_RECEIPT_FILE
    } else {
        TURSO_CLIENT_DB_FORMAT_RECEIPT_FILE
    };
    turso_path.with_file_name(receipt_file)
}

fn ensure_turso_0_7_format_receipt(turso_path: &Path) -> Result<(), String> {
    if !turso_path.exists() {
        return Ok(());
    }
    let receipt_path = turso_0_7_format_receipt_path(turso_path);
    if !receipt_path.is_file() {
        return Err(format!(
            "existing client DB `{}` has no Turso 0.7 format receipt `{}`; full staging migration is required and in-place compatibility bootstrap is forbidden",
            turso_path.display(),
            receipt_path.display()
        ));
    }
    Ok(())
}

fn ensure_no_active_turso_migration(turso_path: &Path) -> Result<(), String> {
    let Some(client_dir) = turso_path.parent() else {
        return Ok(());
    };
    let marker_path =
        client_dir.join(crate::engine::turso_migration::TURSO_0_7_ACTIVE_MIGRATION_MARKER_FILE);
    if marker_path.is_file() {
        return Err(format!(
            "active Turso 0.7 cutover is in progress for `{}`; retry after migration completes",
            client_dir.display()
        ));
    }
    Ok(())
}

pub(in crate::engine) fn write_turso_0_7_format_receipt(turso_path: &Path) -> Result<(), String> {
    let receipt_path = turso_0_7_format_receipt_path(turso_path);
    let temporary_path = receipt_path.with_extension(format!(
        "json.tmp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("failed to timestamp Turso 0.7 format receipt: {error}"))?
            .as_nanos()
    ));
    let receipt = serde_json::to_vec_pretty(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.turso-client-db-format-receipt",
        "schemaVersion": "1",
        "physicalFormat": TURSO_CLIENT_DB_PHYSICAL_FORMAT_ID,
        "tursoVersion": "0.7",
        "logicalSchemaVersion": TURSO_CLIENT_DB_SCHEMA_VERSION,
        "migrationId": "client-db-v1-project-partition-cutover",
    }))
    .map_err(|error| format!("failed to encode Turso 0.7 format receipt: {error}"))?;
    std::fs::write(&temporary_path, receipt).map_err(|error| {
        format!(
            "failed to write Turso 0.7 format receipt `{}`: {error}",
            temporary_path.display()
        )
    })?;
    std::fs::rename(&temporary_path, &receipt_path).map_err(|error| {
        format!(
            "failed to promote Turso 0.7 format receipt `{}`: {error}",
            receipt_path.display()
        )
    })
}

pub(in crate::engine) async fn bootstrap_turso_schema_version(
    connection: &mut mrr_data_backend::turso_driver::Connection,
) -> Result<(), String> {
    execute_turso_statement(
        connection,
        "CREATE TABLE IF NOT EXISTS asp_db_engine_bootstrap (schema_version INTEGER NOT NULL)",
        "failed to bootstrap Turso client DB schema",
    )
    .await?;

    let current_version = {
        let mut rows = connection
            .query(
                "SELECT MAX(schema_version) FROM asp_db_engine_bootstrap",
                (),
            )
            .await
            .map_err(|error| format!("failed to read Turso bootstrap schema row: {error}"))?;
        rows.next()
            .await
            .map_err(|error| format!("failed to advance Turso bootstrap schema row: {error}"))?
            .map(|row| row.get::<Option<i64>>(0))
            .transpose()
            .map_err(|error| format!("failed to decode Turso bootstrap schema row: {error}"))?
            .flatten()
    };

    if let Some(version) = current_version {
        if version > TURSO_CLIENT_DB_SCHEMA_VERSION {
            return Err(format!(
                "unsupported newer Turso client DB schema version {version}; maximum supported version is {TURSO_CLIENT_DB_SCHEMA_VERSION}"
            ));
        }
        if version == TURSO_CLIENT_DB_SCHEMA_VERSION {
            let mut complete = true;
            for table in [
                "asp_db_engine_migration",
                "asp_db_engine_format",
                "asp_artifact_pointer",
                "asp_failed_artifact_attempt",
            ] {
                if !turso_table_exists(connection, table).await? {
                    complete = false;
                }
            }
            if complete && turso_physical_format_is_current(connection).await? {
                return Ok(());
            }
        }
        if version != 1 {
            return Err(format!(
                "unsupported older Turso client DB schema version {version}; expected version 1 for migration"
            ));
        }
    }

    let transaction = connection
        .transaction_with_behavior(
            mrr_data_backend::turso_driver::transaction::TransactionBehavior::Immediate,
        )
        .await
        .map_err(|error| {
            format!("failed to begin Turso client DB schema stabilization: {error}")
        })?;
    let stabilization = async {
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS asp_db_engine_migration (\
                    schema_version INTEGER PRIMARY KEY,\
                    migration_id TEXT NOT NULL UNIQUE,\
                    applied_at_ms INTEGER NOT NULL\
                 )",
            )
            .await
            .map_err(|error| format!("failed to create Turso schema history: {error}"))?;
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS asp_db_engine_format (\
                    format_id TEXT PRIMARY KEY,\
                    turso_version TEXT NOT NULL,\
                    logical_schema_version INTEGER NOT NULL,\
                    migration_id TEXT NOT NULL,\
                    promoted_at_ms INTEGER NOT NULL\
                 )",
            )
            .await
            .map_err(|error| {
                format!("failed to create Turso physical-format authority: {error}")
            })?;
        transaction
            .execute(
                "INSERT OR REPLACE INTO asp_db_engine_format (\
                    format_id, turso_version, logical_schema_version, migration_id, promoted_at_ms\
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    TURSO_CLIENT_DB_PHYSICAL_FORMAT_ID,
                    "0.7",
                    TURSO_CLIENT_DB_SCHEMA_VERSION,
                    "client-db-v1-project-partition-cutover",
                    0_i64,
                ),
            )
            .await
            .map_err(|error| {
                format!("failed to record Turso 0.7 physical-format authority: {error}")
            })?;
        transaction
            .execute_batch(crate::artifact_pointer_store::CREATE_SCHEMA_SQL)
            .await
            .map_err(|error| format!("failed to stabilize artifact authority schema: {error}"))?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO asp_db_engine_migration (\
                    schema_version, migration_id, applied_at_ms\
                 ) VALUES (?1, ?2, ?3)",
                (
                    TURSO_CLIENT_DB_SCHEMA_VERSION,
                    "client-db-v1-stable-artifact-authority",
                    0_i64,
                ),
            )
            .await
            .map_err(|error| format!("failed to record Turso client DB stabilization: {error}"))?;
        transaction
            .execute("DELETE FROM asp_db_engine_bootstrap", ())
            .await
            .map_err(|error| format!("failed to replace Turso bootstrap schema row: {error}"))?;
        transaction
            .execute(
                "INSERT INTO asp_db_engine_bootstrap (schema_version) VALUES (?1)",
                [TURSO_CLIENT_DB_SCHEMA_VERSION],
            )
            .await
            .map_err(|error| format!("failed to write Turso bootstrap schema row: {error}"))?;
        Ok::<(), String>(())
    }
    .await;

    match stabilization {
        Ok(()) => transaction.commit().await.map_err(|error| {
            format!("failed to commit Turso client DB schema stabilization: {error}")
        }),
        Err(error) => {
            let rollback = transaction.rollback().await;
            match rollback {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(format!(
                    "{error}; additionally failed to roll back Turso client DB schema stabilization: {rollback_error}"
                )),
            }
        }
    }
}

pub(in crate::engine) fn turso_bootstrap_report(db_path: &Path) -> TursoClientDbEngineReport {
    let backend = TursoClientDbEngineBackend;
    let mut report = backend.inspect(db_path);
    report.status = "bootstrap-smoke";
    report.schema_bootstrap = TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY;
    report.reason = None;
    report
}

pub(in crate::engine) async fn open_turso_client_db_read_only(
    turso_path: PathBuf,
) -> Result<mrr_data_backend::turso_driver::Connection, String> {
    ensure_no_active_turso_migration(&turso_path)?;
    ensure_turso_0_7_format_receipt(&turso_path)?;
    shared_turso_read_only_connection(&turso_path).await
}

pub(in crate::engine) async fn validate_turso_0_7_migration_target(
    turso_path: PathBuf,
) -> Result<(), String> {
    ensure_turso_0_7_format_receipt(&turso_path)?;
    let connection = shared_turso_read_only_connection(&turso_path).await?;
    drop(connection);
    Ok(())
}

/// Open an unreceipted legacy file only for bounded, read-only Turso 0.7 migration.
pub(in crate::engine) async fn open_turso_0_7_migration_source(
    turso_path: &Path,
) -> Result<
    (
        mrr_data_backend::turso_driver::Database,
        mrr_data_backend::turso_driver::Connection,
    ),
    String,
> {
    let database = build_turso_database(turso_path).await?;
    let connection = database
        .connect()
        .map_err(|error| format!("failed to connect Turso 0.7 migration source: {error}"))?;
    connection
        .execute("PRAGMA query_only = 1", ())
        .await
        .map_err(|error| {
            format!("failed to enforce read-only Turso 0.7 migration source: {error}")
        })?;
    Ok((database, connection))
}

pub(in crate::engine) fn turso_client_db_exists(db_path: &Path) -> bool {
    db_path.with_file_name(TURSO_CLIENT_DB_FILE).exists()
}

pub(in crate::engine) async fn connect_turso_client_db(
    db_path: &Path,
) -> Result<TursoConnectionLease, String> {
    let turso_path = db_path.with_file_name(TURSO_CLIENT_DB_FILE);
    shared_turso_write_connection(&turso_path).await
}

pub(in crate::engine) fn turso_search_projection_db_path(db_path: &Path) -> PathBuf {
    db_path.with_file_name(TURSO_SEARCH_PROJECTION_DB_FILE)
}

pub(in crate::engine) async fn connect_turso_search_projection_db_for_write(
    db_path: &Path,
) -> Result<TursoConnectionLease, String> {
    shared_turso_write_connection(&turso_search_projection_db_path(db_path)).await
}

pub(in crate::engine) async fn connect_turso_search_projection_db_read_only(
    db_path: &Path,
) -> Result<mrr_data_backend::turso_driver::Connection, String> {
    shared_turso_read_only_connection(&turso_search_projection_db_path(db_path)).await
}

pub(in crate::engine) async fn connect_turso_client_db_read_only(
    db_path: &Path,
) -> Result<mrr_data_backend::turso_driver::Connection, String> {
    let turso_path = db_path.with_file_name(TURSO_CLIENT_DB_FILE);
    open_turso_client_db_read_only(turso_path).await
}

pub(in crate::engine) async fn turso_table_exists(
    connection: &mrr_data_backend::turso_driver::Connection,
    table_name: &str,
) -> Result<bool, String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query("PRAGMA table_list", ())
                .await
                .map_err(|error| error.to_string())
        },
        &format!("failed to inspect Turso table {table_name}"),
    )
    .await?;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso table {table_name} existence: {error}"))?
    {
        let name = row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso table list name: {error}"))?;
        if name == table_name {
            return Ok(true);
        }
    }
    Ok(false)
}
