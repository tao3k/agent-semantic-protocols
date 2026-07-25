//! Canonical project client-DB migration execution and receipts.

use std::path::Path;

use agent_semantic_client_db::ClientDbEngine;
use agent_semantic_client_db::engine::ClientDbTurso07ActiveMigration;
use serde_json::json;

use super::turso_migration_args::{CacheMigrationTarget, parse_cache_migration_target};

pub(crate) fn run_cache_migration(
    project_root: &Path,
    args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    let Some(target) = parse_cache_migration_target(args)? else {
        return Ok(());
    };
    match target {
        CacheMigrationTarget::Turso07 => {
            let state = agent_semantic_runtime::project_state_paths(project_root)?;
            render_turso_0_7_migration(
                ClientDbEngine::migrate_active_project_client_dir_to_turso_0_7(
                    &state.client_cache_dir,
                )?,
                receipt_json,
            )
        }
    }
}

fn render_turso_0_7_migration(
    migration: ClientDbTurso07ActiveMigration,
    receipt_json: bool,
) -> Result<(), String> {
    match migration {
        ClientDbTurso07ActiveMigration::Absent { client_dir } => {
            println!(
                "[asp-cache-migrate] status=absent target=turso-0.7 logicalSchema=1 clientDir={}",
                client_dir.display()
            );
            if receipt_json {
                eprintln!(
                    "{}",
                    json!({
                        "schemaId": "agent.semantic-protocols.cache-migration.receipt",
                        "schemaVersion": "1",
                        "status": "absent",
                        "target": "turso-0.7",
                        "logicalSchemaVersion": 1,
                        "clientDir": client_dir,
                    })
                );
            }
        }
        ClientDbTurso07ActiveMigration::AlreadyCurrent {
            client_dir,
            db_path,
        } => {
            println!(
                "[asp-cache-migrate] status=current target=turso-0.7 logicalSchema=1 clientDir={} db={}",
                client_dir.display(),
                db_path.display()
            );
            if receipt_json {
                eprintln!(
                    "{}",
                    json!({
                        "schemaId": "agent.semantic-protocols.cache-migration.receipt",
                        "schemaVersion": "1",
                        "status": "current",
                        "target": "turso-0.7",
                        "logicalSchemaVersion": 1,
                        "clientDir": client_dir,
                        "dbPath": db_path,
                    })
                );
            }
        }
        ClientDbTurso07ActiveMigration::Migrated {
            report,
            rollback_client_dir,
        } => {
            println!(
                "[asp-cache-migrate] status=migrated target=turso-0.7 logicalSchema=1 physicalFormat=turso-0.7-native clientDir={} db={} rollback={} familiesVerified=7",
                report.target_client_dir.display(),
                report.db_path.display(),
                rollback_client_dir.display()
            );
            if receipt_json {
                eprintln!(
                    "{}",
                    json!({
                        "schemaId": "agent.semantic-protocols.cache-migration.receipt",
                        "schemaVersion": "1",
                        "status": "migrated",
                        "target": "turso-0.7",
                        "logicalSchemaVersion": 1,
                        "physicalFormat": "turso-0.7-native",
                        "clientDir": report.target_client_dir,
                        "dbPath": report.db_path,
                        "formatReceiptPath": report.format_receipt_path,
                        "searchProjectionDbPath": report.search_projection_db_path,
                        "searchProjectionFormatReceiptPath":
                            report.search_projection_format_receipt_path,
                        "migrationReceiptPath": report.migration_receipt_path,
                        "rollbackClientDir": rollback_client_dir,
                        "families": report.replay_coverage,
                    })
                );
            }
        }
    }
    Ok(())
}
