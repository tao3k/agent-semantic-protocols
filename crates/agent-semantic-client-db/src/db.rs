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
/// Current SQLite schema version for the local agent semantic client DB.
pub const AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION: i64 = 4;

/// Read-only diagnostic summary for a local SQLite client DB path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbReport {
    pub db_path: PathBuf,
    pub status: ClientDbStatus,
    pub generation_count: u32,
    pub syntax_row_generation_count: u32,
    pub syntax_row_match_count: u32,
    pub syntax_row_capture_count: u32,
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
    conn: Connection,
    db_path: PathBuf,
}

impl ClientDb {
    /// Return the default SQLite DB path under a client cache root.
    #[must_use]
    pub fn default_path(cache_root: impl AsRef<Path>) -> PathBuf {
        cache_root.as_ref().join(AGENT_SEMANTIC_CLIENT_DB_FILE)
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
                raw_source_stored: false,
                runtime_pragmas: None,
                reason: None,
            };
        }

        match Self::open_read_only(&db_path).and_then(|db| {
            let summary = db.summary()?;
            let runtime_pragmas = db.runtime_pragmas()?;
            Ok((summary, runtime_pragmas))
        }) {
            Ok((summary, runtime_pragmas)) => ClientDbReport {
                db_path,
                status: ClientDbStatus::Present,
                generation_count: summary.generation_count,
                syntax_row_generation_count: summary.syntax_row_generation_count,
                syntax_row_match_count: summary.syntax_row_match_count,
                syntax_row_capture_count: summary.syntax_row_capture_count,
                raw_source_stored: summary.raw_source_stored,
                runtime_pragmas: Some(runtime_pragmas),
                reason: None,
            },
            Err(error) => ClientDbReport {
                db_path,
                status: ClientDbStatus::Invalid,
                generation_count: 0,
                syntax_row_generation_count: 0,
                syntax_row_match_count: 0,
                syntax_row_capture_count: 0,
                raw_source_stored: false,
                runtime_pragmas: None,
                reason: Some(error),
            },
        }
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
        Ok(ClientDbSummary {
            generation_count: generation_count.max(0).min(i64::from(u32::MAX)) as u32,
            syntax_row_generation_count: self.count_table_rows("syntax_query_generation")?,
            syntax_row_match_count: self.count_table_rows("syntax_query_match")?,
            syntax_row_capture_count: self.count_table_rows("syntax_query_capture")?,
            raw_source_stored: raw_source_stored != 0,
        })
    }

    /// Return runtime SQLite pragmas observed on this DB connection.
    pub fn runtime_pragmas(&self) -> Result<ClientDbRuntimePragmas, String> {
        read_runtime_pragmas(&self.conn, &self.db_path)
    }

    fn count_table_rows(&self, table: &str) -> Result<u32, String> {
        let sql = match table {
            "syntax_query_generation" => "SELECT COUNT(*) FROM syntax_query_generation",
            "syntax_query_match" => "SELECT COUNT(*) FROM syntax_query_match",
            "syntax_query_capture" => "SELECT COUNT(*) FROM syntax_query_capture",
            _ => return Err(format!("unsupported client db count table `{table}`")),
        };
        let count: i64 = self
            .conn
            .query_row(sql, [], |row| row.get(0))
            .map_err(|error| format!("failed to count client db {table} rows: {error}"))?;
        Ok(count.max(0).min(i64::from(u32::MAX)) as u32)
    }

    fn lookup_generation_for(
        &self,
        language_id: &LanguageId,
        provider_id: &ProviderId,
        project_root: &Path,
        export_method: &CacheExportMethod,
        request_fingerprint: Option<&str>,
    ) -> Result<Option<ClientDbGenerationHit>, String> {
        let project_root = normalized_project_root(project_root);
        let row: Option<CacheGenerationRow> = self
            .conn
            .query_row(
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
             LIMIT 1",
                params![
                    language_id.as_str(),
                    provider_id.as_str(),
                    project_root,
                    export_method.as_str(),
                    request_fingerprint,
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
            .optional()
            .map_err(|error| format!("failed to read client db cache generation: {error}"))?;
        let Some((
            language_id,
            provider_id,
            project_root,
            export_method,
            schema_ids_json,
            request_fingerprint,
            artifact_ids_json,
            file_hashes_json,
        )) = row
        else {
            return Ok(None);
        };
        let schema_ids = serde_json::from_str(&schema_ids_json)
            .map_err(|error| format!("failed to parse client db schema ids: {error}"))?;
        let artifact_ids = serde_json::from_str(&artifact_ids_json)
            .map_err(|error| format!("failed to parse client db artifact ids: {error}"))?;
        let file_hashes = serde_json::from_str(&file_hashes_json)
            .map_err(|error| format!("failed to parse client db file hashes: {error}"))?;
        Ok(Some(ClientDbGenerationHit {
            language_id: LanguageId::from(language_id),
            provider_id: ProviderId::from(provider_id),
            project_root: PathBuf::from(project_root),
            export_method: CacheExportMethod::from(export_method),
            schema_ids,
            request_fingerprint,
            file_hashes,
            artifact_ids,
        }))
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
            .prepare(
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
    pub raw_source_stored: bool,
}
