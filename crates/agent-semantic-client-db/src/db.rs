//! SQLite storage adapter for local `agent-semantic-client` cache state.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheGenerationId, ClientCacheFileHash,
    ClientCacheGeneration, ClientCacheManifest, ClientDbStatus, LanguageId, ProviderId,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

use crate::pragmas::{
    ClientDbRuntimePragmas, configure_readable_connection, configure_writable_connection,
    read_runtime_pragmas,
};
use crate::syntax_query::{
    SYNTAX_QUERY_ROW_ABI_META_KEY, SYNTAX_QUERY_ROW_ABI_VERSION, compact_source_locator,
    parse_syntax_query_packet_import, write_syntax_query_import_rows,
};

pub use crate::syntax_query::{
    ClientDbSyntaxCaptureReplay, ClientDbSyntaxNodeType, ClientDbSyntaxQueryInputKind,
    ClientDbSyntaxQueryLookup, ClientDbSyntaxQueryReplay,
};

/// File name used for the local SQLite client cache.
pub const AGENT_SEMANTIC_CLIENT_DB_FILE: &str = "client.sqlite3";
/// Current released SQLite schema version for the local agent semantic client DB.
///
/// Keep this stable until a versioned release/migration boundary is declared.
/// Internal cache tables are added through idempotent migrations and do not by
/// themselves advance the public schema contract.
pub const AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION: i64 = 1;

/// Read-only diagnostic summary for a local SQLite client DB path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbReport {
    pub db_path: PathBuf,
    pub status: ClientDbStatus,
    pub generation_count: u32,
    pub syntax_row_generation_count: u32,
    pub syntax_row_match_count: u32,
    pub syntax_row_capture_count: u32,
    pub structural_index_generation_count: u32,
    pub structural_index_owner_count: u32,
    pub structural_index_symbol_count: u32,
    pub structural_index_dependency_usage_count: u32,
    pub artifact_event_count: u32,
    pub raw_source_stored: bool,
    pub runtime_pragmas: Option<ClientDbRuntimePragmas>,
    pub reason: Option<String>,
}

/// Named lookup request for one provider cache generation probe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbGenerationLookup {
    pub db_path: PathBuf,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub project_root: PathBuf,
    pub export_method: CacheExportMethod,
    pub request_fingerprint: Option<String>,
}

/// Matching cache generation metadata returned by a DB lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbGenerationHit {
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub project_root: PathBuf,
    pub export_method: CacheExportMethod,
    pub schema_ids: Vec<agent_semantic_client_core::SemanticSchemaId>,
    pub request_fingerprint: Option<String>,
    pub file_hashes: Vec<ClientCacheFileHash>,
    pub artifact_ids: Vec<CacheArtifactId>,
}

/// Cached provider command selection for one activation context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbProviderCommandSelection {
    manifest_id: String,
    manifest_digest: String,
    language_id: String,
    provider_id: String,
    binary: String,
    execution: String,
    provider_command_prefix: Vec<String>,
    executable_path: Option<String>,
    executable_len: Option<i64>,
    executable_mtime_ms: Option<i64>,
}

impl ClientDbProviderCommandSelection {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        manifest_id: String,
        manifest_digest: String,
        language_id: String,
        provider_id: String,
        binary: String,
        execution: String,
        provider_command_prefix: Vec<String>,
        executable_path: Option<String>,
        executable_len: Option<i64>,
        executable_mtime_ms: Option<i64>,
    ) -> Self {
        Self {
            manifest_id,
            manifest_digest,
            language_id,
            provider_id,
            binary,
            execution,
            provider_command_prefix,
            executable_path,
            executable_len,
            executable_mtime_ms,
        }
    }

    #[must_use]
    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    #[must_use]
    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    #[must_use]
    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    #[must_use]
    pub fn binary(&self) -> &str {
        &self.binary
    }

    #[must_use]
    pub fn execution(&self) -> &str {
        &self.execution
    }

    #[must_use]
    pub fn provider_command_prefix(&self) -> &[String] {
        &self.provider_command_prefix
    }

    #[must_use]
    pub fn executable_path(&self) -> Option<&str> {
        self.executable_path.as_deref()
    }

    #[must_use]
    pub fn executable_len(&self) -> Option<i64> {
        self.executable_len
    }

    #[must_use]
    pub fn executable_mtime_ms(&self) -> Option<i64> {
        self.executable_mtime_ms
    }
}

/// Graph-turbo artifact event row stored in Rust SQLite for fast timeline audits.
///
/// Stringly state boundary: serialized SQLite rows keep schema-owned graph-turbo
/// artifact event tokens for lossless JSON handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbArtifactEvent {
    pub artifact_path: String,
    pub event_ordinal: u32,
    pub timestamp_ms: i64,
    pub kind: String,
    pub language: String,
    pub method: String,
    pub target: String,
    pub query: String,
    pub project_root: String,
    pub project_root_arg: String,
    pub bytes: u64,
}

type CacheGenerationRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
);

/// Open SQLite client DB handle.
#[derive(Debug)]
pub struct ClientDb {
    pub(crate) conn: Connection,
    pub(crate) db_path: PathBuf,
}

impl ClientDb {
    /// Return the default SQLite DB path under a client cache root.
    #[must_use]
    pub fn default_path(cache_root: impl AsRef<Path>) -> PathBuf {
        cache_root.as_ref().join(AGENT_SEMANTIC_CLIENT_DB_FILE)
    }

    /// Return the SQLite DB path backing this handle.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.db_path
    }

    /// Inspect an existing DB path without creating or migrating it.
    #[must_use]
    pub fn inspect(db_path: impl AsRef<Path>) -> ClientDbReport {
        let db_path = db_path.as_ref().to_path_buf();
        if !db_path.exists() {
            return ClientDbReport {
                db_path,
                status: ClientDbStatus::Missing,
                generation_count: 0,
                syntax_row_generation_count: 0,
                syntax_row_match_count: 0,
                syntax_row_capture_count: 0,
                structural_index_generation_count: 0,
                structural_index_owner_count: 0,
                structural_index_symbol_count: 0,
                structural_index_dependency_usage_count: 0,
                artifact_event_count: 0,
                raw_source_stored: false,
                runtime_pragmas: None,
                reason: None,
            };
        }

        match Self::open_read_only(&db_path).and_then(|db| db.inspect_open()) {
            Ok(report) => report,
            Err(error) => ClientDbReport {
                db_path,
                status: ClientDbStatus::Invalid,
                generation_count: 0,
                syntax_row_generation_count: 0,
                syntax_row_match_count: 0,
                syntax_row_capture_count: 0,
                structural_index_generation_count: 0,
                structural_index_owner_count: 0,
                structural_index_symbol_count: 0,
                structural_index_dependency_usage_count: 0,
                artifact_event_count: 0,
                raw_source_stored: false,
                runtime_pragmas: None,
                reason: Some(error),
            },
        }
    }

    /// Open an existing SQLite DB in read-only mode.
    ///
    /// Returns `Ok(None)` when the path is missing so callers can avoid a
    /// second path inspection before a hot cache lookup.
    pub fn open_read_only_existing(db_path: impl AsRef<Path>) -> Result<Option<Self>, String> {
        let db_path = db_path.as_ref();
        if !db_path.exists() {
            return Ok(None);
        }
        Self::open_read_only(db_path).map(Some)
    }

    /// Inspect this already opened DB handle without reopening the SQLite file.
    pub fn inspect_open(&self) -> Result<ClientDbReport, String> {
        let summary = self.summary()?;
        let runtime_pragmas = self.runtime_pragmas()?;
        Ok(ClientDbReport {
            db_path: self.db_path.clone(),
            status: ClientDbStatus::Present,
            generation_count: summary.generation_count,
            syntax_row_generation_count: summary.syntax_row_generation_count,
            syntax_row_match_count: summary.syntax_row_match_count,
            syntax_row_capture_count: summary.syntax_row_capture_count,
            structural_index_generation_count: summary.structural_index_generation_count,
            structural_index_owner_count: summary.structural_index_owner_count,
            structural_index_symbol_count: summary.structural_index_symbol_count,
            structural_index_dependency_usage_count: summary
                .structural_index_dependency_usage_count,
            artifact_event_count: summary.artifact_event_count,
            raw_source_stored: summary.raw_source_stored,
            runtime_pragmas: Some(runtime_pragmas),
            reason: None,
        })
    }

    /// Open the SQLite DB and run idempotent schema migration.
    pub fn open_or_create(db_path: impl AsRef<Path>) -> Result<Self, String> {
        let db_path = db_path.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create agent semantic client db directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let conn = Connection::open(&db_path).map_err(|error| {
            format!(
                "failed to open agent semantic client db at {}: {error}",
                db_path.display()
            )
        })?;
        configure_writable_connection(&conn, &db_path)?;
        let db = Self { conn, db_path };
        db.migrate()?;
        Ok(db)
    }

    /// Import manifest generations into SQL rows.
    pub fn import_manifest(&mut self, manifest: &ClientCacheManifest) -> Result<(), String> {
        if manifest
            .generations
            .iter()
            .any(|generation| generation.raw_source_stored)
        {
            return Err(
                "client db refuses cache generations with rawSourceStored=true".to_string(),
            );
        }

        let tx = self.conn.transaction().map_err(|error| {
            format!(
                "failed to start agent semantic client db transaction at {}: {error}",
                self.db_path.display()
            )
        })?;
        for generation in &manifest.generations {
            let project_root = normalized_project_root(Path::new(&generation.project_root));
            let schema_ids_json = serde_json::to_string(&generation.schema_ids)
                .map_err(|error| format!("failed to serialize schema ids: {error}"))?;
            let artifact_ids_json =
                serde_json::to_string(generation.artifact_ids.as_deref().unwrap_or(&[]))
                    .map_err(|error| format!("failed to serialize artifact ids: {error}"))?;
            let file_hashes_json =
                serde_json::to_string(generation.file_hashes.as_deref().unwrap_or(&[]))
                    .map_err(|error| format!("failed to serialize file hashes: {error}"))?;
            tx.execute(
                "INSERT INTO cache_generations (
                generation_id,
                language_id,
                provider_id,
                provider_version,
                export_method,
                project_root,
                package_root,
                schema_ids_json,
                cache_status,
                raw_source_stored,
                request_fingerprint,
                artifact_ids_json,
                file_hashes_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11, ?12)
            ON CONFLICT(generation_id) DO UPDATE SET
                language_id = excluded.language_id,
                provider_id = excluded.provider_id,
                provider_version = excluded.provider_version,
                export_method = excluded.export_method,
                project_root = excluded.project_root,
                package_root = excluded.package_root,
                schema_ids_json = excluded.schema_ids_json,
                cache_status = excluded.cache_status,
                raw_source_stored = excluded.raw_source_stored,
                request_fingerprint = excluded.request_fingerprint,
                artifact_ids_json = excluded.artifact_ids_json,
                file_hashes_json = excluded.file_hashes_json,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![
                    generation.generation_id.as_str(),
                    generation.language_id.as_str(),
                    generation.provider_id.as_str(),
                    generation.provider_version.as_deref(),
                    generation.export_method.as_deref(),
                    project_root,
                    generation.package_root.as_deref(),
                    schema_ids_json,
                    generation.cache_status.as_str(),
                    generation.request_fingerprint.as_deref(),
                    artifact_ids_json,
                    file_hashes_json,
                ],
            )
            .map_err(|error| format!("failed to write cache generation: {error}"))?;
        }
        tx.commit()
            .map_err(|error| format!("failed to commit cache generation import: {error}"))?;
        Ok(())
    }

    /// Replace cached provider command selections for a project/context pair.
    pub fn replace_provider_command_selections(
        &mut self,
        project_root: &Path,
        context_fingerprint: &str,
        selections: &[ClientDbProviderCommandSelection],
    ) -> Result<(), String> {
        let project_root = normalized_project_root(project_root);
        let tx = self.conn.transaction().map_err(|error| {
            format!(
                "failed to start provider command selection transaction at {}: {error}",
                self.db_path.display()
            )
        })?;
        tx.execute(
            "DELETE FROM provider_command_selection WHERE project_root = ?1",
            params![&project_root],
        )
        .map_err(|error| format!("failed to delete provider command selections: {error}"))?;
        for selection in selections {
            let command_prefix_json = serde_json::to_string(&selection.provider_command_prefix)
                .map_err(|error| format!("failed to serialize provider command prefix: {error}"))?;
            tx.execute(
                "INSERT INTO provider_command_selection (
                    project_root,
                    context_fingerprint,
                    manifest_id,
                    manifest_digest,
                    language_id,
                    provider_id,
                    binary,
                    execution,
                    provider_command_prefix_json,
                    executable_path,
                    executable_len,
                    executable_mtime_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    &project_root,
                    context_fingerprint,
                    &selection.manifest_id,
                    &selection.manifest_digest,
                    &selection.language_id,
                    &selection.provider_id,
                    &selection.binary,
                    &selection.execution,
                    command_prefix_json,
                    &selection.executable_path,
                    &selection.executable_len,
                    &selection.executable_mtime_ms,
                ],
            )
            .map_err(|error| format!("failed to write provider command selection: {error}"))?;
        }
        tx.commit()
            .map_err(|error| format!("failed to commit provider command selections: {error}"))?;
        Ok(())
    }

    /// Return cached provider command selections for a project/context pair.
    pub fn lookup_provider_command_selections(
        &self,
        project_root: &Path,
        context_fingerprint: &str,
    ) -> Result<Option<Vec<ClientDbProviderCommandSelection>>, String> {
        let project_root = normalized_project_root(project_root);
        let mut statement = self
            .conn
            .prepare(
                "SELECT
                    manifest_id,
                    manifest_digest,
                    language_id,
                    provider_id,
                    binary,
                    execution,
                    provider_command_prefix_json,
                    executable_path,
                    executable_len,
                    executable_mtime_ms
                FROM provider_command_selection
                WHERE project_root = ?1 AND context_fingerprint = ?2
                ORDER BY language_id, provider_id, manifest_id",
            )
            .map_err(|error| {
                format!("failed to prepare provider command selection lookup: {error}")
            })?;
        let rows = statement
            .query_map(params![project_root, context_fingerprint], |row| {
                let command_prefix_json = row.get::<_, String>(6)?;
                let provider_command_prefix =
                    serde_json::from_str::<Vec<String>>(&command_prefix_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                Ok(ClientDbProviderCommandSelection::new(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    provider_command_prefix,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                ))
            })
            .map_err(|error| format!("failed to read provider command selections: {error}"))?;
        let mut selections = Vec::new();
        for row in rows {
            selections.push(
                row.map_err(|error| format!("failed to read provider command selection: {error}"))?,
            );
        }
        Ok((!selections.is_empty()).then_some(selections))
    }

    /// Import normalized rows derived from a validated `semantic-tree-sitter-query` packet.
    pub fn import_semantic_tree_sitter_query_packet(
        &mut self,
        generation: &ClientCacheGeneration,
        packet_bytes: &[u8],
    ) -> Result<(), String> {
        let parsed = parse_syntax_query_packet_import(generation, packet_bytes)?;
        let tx = self.conn.transaction().map_err(|error| {
            format!(
                "failed to start syntax query row import transaction at {}: {error}",
                self.db_path.display()
            )
        })?;
        write_syntax_query_import_rows(&tx, &parsed)?;
        tx.commit()
            .map_err(|error| format!("failed to commit syntax query row import: {error}"))?;
        Ok(())
    }

    /// Upsert graph-turbo artifact events into the local timeline index.
    pub fn upsert_artifact_events(
        &mut self,
        events: &[ClientDbArtifactEvent],
    ) -> Result<u32, String> {
        let tx = self.conn.transaction().map_err(|error| {
            format!(
                "failed to start artifact event import transaction at {}: {error}",
                self.db_path.display()
            )
        })?;
        let mut written = 0_u32;
        for event in events {
            tx.execute(
                "INSERT INTO artifact_event (
                    artifact_path,
                    event_ordinal,
                    timestamp_ms,
                    kind,
                    language,
                    method,
                    target,
                    query,
                    project_root,
                    project_root_arg,
                    bytes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                ON CONFLICT(artifact_path, event_ordinal) DO UPDATE SET
                    timestamp_ms = excluded.timestamp_ms,
                    kind = excluded.kind,
                    language = excluded.language,
                    method = excluded.method,
                    target = excluded.target,
                    query = excluded.query,
                    project_root = excluded.project_root,
                    project_root_arg = excluded.project_root_arg,
                    bytes = excluded.bytes,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![
                    event.artifact_path.as_str(),
                    i64::from(event.event_ordinal),
                    event.timestamp_ms,
                    event.kind.as_str(),
                    event.language.as_str(),
                    event.method.as_str(),
                    event.target.as_str(),
                    event.query.as_str(),
                    event.project_root.as_str(),
                    event.project_root_arg.as_str(),
                    event.bytes.min(i64::MAX as u64) as i64,
                ],
            )
            .map_err(|error| format!("failed to write artifact event: {error}"))?;
            written = written.saturating_add(1);
        }
        tx.commit()
            .map_err(|error| format!("failed to commit artifact event import: {error}"))?;
        Ok(written)
    }

    /// Delete all local generation rows from an existing DB without touching provider artifacts.
    pub fn invalidate_generations(db_path: impl AsRef<Path>) -> Result<u32, String> {
        let db_path = db_path.as_ref();
        if !db_path.exists() {
            return Ok(0);
        }
        let db = Self::open_or_create(db_path)?;
        db.conn
            .execute("DELETE FROM cache_generations", [])
            .map(|count| count.min(u32::MAX as usize) as u32)
            .map_err(|error| {
                format!(
                    "failed to invalidate agent semantic client db generations at {}: {error}",
                    db.db_path.display()
                )
            })
    }

    /// Delete local generation rows for one project root without touching provider artifacts.
    pub fn invalidate_generations_for_project(
        db_path: impl AsRef<Path>,
        project_root: impl AsRef<Path>,
    ) -> Result<u32, String> {
        let db_path = db_path.as_ref();
        if !db_path.exists() {
            return Ok(0);
        }
        let db = Self::open_or_create(db_path)?;
        let project_root = normalized_project_root(project_root.as_ref());
        db.conn
            .execute(
                "DELETE FROM cache_generations WHERE project_root = ?1",
                params![project_root],
            )
            .map(|count| count.min(u32::MAX as usize) as u32)
            .map_err(|error| {
                format!(
                    "failed to invalidate agent semantic client db generations for project at {}: {error}",
                    db.db_path.display()
                )
            })
    }

    /// Delete normalized syntax query rows without touching cache generations or artifacts.
    pub fn flush_syntax_query_rows(db_path: impl AsRef<Path>) -> Result<u32, String> {
        let db_path = db_path.as_ref();
        if !db_path.exists() {
            return Ok(0);
        }
        let db = Self::open_or_create(db_path)?;
        let flushed = db
            .conn
            .execute("DELETE FROM syntax_query_generation", [])
            .map(|count| count.min(u32::MAX as usize) as u32)
            .map_err(|error| {
                format!(
                    "failed to flush syntax query row cache at {}: {error}",
                    db.db_path.display()
                )
            })?;
        db.conn
            .execute(
                "INSERT OR REPLACE INTO schema_meta (key, value) VALUES (?1, ?2)",
                params![SYNTAX_QUERY_ROW_ABI_META_KEY, SYNTAX_QUERY_ROW_ABI_VERSION],
            )
            .map_err(|error| {
                format!(
                    "failed to write syntax query row ABI version at {}: {error}",
                    db.db_path.display()
                )
            })?;
        Ok(flushed)
    }

    /// Return true when a matching cache generation is present.
    pub fn has_generation(lookup: &ClientDbGenerationLookup) -> Result<bool, String> {
        Ok(Self::lookup_generation(lookup)?.is_some())
    }

    /// Return matching generation artifact metadata when present.
    pub fn lookup_generation(
        lookup: &ClientDbGenerationLookup,
    ) -> Result<Option<ClientDbGenerationHit>, String> {
        let db_path = lookup.db_path.as_path();
        if !db_path.exists() {
            return Ok(None);
        }
        Self::open_read_only(db_path)?.lookup_generation_for(
            &lookup.language_id,
            &lookup.provider_id,
            &lookup.project_root,
            &lookup.export_method,
            lookup.request_fingerprint.as_deref(),
        )
    }

    /// Return matching generation metadata using this already opened DB handle.
    pub fn lookup_generation_open(
        &self,
        lookup: &ClientDbGenerationLookup,
    ) -> Result<Option<ClientDbGenerationHit>, String> {
        self.lookup_generation_for(
            &lookup.language_id,
            &lookup.provider_id,
            &lookup.project_root,
            &lookup.export_method,
            lookup.request_fingerprint.as_deref(),
        )
    }

    /// Return recent matching generation artifact metadata, newest first.
    pub fn lookup_recent_generations(
        lookup: &ClientDbGenerationLookup,
        limit: u32,
    ) -> Result<Vec<ClientDbGenerationHit>, String> {
        let db_path = lookup.db_path.as_path();
        if !db_path.exists() || limit == 0 {
            return Ok(Vec::new());
        }
        Self::open_read_only(db_path)?.lookup_recent_generations_for(
            &lookup.language_id,
            &lookup.provider_id,
            &lookup.project_root,
            &lookup.export_method,
            lookup.request_fingerprint.as_deref(),
            limit,
        )
    }

    /// Return recent matching generation metadata using this opened DB handle.
    pub fn lookup_recent_generations_open(
        &self,
        lookup: &ClientDbGenerationLookup,
        limit: u32,
    ) -> Result<Vec<ClientDbGenerationHit>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        self.lookup_recent_generations_for(
            &lookup.language_id,
            &lookup.provider_id,
            &lookup.project_root,
            &lookup.export_method,
            lookup.request_fingerprint.as_deref(),
            limit,
        )
    }

    /// Return graph-turbo artifact events from the local SQLite index, oldest first.
    pub fn lookup_artifact_events(
        db_path: impl AsRef<Path>,
        since_timestamp_ms: Option<i64>,
        limit: u32,
    ) -> Result<Vec<ClientDbArtifactEvent>, String> {
        let db_path = db_path.as_ref();
        if !db_path.exists() || limit == 0 {
            return Ok(Vec::new());
        }
        Self::open_read_only(db_path)?.lookup_artifact_events_for(since_timestamp_ms, limit)
    }

    /// Return normalized syntax query rows for an exact request fingerprint.
    pub fn lookup_syntax_query_replay(
        lookup: &ClientDbSyntaxQueryLookup,
    ) -> Result<Option<ClientDbSyntaxQueryReplay>, String> {
        let db_path = lookup.db_path.as_path();
        if !db_path.exists() {
            return Ok(None);
        }
        Self::open_read_only(db_path)?.lookup_syntax_query_replay_for(lookup)
    }

    /// Return normalized syntax query rows using this already opened DB handle.
    pub fn lookup_syntax_query_replay_open(
        &self,
        lookup: &ClientDbSyntaxQueryLookup,
    ) -> Result<Option<ClientDbSyntaxQueryReplay>, String> {
        self.lookup_syntax_query_replay_for(lookup)
    }

    /// Return aggregate cache generation counts from the DB.
    pub fn summary(&self) -> Result<ClientDbSummary, String> {
        let (generation_count, raw_source_stored): (i64, i64) = self
            .conn
            .query_row(
                "SELECT COUNT(*), COALESCE(MAX(raw_source_stored), 0) FROM cache_generations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| format!("failed to read client db cache summary: {error}"))?;
        let table_counts = self.table_counts()?;
        Ok(ClientDbSummary {
            generation_count: generation_count.max(0).min(i64::from(u32::MAX)) as u32,
            syntax_row_generation_count: table_counts.syntax_row_generation_count,
            syntax_row_match_count: table_counts.syntax_row_match_count,
            syntax_row_capture_count: table_counts.syntax_row_capture_count,
            structural_index_generation_count: table_counts.structural_index_generation_count,
            structural_index_owner_count: table_counts.structural_index_owner_count,
            structural_index_symbol_count: table_counts.structural_index_symbol_count,
            structural_index_dependency_usage_count: table_counts
                .structural_index_dependency_usage_count,
            artifact_event_count: table_counts.artifact_event_count,
            raw_source_stored: raw_source_stored != 0,
        })
    }

    /// Return runtime SQLite pragmas observed on this DB connection.
    pub fn runtime_pragmas(&self) -> Result<ClientDbRuntimePragmas, String> {
        read_runtime_pragmas(&self.conn, &self.db_path)
    }

    fn table_counts(&self) -> Result<ClientDbTableCounts, String> {
        const COUNT_TABLES: &[&str] = &[
            "syntax_query_generation",
            "syntax_query_match",
            "syntax_query_capture",
            "structural_index_generation",
            "structural_index_owner",
            "structural_index_symbol",
            "structural_index_dependency_usage",
            "artifact_event",
        ];

        let existing_tables = self.existing_tables()?;
        let selects = COUNT_TABLES
            .iter()
            .filter(|table| existing_tables.iter().any(|existing| existing == **table))
            .map(|table| {
                format!("SELECT '{table}' AS table_name, COUNT(*) AS row_count FROM {table}")
            })
            .collect::<Vec<_>>();
        if selects.is_empty() {
            return Ok(ClientDbTableCounts::default());
        }

        let sql = selects.join(" UNION ALL ");
        let mut statement = self
            .conn
            .prepare_cached(&sql)
            .map_err(|error| format!("failed to prepare client db table counts: {error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|error| format!("failed to read client db table counts: {error}"))?;
        let mut counts = ClientDbTableCounts::default();
        for row in rows {
            let (table, count) =
                row.map_err(|error| format!("failed to decode client db table count: {error}"))?;
            counts.set(&table, count);
        }
        Ok(counts)
    }

    fn existing_tables(&self) -> Result<Vec<String>, String> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT name FROM sqlite_master WHERE type = 'table'")
            .map_err(|error| format!("failed to prepare client db table list: {error}"))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| format!("failed to read client db table list: {error}"))?;
        let mut tables = Vec::new();
        for row in rows {
            tables.push(row.map_err(|error| format!("failed to decode table name: {error}"))?);
        }
        Ok(tables)
    }

    fn lookup_generation_for(
        &self,
        language_id: &LanguageId,
        provider_id: &ProviderId,
        project_root: &Path,
        export_method: &CacheExportMethod,
        request_fingerprint: Option<&str>,
    ) -> Result<Option<ClientDbGenerationHit>, String> {
        Ok(self
            .lookup_recent_generations_for(
                language_id,
                provider_id,
                project_root,
                export_method,
                request_fingerprint,
                1,
            )?
            .into_iter()
            .next())
    }

    fn lookup_recent_generations_for(
        &self,
        language_id: &LanguageId,
        provider_id: &ProviderId,
        project_root: &Path,
        export_method: &CacheExportMethod,
        request_fingerprint: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ClientDbGenerationHit>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let project_root = normalized_project_root(project_root);
        let mut statement = self
            .conn
            .prepare(
                "SELECT language_id,
                    provider_id,
                    project_root,
                    export_method,
                    schema_ids_json,
                    request_fingerprint,
                    artifact_ids_json,
                    file_hashes_json
             FROM cache_generations
             WHERE language_id = ?1
               AND provider_id = ?2
               AND project_root = ?3
               AND export_method = ?4
               AND (?5 IS NULL OR request_fingerprint = ?5)
               AND raw_source_stored = 0
             ORDER BY updated_at DESC
             LIMIT ?6",
            )
            .map_err(|error| {
                format!("failed to prepare client db cache generation query: {error}")
            })?;
        let row_iter = statement
            .query_map(
                params![
                    language_id.as_str(),
                    provider_id.as_str(),
                    project_root,
                    export_method.as_str(),
                    request_fingerprint,
                    i64::from(limit),
                ],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .map_err(|error| format!("failed to read client db cache generation: {error}"))?;
        let mut hits = Vec::new();
        for row in row_iter {
            hits.push(cache_generation_hit_from_row(row.map_err(|error| {
                format!("failed to decode client db cache generation row: {error}")
            })?)?);
        }
        Ok(hits)
    }

    fn lookup_syntax_query_replay_for(
        &self,
        lookup: &ClientDbSyntaxQueryLookup,
    ) -> Result<Option<ClientDbSyntaxQueryReplay>, String> {
        let project_root = normalized_project_root(&lookup.project_root);
        let row = self
            .conn
            .query_row(
                "SELECT g.generation_id,
                    g.language_id,
                    g.grammar_id,
                    g.grammar_profile_version,
                    g.input_form,
                    g.input_kind,
                    p.compiled_source,
                    p.captures_json,
                    g.query_ast_fingerprint,
                    g.packet_bytes,
                    cg.file_hashes_json
             FROM syntax_query_generation g
             JOIN syntax_query_pattern p ON p.generation_id = g.generation_id
             JOIN cache_generations cg ON cg.generation_id = g.generation_id
             WHERE g.language_id = ?1
               AND g.provider_id = ?2
               AND g.project_root = ?3
               AND g.query_ast_fingerprint = ?4
               AND ((?5 IS NULL AND p.selector IS NULL) OR p.selector = ?5)
               AND g.raw_source_stored = 0
               AND cg.raw_source_stored = 0
               AND g.query_ast_fingerprint IS NOT NULL
             ORDER BY g.updated_at DESC
             LIMIT 1",
                params![
                    lookup.language_id.as_str(),
                    lookup.provider_id.as_str(),
                    project_root,
                    lookup.query_ast_fingerprint.as_str(),
                    lookup.selector.as_deref(),
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, Option<i64>>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("failed to read syntax query replay generation: {error}"))?;
        let Some((
            generation_id,
            language_id,
            grammar_id,
            grammar_profile_version,
            input_form,
            input_kind,
            compiled_source,
            captures_json,
            query_ast_fingerprint,
            packet_bytes,
            file_hashes_json,
        )) = row
        else {
            return Ok(None);
        };
        let captures = serde_json::from_str(&captures_json)
            .map_err(|error| format!("failed to parse syntax capture names: {error}"))?;
        let file_hashes = serde_json::from_str(&file_hashes_json)
            .map_err(|error| format!("failed to parse syntax generation file hashes: {error}"))?;
        let artifact_id = self
            .conn
            .query_row(
                "SELECT artifact_id
                 FROM syntax_query_artifact_ref
                 WHERE generation_id = ?1
                 ORDER BY artifact_ordinal ASC
                 LIMIT 1",
                params![generation_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("failed to read syntax query artifact ref: {error}"))?
            .map(CacheArtifactId::from);
        let mut statement = self
            .conn
            .prepare_cached(
                "SELECT m.path,
                        m.start_line,
                        m.end_line,
                        c.path,
                        c.start_line,
                        c.end_line,
                        c.capture_name,
                        c.capture_node_type,
                        c.item_node_type,
                        c.field,
                        c.capture_text
                 FROM syntax_query_capture c
                 JOIN syntax_query_match m
                   ON m.generation_id = c.generation_id
                 AND m.match_ordinal = c.match_ordinal
                 WHERE c.generation_id = ?1
                   AND c.capture_node_type IS NOT NULL
                   AND c.item_node_type IS NOT NULL
                 ORDER BY c.match_ordinal ASC, c.capture_ordinal ASC",
            )
            .map_err(|error| format!("failed to prepare syntax capture replay query: {error}"))?;
        let row_iter = statement
            .query_map(params![generation_id.as_str()], |row| {
                let match_path = row.get::<_, String>(0)?;
                let match_start_line = row.get::<_, i64>(1)?;
                let match_end_line = row.get::<_, i64>(2)?;
                let capture_path = row.get::<_, String>(3)?;
                let capture_start_line = row.get::<_, i64>(4)?;
                let capture_end_line = row.get::<_, i64>(5)?;
                let capture_name = row.get::<_, String>(6)?;
                let capture_node_type = row.get::<_, String>(7)?;
                let item_node_type = row.get::<_, String>(8)?;
                let field = row.get::<_, Option<String>>(9)?;
                let text = row.get::<_, String>(10)?;
                Ok(ClientDbSyntaxCaptureReplay {
                    match_locator: compact_source_locator(
                        &match_path,
                        match_start_line,
                        match_end_line,
                    ),
                    capture_locator: compact_source_locator(
                        &capture_path,
                        capture_start_line,
                        capture_end_line,
                    ),
                    capture_name,
                    capture_node_type: ClientDbSyntaxNodeType::from(capture_node_type),
                    item_node_type: ClientDbSyntaxNodeType::from(item_node_type),
                    field,
                    text,
                })
            })
            .map_err(|error| format!("failed to read syntax capture replay rows: {error}"))?;
        let mut rows = Vec::new();
        for row in row_iter {
            rows.push(
                row.map_err(|error| format!("failed to decode syntax capture row: {error}"))?,
            );
        }
        Ok(Some(ClientDbSyntaxQueryReplay {
            generation_id: CacheGenerationId::from(generation_id),
            language_id: LanguageId::from(language_id),
            grammar_id,
            grammar_profile_version,
            input_form,
            input_kind: ClientDbSyntaxQueryInputKind::from_wire(&input_kind),
            compiled_source,
            captures,
            query_ast_fingerprint,
            artifact_id,
            packet_bytes: packet_bytes.map(|value| value.max(0) as u64),
            file_hashes,
            rows,
        }))
    }

    fn lookup_artifact_events_for(
        &self,
        since_timestamp_ms: Option<i64>,
        limit: u32,
    ) -> Result<Vec<ClientDbArtifactEvent>, String> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT artifact_path,
                        event_ordinal,
                        timestamp_ms,
                        kind,
                        language,
                        method,
                        target,
                        query,
                        project_root,
                        project_root_arg,
                        bytes
                 FROM artifact_event
                 WHERE (?1 IS NULL OR timestamp_ms >= ?1)
                 ORDER BY timestamp_ms ASC, artifact_path ASC, event_ordinal ASC
                 LIMIT ?2",
            )
            .map_err(|error| format!("failed to prepare artifact event query: {error}"))?;
        let row_iter = statement
            .query_map(
                params![since_timestamp_ms, i64::from(limit)],
                artifact_event_from_row,
            )
            .map_err(|error| format!("failed to read artifact events: {error}"))?;
        let mut events = Vec::new();
        for row in row_iter {
            events.push(row.map_err(|error| format!("failed to decode artifact event: {error}"))?);
        }
        Ok(events)
    }

    fn open_read_only(db_path: &Path) -> Result<Self, String> {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|error| {
            format!(
                "failed to open agent semantic client db at {}: {error}",
                db_path.display()
            )
        })?;
        configure_readable_connection(&conn, db_path)?;
        Ok(Self {
            conn,
            db_path: db_path.to_path_buf(),
        })
    }

    fn migrate(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS schema_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS cache_generations (
                generation_id TEXT PRIMARY KEY,
                language_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                provider_version TEXT,
                export_method TEXT,
                project_root TEXT NOT NULL,
                package_root TEXT,
                schema_ids_json TEXT NOT NULL,
                cache_status TEXT NOT NULL,
                raw_source_stored INTEGER NOT NULL DEFAULT 0 CHECK(raw_source_stored IN (0, 1)),
                request_fingerprint TEXT,
                artifact_ids_json TEXT NOT NULL DEFAULT '[]',
                file_hashes_json TEXT NOT NULL DEFAULT '[]',
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );
            CREATE INDEX IF NOT EXISTS cache_generations_provider_idx
              ON cache_generations(language_id, provider_id);
            CREATE INDEX IF NOT EXISTS cache_generations_project_idx
              ON cache_generations(project_root, package_root);
            CREATE TABLE IF NOT EXISTS syntax_query_generation (
                generation_id TEXT PRIMARY KEY
                    REFERENCES cache_generations(generation_id) ON DELETE CASCADE,
                language_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                project_root TEXT NOT NULL,
                request_fingerprint TEXT NOT NULL,
                query_ast_fingerprint TEXT,
                grammar_id TEXT NOT NULL,
                grammar_profile_version TEXT NOT NULL,
                input_form TEXT NOT NULL,
                input_kind TEXT NOT NULL,
                match_count INTEGER NOT NULL DEFAULT 0,
                truncated INTEGER NOT NULL DEFAULT 0 CHECK(truncated IN (0, 1)),
                packet_bytes INTEGER,
                raw_source_stored INTEGER NOT NULL DEFAULT 0 CHECK(raw_source_stored IN (0, 1)),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );
            CREATE UNIQUE INDEX IF NOT EXISTS syntax_query_generation_request_idx
              ON syntax_query_generation(
                language_id,
                provider_id,
                project_root,
                request_fingerprint
              );
            CREATE TABLE IF NOT EXISTS syntax_query_pattern (
                generation_id TEXT NOT NULL
                    REFERENCES syntax_query_generation(generation_id) ON DELETE CASCADE,
                pattern_index INTEGER NOT NULL,
                query_input TEXT NOT NULL,
                compiled_source TEXT NOT NULL,
                selector TEXT,
                captures_json TEXT NOT NULL DEFAULT '[]',
                PRIMARY KEY (generation_id, pattern_index)
            );
            CREATE TABLE IF NOT EXISTS syntax_query_match (
                generation_id TEXT NOT NULL
                    REFERENCES syntax_query_generation(generation_id) ON DELETE CASCADE,
                match_ordinal INTEGER NOT NULL,
                match_id TEXT,
                path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                native_fact_refs_json TEXT NOT NULL DEFAULT '[]',
                PRIMARY KEY (generation_id, match_ordinal)
            );
            CREATE TABLE IF NOT EXISTS syntax_query_capture (
                generation_id TEXT NOT NULL
                    REFERENCES syntax_query_generation(generation_id) ON DELETE CASCADE,
                match_ordinal INTEGER NOT NULL,
                capture_ordinal INTEGER NOT NULL,
                capture_id TEXT,
                capture_name TEXT NOT NULL,
                node_type TEXT,
                capture_node_type TEXT,
                item_node_type TEXT,
                field TEXT,
                capture_text TEXT NOT NULL,
                path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                PRIMARY KEY (generation_id, match_ordinal, capture_ordinal)
            );
            CREATE TABLE IF NOT EXISTS syntax_query_capture_native_fact_ref (
                generation_id TEXT NOT NULL
                    REFERENCES syntax_query_generation(generation_id) ON DELETE CASCADE,
                match_ordinal INTEGER NOT NULL,
                capture_ordinal INTEGER NOT NULL,
                ref_ordinal INTEGER NOT NULL,
                native_fact_ref TEXT NOT NULL,
                PRIMARY KEY (
                    generation_id,
                    match_ordinal,
                    capture_ordinal,
                    ref_ordinal
                )
            );
            CREATE TABLE IF NOT EXISTS syntax_query_artifact_ref (
                generation_id TEXT NOT NULL
                    REFERENCES syntax_query_generation(generation_id) ON DELETE CASCADE,
                artifact_ordinal INTEGER NOT NULL,
                artifact_id TEXT NOT NULL,
                PRIMARY KEY (generation_id, artifact_ordinal)
            );
            CREATE TABLE IF NOT EXISTS structural_index_generation (
                generation_id TEXT PRIMARY KEY
                    REFERENCES cache_generations(generation_id) ON DELETE CASCADE,
                language_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                provider_version TEXT,
                export_method TEXT,
                project_root TEXT NOT NULL,
                package_root TEXT,
                schema_id TEXT NOT NULL,
                schema_version TEXT NOT NULL,
                source_artifact_id TEXT,
                file_hashes_json TEXT NOT NULL DEFAULT '[]',
                raw_source_stored INTEGER NOT NULL DEFAULT 0 CHECK(raw_source_stored IN (0, 1)),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            );
            CREATE INDEX IF NOT EXISTS structural_index_generation_project_idx
              ON structural_index_generation(language_id, provider_id, project_root);
            CREATE TABLE IF NOT EXISTS structural_index_owner (
                generation_id TEXT NOT NULL
                    REFERENCES structural_index_generation(generation_id) ON DELETE CASCADE,
                owner_ordinal INTEGER NOT NULL,
                owner_path TEXT NOT NULL,
                owner_kind TEXT NOT NULL,
                source_authority TEXT NOT NULL,
                start_line INTEGER,
                end_line INTEGER,
                query_keys_json TEXT NOT NULL DEFAULT '[]',
                search_text TEXT NOT NULL,
                PRIMARY KEY (generation_id, owner_ordinal)
            );
            CREATE INDEX IF NOT EXISTS structural_index_owner_path_idx
              ON structural_index_owner(owner_path);
            CREATE TABLE IF NOT EXISTS structural_index_symbol (
                generation_id TEXT NOT NULL
                    REFERENCES structural_index_generation(generation_id) ON DELETE CASCADE,
                symbol_ordinal INTEGER NOT NULL,
                owner_path TEXT NOT NULL,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                visibility TEXT,
                source_locator TEXT,
                query_keys_json TEXT NOT NULL DEFAULT '[]',
                search_text TEXT NOT NULL,
                PRIMARY KEY (generation_id, symbol_ordinal)
            );
            CREATE INDEX IF NOT EXISTS structural_index_symbol_name_idx
              ON structural_index_symbol(name, kind);
            CREATE TABLE IF NOT EXISTS structural_index_dependency_usage (
                generation_id TEXT NOT NULL
                    REFERENCES structural_index_generation(generation_id) ON DELETE CASCADE,
                usage_ordinal INTEGER NOT NULL,
                owner_path TEXT NOT NULL,
                package_name TEXT NOT NULL,
                package_version TEXT,
                api_name TEXT,
                import_path TEXT,
                manifest_path TEXT,
                lockfile_hash TEXT,
                source TEXT NOT NULL,
                source_locator TEXT,
                query_keys_json TEXT NOT NULL DEFAULT '[]',
                search_text TEXT NOT NULL,
                PRIMARY KEY (generation_id, usage_ordinal)
            );
            CREATE INDEX IF NOT EXISTS structural_index_dependency_pkg_idx
              ON structural_index_dependency_usage(package_name, api_name);
            CREATE TABLE IF NOT EXISTS artifact_event (
                artifact_path TEXT NOT NULL,
                event_ordinal INTEGER NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                kind TEXT NOT NULL,
                language TEXT NOT NULL,
                method TEXT NOT NULL,
                target TEXT NOT NULL,
                query TEXT NOT NULL,
                project_root TEXT NOT NULL,
                project_root_arg TEXT NOT NULL,
                bytes INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                PRIMARY KEY (artifact_path, event_ordinal)
            );
            CREATE INDEX IF NOT EXISTS artifact_event_timestamp_idx
              ON artifact_event(timestamp_ms, artifact_path, event_ordinal);
            CREATE INDEX IF NOT EXISTS artifact_event_project_idx
              ON artifact_event(project_root, timestamp_ms);
            CREATE TABLE IF NOT EXISTS provider_command_selection (
                project_root TEXT NOT NULL,
                context_fingerprint TEXT NOT NULL,
                manifest_id TEXT NOT NULL,
                manifest_digest TEXT NOT NULL,
                language_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                binary TEXT NOT NULL,
                execution TEXT NOT NULL,
                provider_command_prefix_json TEXT NOT NULL,
                executable_path TEXT,
                executable_len INTEGER,
                executable_mtime_ms INTEGER,
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                PRIMARY KEY (
                    project_root,
                    context_fingerprint,
                    language_id,
                    provider_id,
                    manifest_id
                )
            );
            CREATE INDEX IF NOT EXISTS provider_command_selection_project_idx
              ON provider_command_selection(project_root, context_fingerprint);
            ",
            )
            .map_err(|error| {
                format!(
                    "failed to migrate agent semantic client db at {}: {error}",
                    self.db_path.display()
                )
            })?;
        self.conn
            .execute(
                "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('schemaVersion', ?1)",
                params![AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION.to_string()],
            )
            .map_err(|error| {
                format!(
                    "failed to write agent semantic client db schema version at {}: {error}",
                    self.db_path.display()
                )
            })?;

        let has_request_fingerprint = {
            let mut statement = self
                .conn
                .prepare("PRAGMA table_info(cache_generations)")
                .map_err(|error| {
                    format!(
                        "failed to inspect agent semantic client db at {}: {error}",
                        self.db_path.display()
                    )
                })?;
            let columns = statement
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|error| format!("failed to read client db columns: {error}"))?;
            let mut found = false;
            for column in columns {
                if column.map_err(|error| format!("failed to read client db column: {error}"))?
                    == "request_fingerprint"
                {
                    found = true;
                    break;
                }
            }
            found
        };

        if !has_request_fingerprint {
            self.conn
                .execute(
                    "ALTER TABLE cache_generations ADD COLUMN request_fingerprint TEXT",
                    [],
                )
                .map_err(|error| format!("failed to add request fingerprint column: {error}"))?;
        }

        if !self.table_has_column("syntax_query_generation", "query_ast_fingerprint")? {
            self.conn
                .execute(
                    "ALTER TABLE syntax_query_generation ADD COLUMN query_ast_fingerprint TEXT",
                    [],
                )
                .map_err(|error| {
                    format!("failed to add syntax query AST fingerprint column: {error}")
                })?;
        }

        if !self.table_has_column("syntax_query_capture", "field")? {
            self.conn
                .execute("ALTER TABLE syntax_query_capture ADD COLUMN field TEXT", [])
                .map_err(|error| format!("failed to add syntax capture field column: {error}"))?;
        }
        if !self.table_has_column("syntax_query_capture", "capture_node_type")? {
            self.conn
                .execute(
                    "ALTER TABLE syntax_query_capture ADD COLUMN capture_node_type TEXT",
                    [],
                )
                .map_err(|error| {
                    format!("failed to add syntax capture node type column: {error}")
                })?;
        }
        if !self.table_has_column("syntax_query_capture", "item_node_type")? {
            self.conn
                .execute(
                    "ALTER TABLE syntax_query_capture ADD COLUMN item_node_type TEXT",
                    [],
                )
                .map_err(|error| format!("failed to add syntax item node type column: {error}"))?;
        }
        if !self.table_has_column("structural_index_dependency_usage", "source_locator")? {
            self.conn
                .execute(
                    "ALTER TABLE structural_index_dependency_usage ADD COLUMN source_locator TEXT",
                    [],
                )
                .map_err(|error| {
                    format!("failed to add structural dependency source locator column: {error}")
                })?;
        }

        self.flush_stale_syntax_query_rows()?;

        Ok(())
    }

    fn flush_stale_syntax_query_rows(&self) -> Result<(), String> {
        let current_row_abi = self
            .conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = ?1",
                params![SYNTAX_QUERY_ROW_ABI_META_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| {
                format!(
                    "failed to read syntax query row ABI version at {}: {error}",
                    self.db_path.display()
                )
            })?;
        if current_row_abi.as_deref() != Some(SYNTAX_QUERY_ROW_ABI_VERSION) {
            self.conn
                .execute("DELETE FROM syntax_query_generation", [])
                .map_err(|error| format!("failed to flush syntax query row cache: {error}"))?;
            self.conn
                .execute(
                    "INSERT OR REPLACE INTO schema_meta (key, value) VALUES (?1, ?2)",
                    params![SYNTAX_QUERY_ROW_ABI_META_KEY, SYNTAX_QUERY_ROW_ABI_VERSION],
                )
                .map_err(|error| {
                    format!("failed to write syntax query row ABI version: {error}")
                })?;
        }
        Ok(())
    }

    fn table_has_column(&self, table: &str, column: &str) -> Result<bool, String> {
        let mut statement = self
            .conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|error| {
                format!(
                    "failed to inspect agent semantic client db table {table} at {}: {error}",
                    self.db_path.display()
                )
            })?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|error| format!("failed to read client db columns: {error}"))?;
        for candidate in columns {
            if candidate.map_err(|error| format!("failed to read client db column: {error}"))?
                == column
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn artifact_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClientDbArtifactEvent> {
    let event_ordinal = row.get::<_, i64>(1)?.max(0).min(i64::from(u32::MAX)) as u32;
    let bytes = row.get::<_, i64>(10)?.max(0) as u64;
    Ok(ClientDbArtifactEvent {
        artifact_path: row.get(0)?,
        event_ordinal,
        timestamp_ms: row.get(2)?,
        kind: row.get(3)?,
        language: row.get(4)?,
        method: row.get(5)?,
        target: row.get(6)?,
        query: row.get(7)?,
        project_root: row.get(8)?,
        project_root_arg: row.get(9)?,
        bytes,
    })
}

fn cache_generation_hit_from_row(row: CacheGenerationRow) -> Result<ClientDbGenerationHit, String> {
    let (
        language_id,
        provider_id,
        project_root,
        export_method,
        schema_ids_json,
        request_fingerprint,
        artifact_ids_json,
        file_hashes_json,
    ) = row;
    let schema_ids = serde_json::from_str(&schema_ids_json)
        .map_err(|error| format!("failed to parse client db schema ids: {error}"))?;
    let artifact_ids = serde_json::from_str(&artifact_ids_json)
        .map_err(|error| format!("failed to parse client db artifact ids: {error}"))?;
    let file_hashes = serde_json::from_str(&file_hashes_json)
        .map_err(|error| format!("failed to parse client db file hashes: {error}"))?;
    Ok(ClientDbGenerationHit {
        language_id: LanguageId::from(language_id),
        provider_id: ProviderId::from(provider_id),
        project_root: PathBuf::from(project_root),
        export_method: CacheExportMethod::from(export_method),
        schema_ids,
        request_fingerprint,
        file_hashes,
        artifact_ids,
    })
}

pub(crate) fn normalized_project_root(project_root: &Path) -> String {
    project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf())
        .display()
        .to_string()
}

/// Aggregate counts read from the local SQLite client DB.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSummary {
    pub generation_count: u32,
    pub syntax_row_generation_count: u32,
    pub syntax_row_match_count: u32,
    pub syntax_row_capture_count: u32,
    pub structural_index_generation_count: u32,
    pub structural_index_owner_count: u32,
    pub structural_index_symbol_count: u32,
    pub structural_index_dependency_usage_count: u32,
    pub artifact_event_count: u32,
    pub raw_source_stored: bool,
}

#[derive(Default)]
struct ClientDbTableCounts {
    syntax_row_generation_count: u32,
    syntax_row_match_count: u32,
    syntax_row_capture_count: u32,
    structural_index_generation_count: u32,
    structural_index_owner_count: u32,
    structural_index_symbol_count: u32,
    structural_index_dependency_usage_count: u32,
    artifact_event_count: u32,
}

impl ClientDbTableCounts {
    fn set(&mut self, table: &str, count: i64) {
        let count = count.max(0).min(i64::from(u32::MAX)) as u32;
        match table {
            "syntax_query_generation" => self.syntax_row_generation_count = count,
            "syntax_query_match" => self.syntax_row_match_count = count,
            "syntax_query_capture" => self.syntax_row_capture_count = count,
            "structural_index_generation" => self.structural_index_generation_count = count,
            "structural_index_owner" => self.structural_index_owner_count = count,
            "structural_index_symbol" => self.structural_index_symbol_count = count,
            "structural_index_dependency_usage" => {
                self.structural_index_dependency_usage_count = count;
            }
            "artifact_event" => self.artifact_event_count = count,
            _ => {}
        }
    }
}
