// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Shared Turso database authority, write lanes and connection leases.

use std::path::Path;

use super::adapter::{
    TURSO_CLIENT_DB_FILE, TURSO_CLIENT_DB_INDEX_METHOD, TURSO_CLIENT_DB_MVCC_ENABLED,
    TURSO_SEARCH_PROJECTION_DB_FILE,
};

fn turso_builder(turso_path: &Path) -> mrr_data_backend::turso_driver::Builder {
    mrr_data_backend::turso_driver::Builder::new_local(turso_path.to_string_lossy().as_ref())
        .experimental_index_method(TURSO_CLIENT_DB_INDEX_METHOD)
}

type TursoSchemaState = std::sync::Arc<
    tokio::sync::Mutex<
        std::collections::HashMap<&'static str, std::sync::Arc<tokio::sync::OnceCell<()>>>,
    >,
>;

/// A connection paired with the shared database authority that created it.
pub(in crate::engine) struct TursoConnectionLease {
    _database: std::sync::Arc<mrr_data_backend::turso_driver::Database>,
    connection: tokio::sync::OwnedMutexGuard<mrr_data_backend::turso_driver::Connection>,
    schema_state: TursoSchemaState,
}

impl TursoConnectionLease {
    pub(in crate::engine) async fn schema_bootstrap_state(
        &self,
        schema_id: &'static str,
    ) -> std::sync::Arc<tokio::sync::OnceCell<()>> {
        let mut states = self.schema_state.lock().await;
        std::sync::Arc::clone(
            states
                .entry(schema_id)
                .or_insert_with(|| std::sync::Arc::new(tokio::sync::OnceCell::new())),
        )
    }
}

impl std::ops::Deref for TursoConnectionLease {
    type Target = mrr_data_backend::turso_driver::Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl std::ops::DerefMut for TursoConnectionLease {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

struct TursoDatabasePoolEntry {
    database: tokio::sync::OnceCell<std::sync::Arc<mrr_data_backend::turso_driver::Database>>,
    write_lanes: tokio::sync::RwLock<
        Vec<std::sync::Arc<tokio::sync::Mutex<mrr_data_backend::turso_driver::Connection>>>,
    >,
    next_write_lane: std::sync::atomic::AtomicUsize,
    schema_state: TursoSchemaState,
}

type TursoDatabasePool =
    std::collections::BTreeMap<std::path::PathBuf, std::sync::Arc<TursoDatabasePoolEntry>>;

fn turso_database_pool() -> &'static tokio::sync::Mutex<TursoDatabasePool> {
    static POOL: std::sync::OnceLock<tokio::sync::Mutex<TursoDatabasePool>> =
        std::sync::OnceLock::new();
    POOL.get_or_init(|| tokio::sync::Mutex::new(TursoDatabasePool::new()))
}

pub(crate) async fn shared_turso_database(
    turso_path: &Path,
) -> Result<std::sync::Arc<mrr_data_backend::turso_driver::Database>, String> {
    let entry = shared_turso_pool_entry(turso_path).await;
    let database = entry
        .database
        .get_or_try_init(|| async {
            build_turso_database(turso_path)
                .await
                .map(std::sync::Arc::new)
        })
        .await?;
    Ok(std::sync::Arc::clone(database))
}

async fn shared_turso_pool_entry(turso_path: &Path) -> std::sync::Arc<TursoDatabasePoolEntry> {
    let mut pool = turso_database_pool().lock().await;
    std::sync::Arc::clone(pool.entry(turso_path.to_path_buf()).or_insert_with(|| {
        std::sync::Arc::new(TursoDatabasePoolEntry {
            database: tokio::sync::OnceCell::new(),
            write_lanes: tokio::sync::RwLock::new(Vec::new()),
            next_write_lane: std::sync::atomic::AtomicUsize::new(0),
            schema_state: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        })
    }))
}

pub(in crate::engine) async fn evict_turso_client_dir(client_dir: &Path) {
    let mut pool = turso_database_pool().lock().await;
    pool.remove(&client_dir.join(TURSO_CLIENT_DB_FILE));
    pool.remove(&client_dir.join(TURSO_SEARCH_PROJECTION_DB_FILE));
}

async fn configure_turso_write_connection(
    connection: &mrr_data_backend::turso_driver::Connection,
    mvcc_enabled: bool,
) -> Result<(), String> {
    if !mvcc_enabled {
        return Ok(());
    }

    let mut rows = connection
        .query("PRAGMA journal_mode = 'mvcc'", ())
        .await
        .map_err(|error| format!("failed to enable Turso client DB MVCC: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso client DB journal mode: {error}"))?
        .ok_or_else(|| "Turso client DB journal mode returned no row".to_string())?;
    let journal_mode = row
        .get::<String>(0)
        .map_err(|error| format!("failed to decode Turso client DB journal mode: {error}"))?;
    if journal_mode != "mvcc" {
        return Err(format!(
            "Turso client DB requires journal_mode=mvcc, observed {journal_mode}"
        ));
    }
    Ok(())
}

fn adaptive_turso_write_lane_ceiling() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
}

async fn new_turso_write_lane(
    database: &mrr_data_backend::turso_driver::Database,
    turso_path: &Path,
) -> Result<std::sync::Arc<tokio::sync::Mutex<mrr_data_backend::turso_driver::Connection>>, String>
{
    let connection = database
        .connect()
        .map_err(|error| format!("failed to connect Turso client DB write lane: {error}"))?;
    configure_turso_write_connection(
        &connection,
        TURSO_CLIENT_DB_MVCC_ENABLED
            && turso_path.file_name().and_then(|name| name.to_str())
                != Some(TURSO_SEARCH_PROJECTION_DB_FILE),
    )
    .await?;
    Ok(std::sync::Arc::new(tokio::sync::Mutex::new(connection)))
}

pub(super) async fn shared_turso_write_connection(
    turso_path: &Path,
) -> Result<TursoConnectionLease, String> {
    let entry = shared_turso_pool_entry(turso_path).await;
    let database = shared_turso_database(turso_path).await?;
    let mut lanes = {
        let lanes = entry.write_lanes.read().await;
        lanes.iter().cloned().collect::<Vec<_>>()
    };
    if lanes.is_empty() {
        let mut published = entry.write_lanes.write().await;
        if published.is_empty() {
            published.push(new_turso_write_lane(&database, turso_path).await?);
        }
        lanes = published.iter().cloned().collect();
    }

    let first_lane = entry
        .next_write_lane
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        % lanes.len();
    for offset in 0..lanes.len() {
        let lane = std::sync::Arc::clone(&lanes[(first_lane + offset) % lanes.len()]);
        if let Ok(connection) = lane.try_lock_owned() {
            return Ok(TursoConnectionLease {
                _database: database,
                connection,
                schema_state: std::sync::Arc::clone(&entry.schema_state),
            });
        }
    }

    let lane = if lanes.len() < adaptive_turso_write_lane_ceiling() {
        let mut published = entry.write_lanes.write().await;
        if published.len() < adaptive_turso_write_lane_ceiling() {
            let lane = new_turso_write_lane(&database, turso_path).await?;
            published.push(std::sync::Arc::clone(&lane));
            lane
        } else {
            std::sync::Arc::clone(&published[first_lane % published.len()])
        }
    } else {
        std::sync::Arc::clone(&lanes[first_lane])
    };
    let connection = lane.lock_owned().await;
    Ok(TursoConnectionLease {
        _database: database,
        connection,
        schema_state: std::sync::Arc::clone(&entry.schema_state),
    })
}

pub(super) async fn shared_turso_read_only_connection(
    turso_path: &Path,
) -> Result<mrr_data_backend::turso_driver::Connection, String> {
    let database = shared_turso_database(turso_path).await?;
    let connection = database
        .connect()
        .map_err(|error| format!("failed to connect Turso client DB read-only: {error}"))?;
    connection
        .execute("PRAGMA query_only = 1", ())
        .await
        .map_err(|error| format!("failed to enforce Turso client DB read-only mode: {error}"))?;
    Ok(connection)
}

pub(super) async fn build_turso_database(
    turso_path: &Path,
) -> Result<mrr_data_backend::turso_driver::Database, String> {
    let max_attempts = 8;
    let mut last_lock_error = None;
    for attempt in 0..max_attempts {
        match turso_builder(turso_path).build().await {
            Ok(database) => return Ok(database),
            Err(error) => {
                let message = error.to_string();
                if !crate::engine::turso_lock_policy::is_turso_lock_error(&message) {
                    return Err(format!("failed to open Turso client DB: {message}"));
                }
                last_lock_error = Some(message);
                if attempt + 1 == max_attempts {
                    break;
                }
                tokio::time::sleep(crate::engine::turso_lock_policy::turso_lock_retry_delay(
                    attempt,
                ))
                .await;
            }
        }
    }
    Err(format!(
        "failed to open Turso client DB after bounded lock retries: {}",
        last_lock_error.unwrap_or_else(|| "unknown Turso lock error".to_string())
    ))
}

#[cfg(test)]
#[path = "../../../tests/unit/db/engine/turso_pool.rs"]
mod tests;
