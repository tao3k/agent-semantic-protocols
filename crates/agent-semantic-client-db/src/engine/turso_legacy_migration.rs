use std::path::Path;

use agent_semantic_client_core::CacheGenerationId;

use super::turso_cache_key::{active_cache_generation_key, active_cache_lookup_key};
use super::turso_migration::{ClientDbTurso07ReplayCoverage, ClientDbTurso07ReplayFamilyReceipt};

const EMPTY_FAMILY_DIGEST_V1: &str =
    "blake3:af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
const FAMILY_COUNT: usize = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MigrationFamily {
    CacheManifest = 0,
    SyntaxQuery = 1,
    SourceIndex = 2,
    StructuralIndex = 3,
    ProviderCommand = 4,
    ArtifactEvent = 5,
    ArtifactPointer = 6,
}

#[derive(Clone, Copy, Debug)]
enum MigrationPlane {
    Facts,
    SearchProjection,
}

#[derive(Clone, Debug)]
struct MigrationTable {
    name: String,
    family: MigrationFamily,
}

#[derive(Clone, Debug)]
struct MigrationSummary {
    record_counts: [u64; FAMILY_COUNT],
    digests: [u64; FAMILY_COUNT],
}

impl Default for MigrationSummary {
    fn default() -> Self {
        Self {
            record_counts: [0; FAMILY_COUNT],
            digests: [FNV64_OFFSET_BASIS; FAMILY_COUNT],
        }
    }
}

impl MigrationSummary {
    fn observe(
        &mut self,
        family: MigrationFamily,
        table: &str,
        row: &turso::Row,
    ) -> Result<(), String> {
        let index = family as usize;
        self.record_counts[index] = self.record_counts[index].saturating_add(1);
        update_fnv64(&mut self.digests[index], table.as_bytes());
        update_fnv64(&mut self.digests[index], &[0xff]);
        for column_index in 0..row.column_count() {
            let value = row.get_value(column_index).map_err(|error| {
                format!(
                    "failed to read Turso 0.7 migration row value {table}[{column_index}]: {error}"
                )
            })?;
            update_value_digest(&mut self.digests[index], &value);
        }
        update_fnv64(&mut self.digests[index], &[0xfe]);
        Ok(())
    }

    fn receipt(
        &self,
        target: &Self,
        family: MigrationFamily,
    ) -> ClientDbTurso07ReplayFamilyReceipt {
        let index = family as usize;
        ClientDbTurso07ReplayFamilyReceipt::compare(
            self.record_counts[index],
            target.record_counts[index],
            digest_label(self.record_counts[index], self.digests[index]),
            digest_label(target.record_counts[index], target.digests[index]),
        )
    }
}

pub(super) async fn replay_legacy_client_db(
    source_facts_path: &Path,
    target_facts_path: &Path,
) -> Result<ClientDbTurso07ReplayCoverage, String> {
    let (_source_facts_db, source_facts) =
        super::turso::open_turso_0_7_migration_source(source_facts_path).await?;
    let target_facts = super::turso::connect_turso_client_db(target_facts_path).await?;
    let source_fact_tables = list_migration_tables(&source_facts, MigrationPlane::Facts).await?;
    let target_fact_tables = list_migration_tables(&target_facts, MigrationPlane::Facts).await?;
    ensure_target_covers_source_tables(&source_fact_tables, &target_fact_tables)?;
    copy_tables(&source_facts, &target_facts, &source_fact_tables).await?;
    rebuild_active_cache_generation_pointers(&target_facts).await?;
    let source_fact_summary = summarize_tables(&source_facts, &source_fact_tables).await?;
    let target_fact_summary = summarize_tables(&target_facts, &target_fact_tables).await?;

    let source_search_path = super::turso::turso_search_projection_db_path(source_facts_path);
    let target_search =
        super::turso::connect_turso_search_projection_db_for_write(target_facts_path).await?;
    let target_search_tables =
        list_migration_tables(&target_search, MigrationPlane::SearchProjection).await?;
    let (source_search_summary, target_search_summary) = if source_search_path.is_file() {
        let (_source_search_db, source_search) =
            super::turso::open_turso_0_7_migration_source(&source_search_path).await?;
        let source_search_tables =
            list_migration_tables(&source_search, MigrationPlane::SearchProjection).await?;
        ensure_target_covers_source_tables(&source_search_tables, &target_search_tables)?;
        copy_tables(&source_search, &target_search, &source_search_tables).await?;
        (
            summarize_tables(&source_search, &source_search_tables).await?,
            summarize_tables(&target_search, &target_search_tables).await?,
        )
    } else {
        (
            MigrationSummary::default(),
            summarize_tables(&target_search, &target_search_tables).await?,
        )
    };

    Ok(ClientDbTurso07ReplayCoverage {
        cache_manifest: source_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::CacheManifest),
        syntax_query: source_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::SyntaxQuery),
        source_index: source_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::SourceIndex),
        structural_index: source_search_summary
            .receipt(&target_search_summary, MigrationFamily::StructuralIndex),
        provider_command: source_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::ProviderCommand),
        artifact_event: source_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::ArtifactEvent),
        artifact_pointer: source_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::ArtifactPointer),
    })
}

async fn list_migration_tables(
    connection: &turso::Connection,
    plane: MigrationPlane,
) -> Result<Vec<MigrationTable>, String> {
    let mut rows = connection
        .query(
            "SELECT name
             FROM sqlite_schema
             WHERE type = 'table'
               AND name LIKE 'asp_%'
             ORDER BY name",
            (),
        )
        .await
        .map_err(|error| format!("failed to enumerate Turso 0.7 migration tables: {error}"))?;
    let mut tables = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso 0.7 migration table: {error}"))?
    {
        let name = row
            .get::<String>(0)
            .map_err(|error| format!("failed to decode Turso 0.7 migration table: {error}"))?;
        if matches!(
            name.as_str(),
            "asp_db_engine_bootstrap"
                | "asp_db_engine_format"
                | "asp_db_engine_migration"
                | "asp_cache_active_generation_v1"
        ) {
            continue;
        }
        validate_identifier(&name)?;
        let family = table_family(&name, plane).ok_or_else(|| {
            format!("unmapped durable v1 table `{name}` blocks Turso 0.7 migration")
        })?;
        tables.push(MigrationTable { name, family });
    }
    Ok(tables)
}

async fn rebuild_active_cache_generation_pointers(
    connection: &turso::Connection,
) -> Result<(), String> {
    connection
        .execute("DELETE FROM asp_cache_active_generation_v1", ())
        .await
        .map_err(|error| {
            format!("failed to reset Turso 0.7 active-generation pointers: {error}")
        })?;
    let mut rows = connection
        .query(
            "SELECT project_root,
                    language_id,
                    provider_id,
                    export_method,
                    request_fingerprint,
                    generation_id,
                    schema_ids_json,
                    artifact_ids_json,
                    file_hashes_json,
                    updated_at_ms
             FROM asp_cache_generation
             WHERE request_fingerprint IS NOT NULL
               AND raw_source_stored = 0
             ORDER BY updated_at_ms, generation_id",
            (),
        )
        .await
        .map_err(|error| {
            format!("failed to enumerate Turso 0.7 active-generation pointers: {error}")
        })?;
    let mut statement = connection
        .prepare_cached(
            "INSERT OR REPLACE INTO asp_cache_active_generation_v1 (
                lookup_key,
                generation_key,
                project_root,
                language_id,
                provider_id,
                export_method,
                request_fingerprint,
                generation_id,
                schema_ids_json,
                artifact_ids_json,
                file_hashes_json,
                updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )
        .await
        .map_err(|error| {
            format!("failed to prepare Turso 0.7 active-generation pointer rebuild: {error}")
        })?;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso 0.7 pointer source row: {error}"))?
    {
        let project_root = row
            .get::<String>(0)
            .map_err(|error| format!("failed to decode Turso 0.7 pointer project root: {error}"))?;
        let language_id = row
            .get::<String>(1)
            .map_err(|error| format!("failed to decode Turso 0.7 pointer language id: {error}"))?;
        let provider_id = row
            .get::<String>(2)
            .map_err(|error| format!("failed to decode Turso 0.7 pointer provider id: {error}"))?;
        let export_method = row.get::<String>(3).map_err(|error| {
            format!("failed to decode Turso 0.7 pointer export method: {error}")
        })?;
        let request_fingerprint = row.get::<String>(4).map_err(|error| {
            format!("failed to decode Turso 0.7 pointer request fingerprint: {error}")
        })?;
        let generation_id = CacheGenerationId::from(row.get::<String>(5).map_err(|error| {
            format!("failed to decode Turso 0.7 pointer generation id: {error}")
        })?);
        let schema_ids_json = row
            .get::<String>(6)
            .map_err(|error| format!("failed to decode Turso 0.7 pointer schema ids: {error}"))?;
        let artifact_ids_json = row
            .get::<String>(7)
            .map_err(|error| format!("failed to decode Turso 0.7 pointer artifacts: {error}"))?;
        let file_hashes_json = row
            .get::<String>(8)
            .map_err(|error| format!("failed to decode Turso 0.7 pointer file hashes: {error}"))?;
        let updated_at_ms = row
            .get::<i64>(9)
            .map_err(|error| format!("failed to decode Turso 0.7 pointer timestamp: {error}"))?;
        let lookup_key = active_cache_lookup_key(
            &project_root,
            &language_id,
            &provider_id,
            &export_method,
            &request_fingerprint,
        );
        let generation_key = active_cache_generation_key(
            &project_root,
            &language_id,
            &provider_id,
            &export_method,
            &generation_id,
        );
        statement
            .execute((
                lookup_key.as_str(),
                generation_key.as_str(),
                project_root.as_str(),
                language_id.as_str(),
                provider_id.as_str(),
                export_method.as_str(),
                request_fingerprint.as_str(),
                generation_id.as_str(),
                schema_ids_json.as_str(),
                artifact_ids_json.as_str(),
                file_hashes_json.as_str(),
                updated_at_ms,
            ))
            .await
            .map_err(|error| {
                format!("failed to rebuild Turso 0.7 active-generation pointer: {error}")
            })?;
    }
    Ok(())
}

fn table_family(name: &str, plane: MigrationPlane) -> Option<MigrationFamily> {
    if matches!(plane, MigrationPlane::SearchProjection) {
        return Some(MigrationFamily::StructuralIndex);
    }
    if name == "asp_cache_generation" {
        Some(MigrationFamily::CacheManifest)
    } else if name == "asp_syntax_query_replay" {
        Some(MigrationFamily::SyntaxQuery)
    } else if name.starts_with("asp_source_index_") || name == "asp_exact_selector_projection_v1" {
        Some(MigrationFamily::SourceIndex)
    } else if name == "asp_provider_command_selection" {
        Some(MigrationFamily::ProviderCommand)
    } else if matches!(name, "asp_artifact_event" | "asp_failed_artifact_attempt") {
        Some(MigrationFamily::ArtifactEvent)
    } else if matches!(
        name,
        "asp_artifact_pointer"
            | "asp_artifact_root"
            | "asp_artifact_edge"
            | "asp_artifact_repair_chain_frame"
            | "asp_proof_receipt"
    ) {
        Some(MigrationFamily::ArtifactPointer)
    } else {
        None
    }
}

fn ensure_target_covers_source_tables(
    source_tables: &[MigrationTable],
    target_tables: &[MigrationTable],
) -> Result<(), String> {
    for source in source_tables {
        if !target_tables
            .iter()
            .any(|target| target.name == source.name)
        {
            return Err(format!(
                "Turso 0.7 target schema has no durable v1 table `{}`",
                source.name
            ));
        }
    }
    Ok(())
}

async fn copy_tables(
    source: &turso::Connection,
    target: &turso::Connection,
    tables: &[MigrationTable],
) -> Result<(), String> {
    for table in tables {
        let columns = table_columns(source, &table.name).await?;
        if columns.is_empty() {
            return Err(format!(
                "Turso 0.7 migration table `{}` has no columns",
                table.name
            ));
        }
        let quoted_table = quote_identifier(&table.name)?;
        let quoted_columns = columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Result<Vec<_>, _>>()?;
        let order_by = quoted_columns.join(", ");
        let select_sql = format!("SELECT * FROM {quoted_table} ORDER BY {order_by}");
        let placeholders = (1..=columns.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let insert_sql = format!(
            "INSERT INTO {quoted_table} ({}) VALUES ({placeholders})",
            quoted_columns.join(", ")
        );
        let mut rows = source.query(&select_sql, ()).await.map_err(|error| {
            format!(
                "failed to enumerate legacy Turso table `{}`: {error}",
                table.name
            )
        })?;
        while let Some(row) = rows.next().await.map_err(|error| {
            format!(
                "failed to read legacy Turso table `{}`: {error}",
                table.name
            )
        })? {
            let values = (0..row.column_count())
                .map(|index| {
                    row.get_value(index).map_err(|error| {
                        format!(
                            "failed to copy legacy Turso value {}[{index}]: {error}",
                            table.name
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            target
                .execute(&insert_sql, turso::params_from_iter(values))
                .await
                .map_err(|error| {
                    format!(
                        "failed to replay legacy Turso table `{}` into 0.7 staging: {error}",
                        table.name
                    )
                })?;
        }
    }
    Ok(())
}

async fn summarize_tables(
    connection: &turso::Connection,
    tables: &[MigrationTable],
) -> Result<MigrationSummary, String> {
    let mut summary = MigrationSummary::default();
    for table in tables {
        let columns = table_columns(connection, &table.name).await?;
        let quoted_table = quote_identifier(&table.name)?;
        let order_by = columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let sql = format!("SELECT * FROM {quoted_table} ORDER BY {order_by}");
        let mut rows = connection.query(&sql, ()).await.map_err(|error| {
            format!(
                "failed to verify Turso 0.7 migration table `{}`: {error}",
                table.name
            )
        })?;
        while let Some(row) = rows.next().await.map_err(|error| {
            format!(
                "failed to read Turso 0.7 migration verification row `{}`: {error}",
                table.name
            )
        })? {
            summary.observe(table.family, &table.name, &row)?;
        }
    }
    Ok(summary)
}

async fn table_columns(connection: &turso::Connection, table: &str) -> Result<Vec<String>, String> {
    let quoted_table = quote_identifier(table)?;
    let rows = connection
        .query(&format!("SELECT * FROM {quoted_table} LIMIT 0"), ())
        .await
        .map_err(|error| format!("failed to inspect Turso 0.7 table `{table}`: {error}"))?;
    let columns = rows.column_names();
    for column in &columns {
        validate_identifier(column)?;
    }
    Ok(columns)
}

fn quote_identifier(identifier: &str) -> Result<String, String> {
    validate_identifier(identifier)?;
    Ok(format!("\"{identifier}\""))
}

fn validate_identifier(identifier: &str) -> Result<(), String> {
    if identifier.is_empty()
        || !identifier
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
    {
        return Err(format!(
            "invalid Turso 0.7 migration SQL identifier `{identifier}`"
        ));
    }
    Ok(())
}

const FNV64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV64_PRIME: u64 = 0x100000001b3;

fn update_fnv64(state: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *state ^= u64::from(*byte);
        *state = state.wrapping_mul(FNV64_PRIME);
    }
}

fn update_value_digest(state: &mut u64, value: &turso::Value) {
    match value {
        turso::Value::Null => update_fnv64(state, &[0]),
        turso::Value::Integer(value) => {
            update_fnv64(state, &[1]);
            update_fnv64(state, &value.to_le_bytes());
        }
        turso::Value::Real(value) => {
            update_fnv64(state, &[2]);
            update_fnv64(state, &value.to_bits().to_le_bytes());
        }
        turso::Value::Text(value) => {
            update_fnv64(state, &[3]);
            update_fnv64(state, &(value.len() as u64).to_le_bytes());
            update_fnv64(state, value.as_bytes());
        }
        turso::Value::Blob(value) => {
            update_fnv64(state, &[4]);
            update_fnv64(state, &(value.len() as u64).to_le_bytes());
            update_fnv64(state, value);
        }
    }
}

fn digest_label(record_count: u64, digest: u64) -> String {
    if record_count == 0 {
        EMPTY_FAMILY_DIGEST_V1.to_string()
    } else {
        format!("fnv64:{digest:016x}")
    }
}
