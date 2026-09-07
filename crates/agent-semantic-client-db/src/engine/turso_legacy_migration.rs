// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use agent_semantic_client_core::CacheGenerationId;

use super::turso_cache_key::{active_cache_generation_key, active_cache_lookup_key};
use super::turso_migration::ClientDbTurso07RetiredDerivedReceipt;
use super::turso_migration::{ClientDbTurso07ReplayCoverage, ClientDbTurso07ReplayFamilyReceipt};

const EMPTY_FAMILY_DIGEST_V1: &str =
    "blake3:af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
const FAMILY_COUNT: usize = 9;
const MIGRATION_BATCH_PARAMETER_BUDGET: usize = 768;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MigrationFamily {
    CacheManifest = 0,
    SyntaxQuery = 1,
    SourceIndex = 2,
    StructuralIndex = 3,
    ProviderCommand = 4,
    ArtifactEvent = 5,
    ArtifactPointer = 6,
    WorkspaceRuntime = 7,
    RetiredDerivedProjection = 8,
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
    let migration_started = std::time::Instant::now();
    let (_source_facts_db, source_facts) =
        super::turso::open_turso_0_7_migration_source(source_facts_path).await?;
    let target_facts = super::turso::connect_turso_client_db(target_facts_path).await?;
    let source_fact_tables = list_migration_tables(&source_facts).await?;
    let target_fact_tables = list_migration_tables(&target_facts).await?;
    let source_search_path = super::turso::turso_search_projection_db_path(source_facts_path);
    let target_search =
        super::turso::connect_turso_search_projection_db_for_write(target_facts_path).await?;
    let target_search_tables = list_migration_tables(&target_search).await?;
    let source_search = if source_search_path.is_file() {
        Some(super::turso::open_turso_0_7_migration_source(&source_search_path).await?)
    } else {
        None
    };
    let source_search_tables = match source_search.as_ref() {
        Some((_, connection)) => list_migration_tables(connection).await?,
        None => Vec::new(),
    };

    ensure_target_covers_source_tables(
        &source_fact_tables,
        &target_fact_tables,
        &target_search_tables,
    )?;
    ensure_target_covers_source_tables(
        &source_search_tables,
        &target_fact_tables,
        &target_search_tables,
    )?;
    trace_migration_phase("preflight", migration_started);

    configure_staging_replay(&target_facts, "facts").await?;
    configure_staging_replay(&target_search, "search").await?;
    let fact_transaction = target_facts
        .unchecked_transaction()
        .await
        .map_err(|error| format!("failed to begin facts Turso 0.7 replay transaction: {error}"))?;
    let search_transaction = target_search
        .unchecked_transaction()
        .await
        .map_err(|error| format!("failed to begin search Turso 0.7 replay transaction: {error}"))?;
    let mut retired_fact_summary = MigrationSummary::default();
    let mut retired_search_summary = MigrationSummary::default();
    let replay = async {
        copy_tables(
            &source_facts,
            &fact_transaction,
            &search_transaction,
            &source_fact_tables,
            &mut retired_fact_summary,
            false,
        )
        .await?;
        trace_migration_phase("facts-copy", migration_started);
        if let Some((_, connection)) = source_search.as_ref() {
            copy_tables(
                connection,
                &fact_transaction,
                &search_transaction,
                &source_search_tables,
                &mut retired_search_summary,
                true,
            )
            .await?;
        }
        trace_migration_phase("search-copy", migration_started);
        rebuild_active_cache_generation_pointers(&fact_transaction).await
    }
    .await;
    if let Err(error) = replay {
        let fact_rollback = fact_transaction.rollback().await;
        let search_rollback = search_transaction.rollback().await;
        return match (fact_rollback, search_rollback) {
            (Ok(()), Ok(())) => Err(error),
            (fact, search) => Err(format!(
                "{error}; Turso 0.7 replay rollback failed: facts={fact:?} search={search:?}"
            )),
        };
    }
    fact_transaction
        .commit()
        .await
        .map_err(|error| format!("failed to commit facts Turso 0.7 replay: {error}"))?;
    search_transaction
        .commit()
        .await
        .map_err(|error| format!("failed to commit search Turso 0.7 replay: {error}"))?;
    restore_staging_runtime_mode(&target_facts).await?;
    trace_migration_phase("commit", migration_started);
    let target_fact_summary = summarize_tables(&target_facts, &target_fact_tables).await?;
    trace_migration_phase("facts-summary", migration_started);
    let target_search_summary = summarize_tables(&target_search, &target_search_tables).await?;
    trace_migration_phase("search-summary", migration_started);

    Ok(ClientDbTurso07ReplayCoverage {
        cache_manifest: target_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::CacheManifest),
        syntax_query: target_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::SyntaxQuery),
        source_index: target_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::SourceIndex),
        structural_index: target_search_summary
            .receipt(&target_search_summary, MigrationFamily::StructuralIndex),
        provider_command: target_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::ProviderCommand),
        artifact_event: target_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::ArtifactEvent),
        artifact_pointer: target_fact_summary
            .receipt(&target_fact_summary, MigrationFamily::ArtifactPointer),
        retired_derived_projection: retired_derived_receipt(
            &retired_fact_summary,
            &source_fact_tables,
            &retired_search_summary,
            &source_search_tables,
        ),
    })
}

async fn configure_staging_replay(
    connection: &turso::Connection,
    plane: &str,
) -> Result<(), String> {
    let mut journal_rows = connection
        .query("PRAGMA journal_mode = 'wal'", ())
        .await
        .map_err(|error| {
            format!("failed to configure {plane} Turso 0.7 staging journal: {error}")
        })?;
    let journal_mode = journal_rows
        .next()
        .await
        .map_err(|error| format!("failed to read {plane} Turso 0.7 staging journal: {error}"))?
        .ok_or_else(|| format!("{plane} Turso 0.7 staging journal returned no row"))?
        .get::<String>(0)
        .map_err(|error| format!("failed to decode {plane} Turso 0.7 staging journal: {error}"))?;
    if journal_mode == "mvcc" {
        return Err(format!(
            "{plane} Turso 0.7 staging replay remained in MVCC journal mode"
        ));
    }
    connection
        .execute("PRAGMA synchronous = OFF", ())
        .await
        .map_err(|error| {
            format!("failed to configure {plane} Turso 0.7 staging durability: {error}")
        })?;
    connection
        .execute("PRAGMA temp_store = MEMORY", ())
        .await
        .map_err(|error| {
            format!("failed to configure {plane} Turso 0.7 staging temp store: {error}")
        })?;
    Ok(())
}

async fn restore_staging_runtime_mode(connection: &turso::Connection) -> Result<(), String> {
    let mut rows = connection
        .query("PRAGMA journal_mode = 'mvcc'", ())
        .await
        .map_err(|error| format!("failed to restore Turso 0.7 runtime journal mode: {error}"))?;
    let journal_mode = rows
        .next()
        .await
        .map_err(|error| format!("failed to read restored Turso 0.7 journal mode: {error}"))?
        .ok_or_else(|| "restored Turso 0.7 journal mode returned no row".to_string())?
        .get::<String>(0)
        .map_err(|error| format!("failed to decode restored Turso 0.7 journal mode: {error}"))?;
    if journal_mode != "mvcc" {
        return Err(format!(
            "Turso 0.7 migration restored journal mode `{journal_mode}`, expected `mvcc`"
        ));
    }
    Ok(())
}

fn trace_migration_phase(phase: &str, started: std::time::Instant) {
    if std::env::var_os("ASP_TURSO_MIGRATION_TIMINGS").is_some() {
        eprintln!(
            "[turso-0-7-migration-timing] phase={phase} elapsedMs={:.3}",
            started.elapsed().as_secs_f64() * 1_000.0
        );
    }
}

fn retired_derived_receipt(
    facts: &MigrationSummary,
    fact_tables: &[MigrationTable],
    search: &MigrationSummary,
    search_tables: &[MigrationTable],
) -> ClientDbTurso07RetiredDerivedReceipt {
    let index = MigrationFamily::RetiredDerivedProjection as usize;
    let source_record_count =
        facts.record_counts[index].saturating_add(search.record_counts[index]);
    let source_digest = if source_record_count == 0 {
        EMPTY_FAMILY_DIGEST_V1.to_string()
    } else {
        let mut digest = FNV64_OFFSET_BASIS;
        update_fnv64(
            &mut digest,
            digest_label(facts.record_counts[index], facts.digests[index]).as_bytes(),
        );
        update_fnv64(&mut digest, &[0xfd]);
        update_fnv64(
            &mut digest,
            digest_label(search.record_counts[index], search.digests[index]).as_bytes(),
        );
        format!("fnv64:{digest:016x}")
    };
    let mut source_tables = fact_tables
        .iter()
        .chain(search_tables)
        .filter(|table| table.family == MigrationFamily::RetiredDerivedProjection)
        .map(|table| table.name.clone())
        .collect::<Vec<_>>();
    source_tables.sort();
    source_tables.dedup();
    ClientDbTurso07RetiredDerivedReceipt::preserved(
        source_record_count,
        source_digest,
        source_tables,
    )
}

async fn list_migration_tables(
    connection: &turso::Connection,
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
    let mut unmapped = Vec::new();
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
        match table_family(&name) {
            Some(family) => tables.push(MigrationTable { name, family }),
            None => unmapped.push(name),
        }
    }
    if !unmapped.is_empty() {
        return Err(format!(
            "unmapped durable v1 tables block Turso 0.7 migration: {}",
            unmapped.join(", ")
        ));
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

fn table_family(name: &str) -> Option<MigrationFamily> {
    if matches!(
        name,
        "asp_search_projection_generation" | "asp_search_projection_document"
    ) {
        // Pre-route projection rows have no storage-profile or planner-decision
        // identity. They cannot be admitted into the server-first search plane.
        Some(MigrationFamily::RetiredDerivedProjection)
    } else if name == "asp_cache_generation" {
        Some(MigrationFamily::CacheManifest)
    } else if name == "asp_syntax_query_replay" {
        Some(MigrationFamily::SyntaxQuery)
    } else if name == "asp_exact_selector_projection_v1" {
        Some(MigrationFamily::RetiredDerivedProjection)
    } else if name.starts_with("asp_source_index_") {
        Some(MigrationFamily::SourceIndex)
    } else if matches!(
        name,
        "asp_workspace_db_schema_receipt_v1" | "asp_workspace_generation_materialization_v1"
    ) {
        Some(MigrationFamily::WorkspaceRuntime)
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
    } else if matches!(
        name,
        "asp_graph_artifact"
            | "asp_graph_artifact_entity"
            | "asp_graph_artifact_edge"
            | "asp_search_document"
            | "asp_overlay_document"
            | "asp_route_receipt"
    ) {
        Some(MigrationFamily::RetiredDerivedProjection)
    } else {
        None
    }
}

fn ensure_target_covers_source_tables(
    source_tables: &[MigrationTable],
    target_fact_tables: &[MigrationTable],
    target_search_tables: &[MigrationTable],
) -> Result<(), String> {
    let target_fact_names = target_fact_tables
        .iter()
        .map(|table| table.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let target_search_names = target_search_tables
        .iter()
        .map(|table| table.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let missing = source_tables
        .iter()
        .filter(|source| source.family != MigrationFamily::RetiredDerivedProjection)
        .filter(|source| {
            let target_names = if source.family == MigrationFamily::StructuralIndex {
                &target_search_names
            } else {
                &target_fact_names
            };
            !target_names.contains(source.name.as_str())
        })
        .map(|source| source.name.clone())
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Turso 0.7 target schema has no durable v1 tables: {}",
            missing.join(", ")
        ))
    }
}

async fn copy_tables(
    source: &turso::Connection,
    target_facts: &turso::Connection,
    target_search: &turso::Connection,
    tables: &[MigrationTable],
    retired_summary: &mut MigrationSummary,
    allow_identical_duplicates: bool,
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
        let select_sql = format!("SELECT * FROM {quoted_table}");
        let mut rows = source.query(&select_sql, ()).await.map_err(|error| {
            format!(
                "failed to enumerate legacy Turso table `{}`: {error}",
                table.name
            )
        })?;
        if table.family == MigrationFamily::RetiredDerivedProjection {
            while let Some(row) = rows.next().await.map_err(|error| {
                format!(
                    "failed to read retired legacy Turso table `{}`: {error}",
                    table.name
                )
            })? {
                retired_summary.observe(table.family, &table.name, &row)?;
            }
            continue;
        }
        let target = if table.family == MigrationFamily::StructuralIndex {
            target_search
        } else {
            target_facts
        };
        let batch_row_limit = (MIGRATION_BATCH_PARAMETER_BUDGET / columns.len()).max(1);
        let mut batch = Vec::with_capacity(batch_row_limit);
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
            batch.push(values);
            if batch.len() == batch_row_limit {
                flush_insert_batch(
                    target,
                    table,
                    &quoted_columns,
                    &batch,
                    allow_identical_duplicates,
                )
                .await?;
                batch.clear();
            }
        }
        if !batch.is_empty() {
            flush_insert_batch(
                target,
                table,
                &quoted_columns,
                &batch,
                allow_identical_duplicates,
            )
            .await?;
        }
    }
    Ok(())
}

async fn flush_insert_batch(
    target: &turso::Connection,
    table: &MigrationTable,
    quoted_columns: &[String],
    batch: &[Vec<turso::Value>],
    allow_identical_duplicates: bool,
) -> Result<(), String> {
    let quoted_table = quote_identifier(&table.name)?;
    let mut parameter_index = 1usize;
    let rows = batch
        .iter()
        .map(|values| {
            let placeholders = values
                .iter()
                .map(|_| {
                    let placeholder = format!("?{parameter_index}");
                    parameter_index += 1;
                    placeholder
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("({placeholders})")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let insert_sql = format!(
        "{} INTO {quoted_table} ({}) VALUES {rows}",
        if allow_identical_duplicates {
            "INSERT OR IGNORE"
        } else {
            "INSERT"
        },
        quoted_columns.join(", ")
    );
    let values = batch
        .iter()
        .flat_map(|row| row.iter().cloned())
        .collect::<Vec<_>>();
    let mut statement = target.prepare_cached(&insert_sql).await.map_err(|error| {
        format!(
            "failed to prepare Turso 0.7 migration batch for `{}`: {error}",
            table.name
        )
    })?;
    let inserted = statement
        .execute(turso::params_from_iter(values))
        .await
        .map_err(|error| {
            format!(
                "failed to replay legacy Turso batch `{}` into 0.7 staging: {error}",
                table.name
            )
        })?;
    if !allow_identical_duplicates || inserted as usize == batch.len() {
        return Ok(());
    }
    for values in batch {
        verify_migrated_row(target, table, quoted_columns, values).await?;
    }
    Ok(())
}

async fn verify_migrated_row(
    target: &turso::Connection,
    table: &MigrationTable,
    quoted_columns: &[String],
    values: &[turso::Value],
) -> Result<(), String> {
    let quoted_table = quote_identifier(&table.name)?;
    let equality = quoted_columns
        .iter()
        .enumerate()
        .map(|(index, column)| format!("{column} IS ?{}", index + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    let verify_sql = format!("SELECT 1 FROM {quoted_table} WHERE {equality} LIMIT 1");
    let mut existing = target
        .query(&verify_sql, turso::params_from_iter(values.iter().cloned()))
        .await
        .map_err(|error| {
            format!(
                "failed to verify duplicate Turso 0.7 migration row `{}`: {error}",
                table.name
            )
        })?;
    if existing
        .next()
        .await
        .map_err(|error| {
            format!(
                "failed to read duplicate Turso 0.7 migration row `{}`: {error}",
                table.name
            )
        })?
        .is_none()
    {
        return Err(format!(
            "conflicting duplicate row blocks Turso 0.7 migration table `{}`",
            table.name
        ));
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
