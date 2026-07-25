use std::{
    fs,
    path::{Path, PathBuf},
};

use super::facade::{ClientDbEngine, ClientDbEngineWriteSession, block_on_db_engine_async};

const TURSO_0_7_MIGRATION_RECEIPT_FILE: &str = "facts.turso.migration.v1.json";
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

impl ClientDbEngine {
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
        if target_client_dir.exists() {
            if !target_client_dir.is_dir() {
                return Err(format!(
                    "Turso 0.7 migration target is not a directory: `{}`",
                    target_client_dir.display()
                ));
            }
            if fs::read_dir(&target_client_dir)
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
                .is_some()
            {
                return Err(format!(
                    "Turso 0.7 migration target must be absent or empty: `{}`",
                    target_client_dir.display()
                ));
            }
        }
        let staging_client_dir = parent.join(format!(
            ".turso-0.7-staging-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| format!("failed to timestamp Turso 0.7 staging dir: {error}"))?
                .as_nanos()
        ));
        let mut writer = Self::open_write_session_client_dir(&staging_client_dir)?;
        let replay_coverage = match replay(&mut writer) {
            Ok(coverage) => coverage,
            Err(error) => {
                drop(writer);
                cleanup_staging_client_dir(&staging_client_dir);
                return Err(format!("failed to replay Turso 0.7 staging DB: {error}"));
            }
        };
        let validation_errors = replay_coverage.validation_errors();
        drop(writer);
        evict_turso_client_dir(&staging_client_dir)?;
        if !validation_errors.is_empty() {
            let _ = fs::remove_dir_all(&staging_client_dir);
            return Err(format!(
                "Turso 0.7 staging DB is incomplete; unverified v1 replayers: {}",
                validation_errors.join(",")
            ));
        }
        let staging_db_path = Self::turso_path_for_client_dir(&staging_client_dir);
        let staging_format_receipt_path =
            super::turso::turso_0_7_format_receipt_path(&staging_db_path);
        let staging_search_projection_db_path =
            super::turso::turso_search_projection_db_path(&staging_db_path);
        let staging_search_projection_format_receipt_path =
            super::turso::turso_0_7_format_receipt_path(&staging_search_projection_db_path);
        if !staging_db_path.is_file()
            || !staging_format_receipt_path.is_file()
            || !staging_search_projection_db_path.is_file()
            || !staging_search_projection_format_receipt_path.is_file()
        {
            let _ = fs::remove_dir_all(&staging_client_dir);
            return Err(
                "Turso 0.7 staging DB is missing a database file or physical-format receipt"
                    .to_string(),
            );
        }
        let staging_migration_receipt_path =
            staging_db_path.with_file_name(TURSO_0_7_MIGRATION_RECEIPT_FILE);
        write_migration_receipt(&staging_migration_receipt_path, &replay_coverage)?;
        if target_client_dir.exists() {
            fs::remove_dir(&target_client_dir).map_err(|error| {
                format!(
                    "failed to remove empty Turso 0.7 migration target `{}`: {error}",
                    target_client_dir.display()
                )
            })?;
        }
        fs::rename(&staging_client_dir, &target_client_dir).map_err(|error| {
            format!(
                "failed to atomically promote Turso 0.7 staging dir `{}` to `{}`: {error}",
                staging_client_dir.display(),
                target_client_dir.display()
            )
        })?;
        let db_path = Self::turso_path_for_client_dir(&target_client_dir);
        let format_receipt_path = super::turso::turso_0_7_format_receipt_path(&db_path);
        let search_projection_db_path = super::turso::turso_search_projection_db_path(&db_path);
        let search_projection_format_receipt_path =
            super::turso::turso_0_7_format_receipt_path(&search_projection_db_path);
        let migration_receipt_path = db_path.with_file_name(TURSO_0_7_MIGRATION_RECEIPT_FILE);
        Ok(ClientDbTurso07MigrationReport {
            target_client_dir,
            db_path,
            format_receipt_path,
            search_projection_db_path,
            search_projection_format_receipt_path,
            migration_receipt_path,
            replay_coverage,
        })
    }
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
