// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Turso adapter boundary with separate physical admission and pool owners.

mod adapter;
mod pool;

pub use adapter::{TURSO_BOOTSTRAP_TABLE, TursoClientDbEngineReport};
pub(super) use adapter::{
    TursoClientDbEngineBackend, bootstrap_turso_schema_version, connect_turso_client_db,
    connect_turso_client_db_read_only, connect_turso_search_projection_db_for_write,
    connect_turso_search_projection_db_read_only, open_turso_0_7_migration_source,
    open_turso_client_db_read_only, prepare_turso_client_db_path, turso_0_7_format_receipt_path,
    turso_bootstrap_report, turso_client_db_exists, turso_search_projection_db_path,
    turso_table_exists, validate_turso_0_7_migration_target, write_turso_0_7_format_receipt,
};
pub(crate) use pool::shared_turso_database;
pub(super) use pool::{TursoConnectionLease, evict_turso_client_dir};
