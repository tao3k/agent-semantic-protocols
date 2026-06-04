//! SQLite storage adapter for local `agent-semantic-client` cache state.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, ClientCacheManifest, ClientDbStatus, LanguageId, ProviderId,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

/// File name used for the local SQLite client cache.
pub const AGENT_SEMANTIC_CLIENT_DB_FILE: &str = "client.sqlite3";
/// Current SQLite schema version for the local agent semantic client DB.
pub const AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION: i64 = 2;

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
        )
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
    ) -> Result<Option<ClientDbGenerationHit>, String> {
        let project_root = normalized_project_root(project_root);
        let row: Option<(
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            String,
        )> = self
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
               AND raw_source_stored = 0
             ORDER BY updated_at DESC
             LIMIT 1",
                params![
                    language_id.as_str(),
                    provider_id.as_str(),
                    project_root,
                    export_method.as_str(),
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

/// Aggregate counts read from the local SQLite client DB.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSummary {
    pub generation_count: u32,
    pub raw_source_stored: bool,
}
