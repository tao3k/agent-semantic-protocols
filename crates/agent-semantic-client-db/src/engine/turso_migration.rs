// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

use fs2::FileExt;

use super::facade::{ClientDbEngine, ClientDbEngineWriteSession, block_on_db_engine_async};

const TURSO_0_7_MIGRATION_RECEIPT_FILE: &str = "facts.turso.migration.v1.json";
const TURSO_0_7_ACTIVE_MIGRATION_LOCK_FILE: &str = ".client-turso-0.7-migration.lock";
pub(super) const TURSO_0_7_ACTIVE_MIGRATION_MARKER_FILE: &str = ".turso-0.7-cutover.v1.json";
const EMPTY_FAMILY_DIGEST_V1: &str =
    "blake3:af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";

/// Count-and-digest evidence for one fully enumerated durable v1 family.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbTurso07ReplayFamilyReceipt {
    status: &'static str,
    pub source_record_count: u64,
    pub target_record_count: u64,
    pub source_digest: String,
    pub target_digest: String,
}

impl ClientDbTurso07ReplayFamilyReceipt {
    pub fn compare(
        source_record_count: u64,
        target_record_count: u64,
        source_digest: impl Into<String>,
        target_digest: impl Into<String>,
    ) -> Self {
        let source_digest = source_digest.into();
        let target_digest = target_digest.into();
        let status = if source_record_count == target_record_count
            && !source_digest.is_empty()
            && source_digest == target_digest
        {
            "verified"
        } else {
            "mismatch"
        };
        Self {
            status,
            source_record_count,
            target_record_count,
            source_digest,
            target_digest,
        }
    }

    pub fn matched(record_count: u64, digest: impl Into<String>) -> Self {
        let digest = digest.into();
        Self::compare(record_count, record_count, digest.clone(), digest)
    }

    pub fn enumerated_empty() -> Self {
        Self::matched(0, EMPTY_FAMILY_DIGEST_V1)
    }

    fn validation_error(&self, family: &'static str) -> Option<String> {
        (self.status != "verified").then(|| {
            format!(
                "{family}(sourceCount={},targetCount={},sourceDigest={},targetDigest={})",
                self.source_record_count,
                self.target_record_count,
                self.source_digest,
                self.target_digest
            )
        })
    }
}

/// Evidence for obsolete derived tables preserved outside active authority.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbTurso07RetiredDerivedReceipt {
    status: &'static str,
    pub source_record_count: u64,
    pub source_digest: String,
    pub source_tables: Vec<String>,
    disposition: &'static str,
}

impl ClientDbTurso07RetiredDerivedReceipt {
    pub(crate) fn preserved(
        source_record_count: u64,
        source_digest: impl Into<String>,
        source_tables: Vec<String>,
    ) -> Self {
        Self {
            status: "preserved",
            source_record_count,
            source_digest: source_digest.into(),
            source_tables,
            disposition: "read-disabled-rollback-and-rebuild",
        }
    }

    /// Record that no obsolete derived rows were present in the source.
    #[must_use]
    pub fn enumerated_empty() -> Self {
        Self::preserved(0, EMPTY_FAMILY_DIGEST_V1, Vec::new())
    }
}

/// Records count-and-digest evidence for every durable v1 input family.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbTurso07ReplayCoverage {
    pub cache_manifest: ClientDbTurso07ReplayFamilyReceipt,
    pub syntax_query: ClientDbTurso07ReplayFamilyReceipt,
    pub source_index: ClientDbTurso07ReplayFamilyReceipt,
    pub structural_index: ClientDbTurso07ReplayFamilyReceipt,
    pub provider_command: ClientDbTurso07ReplayFamilyReceipt,
    pub artifact_event: ClientDbTurso07ReplayFamilyReceipt,
    pub artifact_pointer: ClientDbTurso07ReplayFamilyReceipt,
    pub retired_derived_projection: ClientDbTurso07RetiredDerivedReceipt,
}

impl ClientDbTurso07ReplayCoverage {
    fn validation_errors(&self) -> Vec<String> {
        [
            ("cache_manifest", &self.cache_manifest),
            ("syntax_query", &self.syntax_query),
            ("source_index", &self.source_index),
            ("structural_index", &self.structural_index),
            ("provider_command", &self.provider_command),
            ("artifact_event", &self.artifact_event),
            ("artifact_pointer", &self.artifact_pointer),
        ]
        .into_iter()
        .filter_map(|(name, receipt)| receipt.validation_error(name))
        .collect()
    }
}

/// Receipt for a completed staging-to-project Turso 0.7 promotion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbTurso07MigrationReport {
    pub target_client_dir: PathBuf,
    pub db_path: PathBuf,
    pub format_receipt_path: PathBuf,
    pub search_projection_db_path: PathBuf,
    pub search_projection_format_receipt_path: PathBuf,
    pub migration_receipt_path: PathBuf,
    pub replay_coverage: ClientDbTurso07ReplayCoverage,
}

/// Outcome of reconciling the canonical active project client directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientDbTurso07ActiveMigration {
    Absent {
        client_dir: PathBuf,
    },
    AlreadyCurrent {
        client_dir: PathBuf,
        db_path: PathBuf,
    },
    Migrated {
        report: Box<ClientDbTurso07MigrationReport>,
        rollback_client_dir: PathBuf,
    },
}

impl ClientDbEngine {
    /// Reconcile an active canonical project client directory to native Turso 0.7.
    ///
    /// Non-database artifacts remain in place. The legacy database authority is
    /// moved into a read-disabled rollback directory only after a complete
    /// staging replay has passed count-and-digest validation.
    pub fn migrate_active_project_client_dir_to_turso_0_7(
        client_dir: impl AsRef<Path>,
    ) -> Result<ClientDbTurso07ActiveMigration, String> {
        let client_dir = client_dir.as_ref().to_path_buf();
        let parent = client_dir.parent().ok_or_else(|| {
            format!(
                "active project client dir has no parent: `{}`",
                client_dir.display()
            )
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create active Turso migration parent `{}`: {error}",
                parent.display()
            )
        })?;
        let lock_path = parent.join(TURSO_0_7_ACTIVE_MIGRATION_LOCK_FILE);
        let lock_file = open_migration_lock(&lock_path)?;
        lock_file.lock_exclusive().map_err(|error| {
            format!(
                "failed to acquire active Turso migration lock `{}`: {error}",
                lock_path.display()
            )
        })?;

        let outcome = migrate_active_project_client_dir_locked(&client_dir);
        FileExt::unlock(&lock_file).map_err(|error| {
            format!(
                "failed to release active Turso migration lock `{}`: {error}",
                lock_path.display()
            )
        })?;
        outcome
    }

    /// Fully replay a legacy project client DB into a fresh Turso 0.7 project DB.
    pub fn migrate_legacy_project_client_dir_to_turso_0_7(
        source_client_dir: impl AsRef<Path>,
        target_client_dir: impl AsRef<Path>,
    ) -> Result<ClientDbTurso07MigrationReport, String> {
        let source_client_dir = source_client_dir.as_ref().to_path_buf();
        let facts_path = Self::turso_path_for_client_dir(&source_client_dir);
        let legacy_client_path = source_client_dir.join("client.turso");
        let source_facts_path = if facts_path.is_file() {
            facts_path
        } else if legacy_client_path.is_file() {
            legacy_client_path
        } else {
            return Err(format!(
                "legacy client DB has no facts.turso or client.turso in `{}`",
                source_client_dir.display()
            ));
        };
        let target_client_dir = target_client_dir.as_ref().to_path_buf();
        Self::migrate_project_client_dir_to_turso_0_7(target_client_dir, move |writer| {
            let target_facts_path = writer.turso_db_path.clone();
            block_on_db_engine_async(async move {
                super::turso_legacy_migration::replay_legacy_client_db(
                    &source_facts_path,
                    &target_facts_path,
                )
                .await
            })
        })
    }

    /// Replay every durable v1 input into a staging Turso 0.7 DB and atomically promote it.
    pub fn migrate_project_client_dir_to_turso_0_7<F>(
        target_client_dir: impl AsRef<Path>,
        replay: F,
    ) -> Result<ClientDbTurso07MigrationReport, String>
    where
        F: FnOnce(&mut ClientDbEngineWriteSession) -> Result<ClientDbTurso07ReplayCoverage, String>,
    {
        let target_client_dir = target_client_dir.as_ref().to_path_buf();
        let parent = prepare_turso_migration_target(&target_client_dir)?;
        let staging_client_dir = turso_migration_staging_path(&parent)?;
        let replay_coverage = replay_turso_migration_staging(&staging_client_dir, replay)?;
        validate_turso_migration_staging(&staging_client_dir, &replay_coverage)?;
        promote_turso_migration_staging(&staging_client_dir, &target_client_dir)?;
        Ok(turso_migration_report(target_client_dir, replay_coverage))
    }
}

fn prepare_turso_migration_target(target_client_dir: &Path) -> Result<PathBuf, String> {
    let parent = target_client_dir.parent().ok_or_else(|| {
        format!(
            "Turso 0.7 project client dir has no parent: `{}`",
            target_client_dir.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create Turso 0.7 migration parent `{}`: {error}",
            parent.display()
        )
    })?;
    if !target_client_dir.exists() {
        return Ok(parent.to_path_buf());
    }
    if !target_client_dir.is_dir() {
        return Err(format!(
            "Turso 0.7 migration target is not a directory: `{}`",
            target_client_dir.display()
        ));
    }
    let has_entry = fs::read_dir(target_client_dir)
        .map_err(|error| {
            format!(
                "failed to inspect Turso 0.7 migration target `{}`: {error}",
                target_client_dir.display()
            )
        })?
        .next()
        .transpose()
        .map_err(|error| {
            format!(
                "failed to inspect Turso 0.7 migration target entry `{}`: {error}",
                target_client_dir.display()
            )
        })?
        .is_some();
    if has_entry {
        return Err(format!(
            "Turso 0.7 migration target must be absent or empty: `{}`",
            target_client_dir.display()
        ));
    }
    Ok(parent.to_path_buf())
}

fn turso_migration_staging_path(parent: &Path) -> Result<PathBuf, String> {
    Ok(parent.join(format!(
        ".turso-0.7-staging-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("failed to timestamp Turso 0.7 staging dir: {error}"))?
            .as_nanos()
    )))
}

fn replay_turso_migration_staging<F>(
    staging_client_dir: &Path,
    replay: F,
) -> Result<ClientDbTurso07ReplayCoverage, String>
where
    F: FnOnce(&mut ClientDbEngineWriteSession) -> Result<ClientDbTurso07ReplayCoverage, String>,
{
    let mut writer = ClientDbEngine::open_write_session_client_dir(staging_client_dir)?;
    let replay_coverage = match replay(&mut writer) {
        Ok(coverage) => coverage,
        Err(error) => {
            drop(writer);
            cleanup_staging_client_dir(staging_client_dir);
            return Err(format!("failed to replay Turso 0.7 staging DB: {error}"));
        }
    };
    let validation_errors = replay_coverage.validation_errors();
    drop(writer);
    evict_turso_client_dir(staging_client_dir)?;
    if validation_errors.is_empty() {
        Ok(replay_coverage)
    } else {
        let _ = fs::remove_dir_all(staging_client_dir);
        Err(format!(
            "Turso 0.7 staging DB is incomplete; unverified v1 replayers: {}",
            validation_errors.join(",")
        ))
    }
}

fn validate_turso_migration_staging(
    staging_client_dir: &Path,
    replay_coverage: &ClientDbTurso07ReplayCoverage,
) -> Result<(), String> {
    let staging_db_path = ClientDbEngine::turso_path_for_client_dir(staging_client_dir);
    let format_receipt = super::turso::turso_0_7_format_receipt_path(&staging_db_path);
    let search_db = super::turso::turso_search_projection_db_path(&staging_db_path);
    let search_receipt = super::turso::turso_0_7_format_receipt_path(&search_db);
    if ![
        &staging_db_path,
        &format_receipt,
        &search_db,
        &search_receipt,
    ]
    .into_iter()
    .all(|path| path.is_file())
    {
        let _ = fs::remove_dir_all(staging_client_dir);
        return Err(
            "Turso 0.7 staging DB is missing a database file or physical-format receipt".to_owned(),
        );
    }
    write_migration_receipt(
        &staging_db_path.with_file_name(TURSO_0_7_MIGRATION_RECEIPT_FILE),
        replay_coverage,
    )
}

fn promote_turso_migration_staging(
    staging_client_dir: &Path,
    target_client_dir: &Path,
) -> Result<(), String> {
    if target_client_dir.exists() {
        fs::remove_dir(target_client_dir).map_err(|error| {
            format!(
                "failed to remove empty Turso 0.7 migration target `{}`: {error}",
                target_client_dir.display()
            )
        })?;
    }
    fs::rename(staging_client_dir, target_client_dir).map_err(|error| {
        format!(
            "failed to atomically promote Turso 0.7 staging dir `{}` to `{}`: {error}",
            staging_client_dir.display(),
            target_client_dir.display()
        )
    })
}

fn turso_migration_report(
    target_client_dir: PathBuf,
    replay_coverage: ClientDbTurso07ReplayCoverage,
) -> ClientDbTurso07MigrationReport {
    let db_path = ClientDbEngine::turso_path_for_client_dir(&target_client_dir);
    let format_receipt_path = super::turso::turso_0_7_format_receipt_path(&db_path);
    let search_projection_db_path = super::turso::turso_search_projection_db_path(&db_path);
    let search_projection_format_receipt_path =
        super::turso::turso_0_7_format_receipt_path(&search_projection_db_path);
    let migration_receipt_path = db_path.with_file_name(TURSO_0_7_MIGRATION_RECEIPT_FILE);
    ClientDbTurso07MigrationReport {
        target_client_dir,
        db_path,
        format_receipt_path,
        search_projection_db_path,
        search_projection_format_receipt_path,
        migration_receipt_path,
        replay_coverage,
    }
}

fn migrate_active_project_client_dir_locked(
    client_dir: &Path,
) -> Result<ClientDbTurso07ActiveMigration, String> {
    let db_path = ClientDbEngine::turso_path_for_client_dir(client_dir);
    let legacy_db_path = client_dir.join("client.turso");
    if !db_path.is_file() && !legacy_db_path.is_file() {
        return Ok(ClientDbTurso07ActiveMigration::Absent {
            client_dir: client_dir.to_path_buf(),
        });
    }
    let format_receipt_path = super::turso::turso_0_7_format_receipt_path(&db_path);
    if db_path.is_file() && format_receipt_path.is_file() {
        let reader = ClientDbEngine::open_read_session_client_dir(client_dir)?
            .ok_or_else(|| format!("active Turso DB disappeared: `{}`", db_path.display()))?;
        drop(reader);
        return Ok(ClientDbTurso07ActiveMigration::AlreadyCurrent {
            client_dir: client_dir.to_path_buf(),
            db_path,
        });
    }

    fs::create_dir_all(client_dir).map_err(|error| {
        format!(
            "failed to create active project client dir `{}`: {error}",
            client_dir.display()
        )
    })?;
    let nonce = migration_nonce()?;
    let prepared_client_dir = client_dir
        .parent()
        .expect("active client parent checked")
        .join(format!(".client-turso-0.7-prepared-{nonce}"));
    let staged_report = ClientDbEngine::migrate_legacy_project_client_dir_to_turso_0_7(
        client_dir,
        &prepared_client_dir,
    )?;
    let rollback_client_dir = client_dir.join(format!("retired-client-turso-pre-0-7-{nonce}"));
    fs::create_dir(&rollback_client_dir).map_err(|error| {
        cleanup_staging_client_dir(&prepared_client_dir);
        format!(
            "failed to create read-disabled Turso rollback dir `{}`: {error}",
            rollback_client_dir.display()
        )
    })?;
    let marker_path = client_dir.join(TURSO_0_7_ACTIVE_MIGRATION_MARKER_FILE);
    if let Err(error) =
        write_active_migration_marker(&marker_path, &prepared_client_dir, &rollback_client_dir)
    {
        cleanup_staging_client_dir(&prepared_client_dir);
        let _ = fs::remove_dir(&rollback_client_dir);
        return Err(error);
    }

    evict_turso_client_dir(client_dir)?;
    evict_turso_client_dir(&prepared_client_dir)?;
    let old_files = database_authority_files(client_dir)?;
    let new_files = database_authority_files(&prepared_client_dir)?;
    let cutover = move_database_authority(
        client_dir,
        &prepared_client_dir,
        &rollback_client_dir,
        &old_files,
        &new_files,
    );
    if let Err(error) = cutover {
        let _ = fs::remove_file(&marker_path);
        cleanup_staging_client_dir(&prepared_client_dir);
        return Err(error);
    }

    let validation_path = db_path.clone();
    let validation = block_on_db_engine_async(async move {
        super::turso::validate_turso_0_7_migration_target(validation_path).await
    });
    if let Err(error) = validation {
        let rollback =
            rollback_promoted_authority(client_dir, &prepared_client_dir, &rollback_client_dir);
        let _ = fs::remove_file(&marker_path);
        cleanup_staging_client_dir(&prepared_client_dir);
        return match rollback {
            Ok(()) => Err(format!(
                "promoted Turso 0.7 DB failed validation and was rolled back: {error}"
            )),
            Err(rollback_error) => Err(format!(
                "promoted Turso 0.7 DB failed validation: {error}; rollback failed: {rollback_error}"
            )),
        };
    }
    fs::remove_file(&marker_path).map_err(|error| {
        format!(
            "failed to clear active Turso migration marker `{}`: {error}",
            marker_path.display()
        )
    })?;
    fs::remove_dir(&prepared_client_dir).map_err(|error| {
        format!(
            "failed to remove empty prepared Turso dir `{}`: {error}",
            prepared_client_dir.display()
        )
    })?;

    Ok(ClientDbTurso07ActiveMigration::Migrated {
        report: Box::new(ClientDbTurso07MigrationReport {
            target_client_dir: client_dir.to_path_buf(),
            db_path: client_dir.join(
                staged_report
                    .db_path
                    .file_name()
                    .expect("staged DB path has file name"),
            ),
            format_receipt_path: client_dir.join(
                staged_report
                    .format_receipt_path
                    .file_name()
                    .expect("staged format receipt has file name"),
            ),
            search_projection_db_path: client_dir.join(
                staged_report
                    .search_projection_db_path
                    .file_name()
                    .expect("staged search DB path has file name"),
            ),
            search_projection_format_receipt_path: client_dir.join(
                staged_report
                    .search_projection_format_receipt_path
                    .file_name()
                    .expect("staged search receipt has file name"),
            ),
            migration_receipt_path: client_dir.join(
                staged_report
                    .migration_receipt_path
                    .file_name()
                    .expect("staged migration receipt has file name"),
            ),
            replay_coverage: staged_report.replay_coverage,
        }),
        rollback_client_dir,
    })
}

fn open_migration_lock(lock_path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|error| {
            format!(
                "failed to open active Turso migration lock `{}`: {error}",
                lock_path.display()
            )
        })
}

fn migration_nonce() -> Result<String, String> {
    Ok(format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("failed to timestamp active Turso migration: {error}"))?
            .as_nanos()
    ))
}

fn write_active_migration_marker(
    marker_path: &Path,
    prepared_client_dir: &Path,
    rollback_client_dir: &Path,
) -> Result<(), String> {
    let receipt = serde_json::to_vec_pretty(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.turso-client-db-active-cutover",
        "schemaVersion": "1",
        "physicalFormat": "turso-0.7-native",
        "preparedClientDir": prepared_client_dir,
        "rollbackClientDir": rollback_client_dir,
    }))
    .map_err(|error| format!("failed to encode active Turso migration marker: {error}"))?;
    fs::write(marker_path, receipt).map_err(|error| {
        format!(
            "failed to write active Turso migration marker `{}`: {error}",
            marker_path.display()
        )
    })
}

fn database_authority_files(client_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = fs::read_dir(client_dir)
        .map_err(|error| {
            format!(
                "failed to inspect Turso client dir `{}`: {error}",
                client_dir.display()
            )
        })?
        .map(|entry| {
            entry.map(|value| value.path()).map_err(|error| {
                format!(
                    "failed to inspect Turso client entry in `{}`: {error}",
                    client_dir.display()
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    files.retain(|path| is_database_authority_file(path));
    files.sort();
    Ok(files)
}

fn is_database_authority_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    path.is_file()
        && (name.starts_with("facts.")
            || name.starts_with("search-projection.")
            || matches!(name, "client.turso" | "client.turso-wal" | "client.db-log"))
}

fn move_database_authority(
    active_client_dir: &Path,
    prepared_client_dir: &Path,
    rollback_client_dir: &Path,
    old_files: &[PathBuf],
    new_files: &[PathBuf],
) -> Result<(), String> {
    let mut moved_old = Vec::with_capacity(old_files.len());
    for source in old_files {
        match move_file_to_dir(source, rollback_client_dir) {
            Ok(target) => moved_old.push((source.clone(), target)),
            Err(error) => return rollback_cutover(error, &moved_old, &[]),
        }
    }
    let mut moved_new = Vec::with_capacity(new_files.len());
    for source in new_files {
        match move_file_to_dir(source, active_client_dir) {
            Ok(target) => moved_new.push((source.clone(), target)),
            Err(error) => return rollback_cutover(error, &moved_old, &moved_new),
        }
    }
    if ClientDbEngine::turso_path_for_client_dir(prepared_client_dir).exists() {
        return rollback_cutover(
            "prepared Turso DB still exists after active promotion".to_string(),
            &moved_old,
            &moved_new,
        );
    }
    Ok(())
}

fn move_file_to_dir(source: &Path, target_dir: &Path) -> Result<PathBuf, String> {
    let file_name = source.file_name().ok_or_else(|| {
        format!(
            "Turso migration source has no file name: `{}`",
            source.display()
        )
    })?;
    let target = target_dir.join(file_name);
    fs::rename(source, &target)
        .map(|()| target.clone())
        .map_err(|error| {
            format!(
                "failed to move Turso migration file `{}` to `{}`: {error}",
                source.display(),
                target.display()
            )
        })
}

fn rollback_cutover(
    cutover_error: String,
    moved_old: &[(PathBuf, PathBuf)],
    moved_new: &[(PathBuf, PathBuf)],
) -> Result<(), String> {
    let mut rollback_errors = Vec::new();
    if let Err(error) = reverse_moves(moved_new) {
        rollback_errors.push(error);
    }
    if let Err(error) = reverse_moves(moved_old) {
        rollback_errors.push(error);
    }
    if rollback_errors.is_empty() {
        Err(cutover_error)
    } else {
        Err(format!(
            "{cutover_error}; active Turso rollback also failed: {}",
            rollback_errors.join("; ")
        ))
    }
}

fn reverse_moves(moves: &[(PathBuf, PathBuf)]) -> Result<(), String> {
    for (source, target) in moves.iter().rev() {
        fs::rename(target, source).map_err(|error| {
            format!(
                "failed to restore Turso migration file `{}` to `{}`: {error}",
                target.display(),
                source.display()
            )
        })?;
    }
    Ok(())
}

fn rollback_promoted_authority(
    active_client_dir: &Path,
    prepared_client_dir: &Path,
    rollback_client_dir: &Path,
) -> Result<(), String> {
    for source in database_authority_files(active_client_dir)? {
        move_file_to_dir(&source, prepared_client_dir)?;
    }
    for source in database_authority_files(rollback_client_dir)? {
        move_file_to_dir(&source, active_client_dir)?;
    }
    Ok(())
}

fn evict_turso_client_dir(client_dir: &Path) -> Result<(), String> {
    let evict_dir = client_dir.to_path_buf();
    block_on_db_engine_async(async move {
        super::turso::evict_turso_client_dir(&evict_dir).await;
        Ok::<(), String>(())
    })
}

fn cleanup_staging_client_dir(staging_client_dir: &Path) {
    let _ = evict_turso_client_dir(staging_client_dir);
    let _ = fs::remove_dir_all(staging_client_dir);
}

fn write_migration_receipt(
    receipt_path: &Path,
    replay_coverage: &ClientDbTurso07ReplayCoverage,
) -> Result<(), String> {
    let temporary_path = receipt_path.with_extension(format!(
        "json.tmp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("failed to timestamp Turso 0.7 migration receipt: {error}"))?
            .as_nanos()
    ));
    let receipt = serde_json::to_vec_pretty(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.turso-client-db-migration-receipt",
        "schemaVersion": "1",
        "migrationId": "client-db-v1-project-partition-cutover",
        "logicalSchemaVersion": 1,
        "physicalFormat": "turso-0.7-native",
        "tursoVersion": "0.7",
        "sourcePreserved": true,
        "atomicPromotion": true,
        "families": replay_coverage,
    }))
    .map_err(|error| format!("failed to encode Turso 0.7 migration receipt: {error}"))?;
    fs::write(&temporary_path, receipt).map_err(|error| {
        format!(
            "failed to write Turso 0.7 migration receipt `{}`: {error}",
            temporary_path.display()
        )
    })?;
    fs::rename(&temporary_path, receipt_path).map_err(|error| {
        format!(
            "failed to promote Turso 0.7 migration receipt `{}`: {error}",
            receipt_path.display()
        )
    })
}
