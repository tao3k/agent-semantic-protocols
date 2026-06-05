//! SQLite storage adapter for local `agent-semantic-client` cache state.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheGenerationId, ClientCacheGeneration,
    ClientCacheManifest, ClientDbStatus, LanguageId, ProviderId,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde_json::Value;

/// File name used for the local SQLite client cache.
pub const AGENT_SEMANTIC_CLIENT_DB_FILE: &str = "client.sqlite3";
/// Current SQLite schema version for the local agent semantic client DB.
pub const AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION: i64 = 3;

const SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-tree-sitter-query";

/// Read-only diagnostic summary for a local SQLite client DB path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbReport {
    pub db_path: PathBuf,
    pub status: ClientDbStatus,
    pub generation_count: u32,
    pub raw_source_stored: bool,
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
    pub artifact_ids: Vec<CacheArtifactId>,
}

/// Named lookup request for normalized syntax query replay rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSyntaxQueryLookup {
    pub db_path: PathBuf,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub project_root: PathBuf,
    pub request_fingerprint: String,
}

/// Normalized syntax query rows that can render compact locator/capture output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSyntaxQueryReplay {
    pub generation_id: CacheGenerationId,
    pub grammar_id: String,
    pub grammar_profile_version: String,
    pub input_form: String,
    pub input_kind: ClientDbSyntaxQueryInputKind,
    pub captures: Vec<String>,
    pub artifact_id: Option<CacheArtifactId>,
    pub packet_bytes: Option<u64>,
    pub rows: Vec<ClientDbSyntaxCaptureReplay>,
}

/// Tree-sitter query input family represented by normalized syntax query rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientDbSyntaxQueryInputKind {
    Inline,
    Catalog,
}

impl ClientDbSyntaxQueryInputKind {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Catalog => "catalog",
        }
    }

    fn from_wire(value: &str) -> Self {
        if value == "catalog" {
            Self::Catalog
        } else {
            Self::Inline
        }
    }
}

/// One replayable syntax capture row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSyntaxCaptureReplay {
    pub locator: String,
    pub text: String,
}

type CacheGenerationRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
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
                raw_source_stored: false,
                reason: None,
            };
        }

        match Self::open_read_only(&db_path).and_then(|db| db.summary()) {
            Ok(summary) => ClientDbReport {
                db_path,
                status: ClientDbStatus::Present,
                generation_count: summary.generation_count,
                raw_source_stored: summary.raw_source_stored,
                reason: None,
            },
            Err(error) => ClientDbReport {
                db_path,
                status: ClientDbStatus::Invalid,
                generation_count: 0,
                raw_source_stored: false,
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
                "INSERT OR REPLACE INTO cache_generations (
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
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11, ?12)",
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
            raw_source_stored: raw_source_stored != 0,
        })
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
                    artifact_ids_json
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
        )) = row
        else {
            return Ok(None);
        };
        let schema_ids = serde_json::from_str(&schema_ids_json)
            .map_err(|error| format!("failed to parse client db schema ids: {error}"))?;
        let artifact_ids = serde_json::from_str(&artifact_ids_json)
            .map_err(|error| format!("failed to parse client db artifact ids: {error}"))?;
        Ok(Some(ClientDbGenerationHit {
            language_id: LanguageId::from(language_id),
            provider_id: ProviderId::from(provider_id),
            project_root: PathBuf::from(project_root),
            export_method: CacheExportMethod::from(export_method),
            schema_ids,
            request_fingerprint,
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
                    g.grammar_id,
                    g.grammar_profile_version,
                    g.input_form,
                    g.input_kind,
                    p.captures_json,
                    g.packet_bytes
             FROM syntax_query_generation g
             JOIN syntax_query_pattern p ON p.generation_id = g.generation_id
             WHERE g.language_id = ?1
               AND g.provider_id = ?2
               AND g.project_root = ?3
               AND g.request_fingerprint = ?4
               AND g.raw_source_stored = 0
             ORDER BY g.updated_at DESC
             LIMIT 1",
                params![
                    lookup.language_id.as_str(),
                    lookup.provider_id.as_str(),
                    project_root,
                    lookup.request_fingerprint.as_str(),
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("failed to read syntax query replay generation: {error}"))?;
        let Some((
            generation_id,
            grammar_id,
            grammar_profile_version,
            input_form,
            input_kind,
            captures_json,
            packet_bytes,
        )) = row
        else {
            return Ok(None);
        };
        let captures = serde_json::from_str(&captures_json)
            .map_err(|error| format!("failed to parse syntax capture names: {error}"))?;
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
                "SELECT path, start_line, end_line, capture_text
                 FROM syntax_query_capture
                 WHERE generation_id = ?1
                 ORDER BY match_ordinal ASC, capture_ordinal ASC",
            )
            .map_err(|error| format!("failed to prepare syntax capture replay query: {error}"))?;
        let row_iter = statement
            .query_map(params![generation_id.as_str()], |row| {
                let path = row.get::<_, String>(0)?;
                let start_line = row.get::<_, i64>(1)?;
                let end_line = row.get::<_, i64>(2)?;
                let text = row.get::<_, String>(3)?;
                Ok(ClientDbSyntaxCaptureReplay {
                    locator: compact_source_locator(&path, start_line, end_line),
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
            grammar_id,
            grammar_profile_version,
            input_form,
            input_kind: ClientDbSyntaxQueryInputKind::from_wire(&input_kind),
            captures,
            artifact_id,
            packet_bytes: packet_bytes.map(|value| value.max(0) as u64),
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

        Ok(())
    }
}

fn normalized_project_root(project_root: &Path) -> String {
    project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf())
        .display()
        .to_string()
}

struct ParsedSyntaxQueryPacketImport<'a> {
    generation: &'a ClientCacheGeneration,
    request_fingerprint: &'a str,
    project_root: String,
    grammar_id: String,
    grammar_profile_version: String,
    input_form: String,
    input_kind: &'static str,
    query_input: String,
    compiled_source: String,
    selector: Option<String>,
    captures_json: String,
    matches: Vec<Value>,
    truncated: bool,
    packet_bytes: usize,
    artifact_ids: &'a [CacheArtifactId],
}

fn parse_syntax_query_packet_import<'a>(
    generation: &'a ClientCacheGeneration,
    packet_bytes: &'a [u8],
) -> Result<ParsedSyntaxQueryPacketImport<'a>, String> {
    if generation.raw_source_stored {
        return Err("syntax query rows refuse rawSourceStored=true generation".to_string());
    }
    let request_fingerprint = generation
        .request_fingerprint
        .as_deref()
        .ok_or_else(|| "syntax query rows require requestFingerprint".to_string())?;
    let packet: Value = serde_json::from_slice(packet_bytes)
        .map_err(|error| format!("failed to parse semantic tree-sitter query packet: {error}"))?;
    validate_syntax_query_packet_for_rows(&packet)?;

    let query = packet
        .get("query")
        .ok_or_else(|| "syntax query packet is missing query".to_string())?;
    let captures = query
        .get("fields")
        .and_then(|fields| fields.get("captures"))
        .and_then(Value::as_array)
        .map(|captures| {
            captures
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let matches = packet
        .get("matches")
        .and_then(Value::as_array)
        .ok_or_else(|| "syntax query packet is missing matches".to_string())?;
    let query_input = optional_string_field(query, "input").unwrap_or("");
    Ok(ParsedSyntaxQueryPacketImport {
        generation,
        request_fingerprint,
        project_root: normalized_project_root(Path::new(&generation.project_root)),
        grammar_id: string_field(&packet, "grammarId")?.to_string(),
        grammar_profile_version: string_field(&packet, "grammarProfileVersion")?.to_string(),
        input_form: string_field(query, "inputForm")?.to_string(),
        input_kind: if query.get("catalogId").is_some() {
            "catalog"
        } else {
            "inline"
        },
        query_input: query_input.to_string(),
        compiled_source: optional_string_field(query, "compiledSource")
            .unwrap_or(query_input)
            .to_string(),
        selector: query
            .get("fields")
            .and_then(|fields| optional_string_field(fields, "selector"))
            .map(str::to_string),
        captures_json: serde_json::to_string(&captures)
            .map_err(|error| format!("failed to serialize syntax captures: {error}"))?,
        matches: matches.clone(),
        truncated: packet
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        packet_bytes: packet_bytes.len(),
        artifact_ids: generation.artifact_ids.as_deref().unwrap_or(&[]),
    })
}

fn validate_syntax_query_packet_for_rows(packet: &Value) -> Result<(), String> {
    if string_field(packet, "schemaId")? != SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID {
        return Err("syntax query rows require semantic-tree-sitter-query packet".to_string());
    }
    if packet
        .pointer("/cache/rawSourceStored")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err("syntax query rows refuse packet rawSourceStored=true".to_string());
    }
    if packet
        .pointer("/query/fields/codeOutput")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err("syntax query rows do not store --code packet output".to_string());
    }
    Ok(())
}

fn write_syntax_query_import_rows(
    tx: &rusqlite::Transaction<'_>,
    parsed: &ParsedSyntaxQueryPacketImport<'_>,
) -> Result<(), String> {
    let generation_id = parsed.generation.generation_id.as_str();
    delete_syntax_query_rows(tx, generation_id)?;
    write_syntax_query_generation_row(tx, parsed)?;
    write_syntax_query_pattern_row(tx, parsed)?;
    write_syntax_query_artifact_ref_rows(tx, parsed)?;
    for (match_index, item) in parsed.matches.iter().enumerate() {
        import_syntax_match_rows(tx, generation_id, match_index, item)?;
    }
    Ok(())
}

fn write_syntax_query_generation_row(
    tx: &rusqlite::Transaction<'_>,
    parsed: &ParsedSyntaxQueryPacketImport<'_>,
) -> Result<(), String> {
    tx.execute(
        "INSERT OR REPLACE INTO syntax_query_generation (
            generation_id,
            language_id,
            provider_id,
            project_root,
            request_fingerprint,
            grammar_id,
            grammar_profile_version,
            input_form,
            input_kind,
            match_count,
            truncated,
            packet_bytes,
            raw_source_stored
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0)",
        params![
            parsed.generation.generation_id.as_str(),
            parsed.generation.language_id.as_str(),
            parsed.generation.provider_id.as_str(),
            parsed.project_root.as_str(),
            parsed.request_fingerprint,
            parsed.grammar_id.as_str(),
            parsed.grammar_profile_version.as_str(),
            parsed.input_form.as_str(),
            parsed.input_kind,
            parsed.matches.len().min(i64::MAX as usize) as i64,
            parsed.truncated as i64,
            parsed.packet_bytes.min(i64::MAX as usize) as i64,
        ],
    )
    .map_err(|error| format!("failed to write syntax query generation rows: {error}"))?;
    Ok(())
}

fn write_syntax_query_pattern_row(
    tx: &rusqlite::Transaction<'_>,
    parsed: &ParsedSyntaxQueryPacketImport<'_>,
) -> Result<(), String> {
    tx.execute(
        "INSERT OR REPLACE INTO syntax_query_pattern (
            generation_id,
            pattern_index,
            query_input,
            compiled_source,
            selector,
            captures_json
        ) VALUES (?1, 0, ?2, ?3, ?4, ?5)",
        params![
            parsed.generation.generation_id.as_str(),
            parsed.query_input.as_str(),
            parsed.compiled_source.as_str(),
            parsed.selector.as_deref(),
            parsed.captures_json.as_str(),
        ],
    )
    .map_err(|error| format!("failed to write syntax query pattern row: {error}"))?;
    Ok(())
}

fn write_syntax_query_artifact_ref_rows(
    tx: &rusqlite::Transaction<'_>,
    parsed: &ParsedSyntaxQueryPacketImport<'_>,
) -> Result<(), String> {
    for (artifact_ordinal, artifact_id) in parsed.artifact_ids.iter().enumerate() {
        tx.execute(
            "INSERT OR REPLACE INTO syntax_query_artifact_ref (
                generation_id,
                artifact_ordinal,
                artifact_id
            ) VALUES (?1, ?2, ?3)",
            params![
                parsed.generation.generation_id.as_str(),
                artifact_ordinal.min(i64::MAX as usize) as i64,
                artifact_id.as_str(),
            ],
        )
        .map_err(|error| format!("failed to write syntax artifact ref row: {error}"))?;
    }
    Ok(())
}

fn delete_syntax_query_rows(
    tx: &rusqlite::Transaction<'_>,
    generation_id: &str,
) -> Result<(), String> {
    for table in [
        "syntax_query_capture_native_fact_ref",
        "syntax_query_capture",
        "syntax_query_match",
        "syntax_query_artifact_ref",
        "syntax_query_pattern",
        "syntax_query_generation",
    ] {
        tx.execute(
            &format!("DELETE FROM {table} WHERE generation_id = ?1"),
            params![generation_id],
        )
        .map_err(|error| format!("failed to clear {table} rows: {error}"))?;
    }
    Ok(())
}

fn import_syntax_match_rows(
    tx: &rusqlite::Transaction<'_>,
    generation_id: &str,
    match_index: usize,
    item: &Value,
) -> Result<(), String> {
    let captures = item
        .get("captures")
        .and_then(Value::as_array)
        .ok_or_else(|| "syntax match is missing captures".to_string())?;
    let first_capture_range = captures
        .iter()
        .find_map(|capture| capture.get("range").and_then(parse_syntax_range));
    let match_range = item
        .get("range")
        .and_then(parse_syntax_range)
        .or(first_capture_range)
        .ok_or_else(|| "syntax match is missing a replayable range".to_string())?;
    let match_ordinal = match_index.min(i64::MAX as usize) as i64;
    let native_fact_refs = string_array_field(item, "nativeFactRefs");
    let native_fact_refs_json = serde_json::to_string(&native_fact_refs)
        .map_err(|error| format!("failed to serialize syntax match native refs: {error}"))?;
    tx.execute(
        "INSERT OR REPLACE INTO syntax_query_match (
            generation_id,
            match_ordinal,
            match_id,
            path,
            start_line,
            end_line,
            native_fact_refs_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            generation_id,
            match_ordinal,
            optional_string_field(item, "id"),
            match_range.0,
            match_range.1,
            match_range.2,
            native_fact_refs_json,
        ],
    )
    .map_err(|error| format!("failed to write syntax match row: {error}"))?;

    for (capture_index, capture) in captures.iter().enumerate() {
        let Some(text) =
            safe_syntax_capture_text(capture).or_else(|| safe_syntax_capture_text(item))
        else {
            continue;
        };
        let capture_range = item
            .get("range")
            .and_then(parse_syntax_range)
            .or_else(|| capture.get("range").and_then(parse_syntax_range))
            .ok_or_else(|| "syntax capture is missing a replayable range".to_string())?;
        let capture_ordinal = capture_index.min(i64::MAX as usize) as i64;
        tx.execute(
            "INSERT OR REPLACE INTO syntax_query_capture (
                generation_id,
                match_ordinal,
                capture_ordinal,
                capture_id,
                capture_name,
                node_type,
                capture_text,
                path,
                start_line,
                end_line
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                generation_id,
                match_ordinal,
                capture_ordinal,
                optional_string_field(capture, "id"),
                optional_string_field(capture, "name").unwrap_or("capture"),
                optional_string_field(capture, "nodeType"),
                text,
                capture_range.0,
                capture_range.1,
                capture_range.2,
            ],
        )
        .map_err(|error| format!("failed to write syntax capture row: {error}"))?;
        for (ref_index, native_fact_ref) in string_array_field(capture, "nativeFactRefs")
            .iter()
            .enumerate()
        {
            tx.execute(
                "INSERT OR REPLACE INTO syntax_query_capture_native_fact_ref (
                    generation_id,
                    match_ordinal,
                    capture_ordinal,
                    ref_ordinal,
                    native_fact_ref
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    generation_id,
                    match_ordinal,
                    capture_ordinal,
                    ref_index.min(i64::MAX as usize) as i64,
                    native_fact_ref,
                ],
            )
            .map_err(|error| format!("failed to write syntax capture native ref row: {error}"))?;
        }
    }
    Ok(())
}

fn parse_syntax_range(range: &Value) -> Option<(String, i64, i64)> {
    let path = optional_string_field(range, "path")?.to_string();
    let line_range = range.get("lineRange")?;
    let (start, end) = if let Some(line_range) = line_range.as_str() {
        let (start, end) = line_range.split_once(':')?;
        (start.parse::<i64>().ok()?, end.parse::<i64>().ok()?)
    } else {
        (
            line_range.get("start")?.as_i64()?,
            line_range.get("end")?.as_i64()?,
        )
    };
    Some((path, start.max(1), end.max(start).max(1)))
}

fn compact_source_locator(path: &str, start_line: i64, end_line: i64) -> String {
    let start_line = start_line.max(1);
    let end_line = end_line.max(start_line);
    if start_line == end_line {
        format!("{path}:{start_line}")
    } else {
        format!("{path}:{start_line}:{end_line}")
    }
}

fn safe_syntax_capture_text(value: &Value) -> Option<&str> {
    value.get("fields").and_then(|fields| {
        optional_string_field(fields, "symbol").or_else(|| optional_string_field(fields, "name"))
    })
}

fn string_array_field(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn string_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    optional_string_field(value, field).ok_or_else(|| format!("missing string field `{field}`"))
}

fn optional_string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

/// Aggregate counts read from the local SQLite client DB.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSummary {
    pub generation_count: u32,
    pub raw_source_stored: bool,
}
