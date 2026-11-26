// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! BLAKE3 `content_hash`-verified persistence cache for ingested Open EDB facts.
//!
//! Allows Marlin and ASP hosts to persist native-parser and `:gerbil-parser`
//! per-file Open EDB facts across cold restarts (either as an atomic sidecar file
//! or inside `SearchEngineStoragePort` / Turso CAS) while guaranteeing **zero
//! stale facts**: any file whose BLAKE3 `content_hash` no longer matches is
//! skipped automatically so the host only re-extracts changed files.

use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::SearchGenerationDigest;

use super::edb::{WorkspaceEdbEdgeFact, WorkspaceEdbEntityFact};
use super::index::WorkspaceSearchIndex;

/// Schema version for persisted content-hash-keyed Open EDB caches.
pub const OPEN_EDB_CACHE_SCHEMA_VERSION: u32 = 1;

/// One persisted file's Open EDB facts pinned to its exact BLAKE3 `content_hash`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOpenEdbCacheFile {
    pub path: String,
    pub content_hash_hex: String,
    #[serde(default)]
    pub entities: Vec<WorkspaceEdbEntityFact>,
    #[serde(default)]
    pub edges: Vec<WorkspaceEdbEdgeFact>,
}

/// Content-addressed Open EDB cache snapshot over live workspace files.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOpenEdbCache {
    pub schema_version: u32,
    #[serde(default)]
    pub files: Vec<WorkspaceOpenEdbCacheFile>,
}

/// Deterministic receipt returned by [`WorkspaceSearchIndex::restore_open_edb_cache`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOpenEdbRestoreReceipt {
    pub generation_digest: SearchGenerationDigest,
    pub restored_files: usize,
    pub restored_entities: usize,
    pub restored_edges: usize,
    pub stale_or_missing_files: usize,
}

fn encode_hash_hex(hash: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in hash {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

impl WorkspaceSearchIndex {
    /// Export all currently attached per-file Open EDB facts stamped with each
    /// live file's exact BLAKE3 `content_hash`.
    #[must_use]
    pub fn export_open_edb_cache(&self) -> WorkspaceOpenEdbCache {
        let mut files: Vec<WorkspaceOpenEdbCacheFile> = self
            .live_files()
            .filter(|(_, f)| !f.open_edb_entities.is_empty() || !f.open_edb_edges.is_empty())
            .map(|(_, f)| WorkspaceOpenEdbCacheFile {
                path: f.relative_path.clone(),
                content_hash_hex: encode_hash_hex(&f.content_hash),
                entities: f.open_edb_entities.clone(),
                edges: f.open_edb_edges.clone(),
            })
            .collect();
        files.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        WorkspaceOpenEdbCache {
            schema_version: OPEN_EDB_CACHE_SCHEMA_VERSION,
            files,
        }
    }

    /// Restore cached Open EDB facts for every live file whose BLAKE3
    /// `content_hash` matches byte-for-byte; stale or deleted files are skipped
    /// fail-closed.
    pub fn restore_open_edb_cache(
        &mut self,
        cache: WorkspaceOpenEdbCache,
    ) -> WorkspaceOpenEdbRestoreReceipt {
        if cache.schema_version != OPEN_EDB_CACHE_SCHEMA_VERSION {
            return WorkspaceOpenEdbRestoreReceipt {
                generation_digest: self.generation_digest.clone(),
                restored_files: 0,
                restored_entities: 0,
                restored_edges: 0,
                stale_or_missing_files: cache.files.len(),
            };
        }

        let mut restored_files = 0usize;
        let mut restored_entities = 0usize;
        let mut restored_edges = 0usize;
        let mut stale_or_missing_files = 0usize;

        for mut entry in cache.files {
            let clean_path = entry.path.replace('\\', "/");
            let Some(&file_id) = self.path_to_id.get(&clean_path) else {
                stale_or_missing_files += 1;
                continue;
            };
            if self.is_tombstoned(file_id) {
                stale_or_missing_files += 1;
                continue;
            }
            let Some(file) = self.files.get_mut(file_id as usize) else {
                stale_or_missing_files += 1;
                continue;
            };
            if encode_hash_hex(&file.content_hash) != entry.content_hash_hex {
                stale_or_missing_files += 1;
                continue;
            }
            for entity in &mut entry.entities {
                entity.file_id = file_id;
            }
            restored_entities += entry.entities.len();
            restored_edges += entry.edges.len();
            file.open_edb_entities = entry.entities;
            file.open_edb_edges = entry.edges;
            restored_files += 1;
        }

        WorkspaceOpenEdbRestoreReceipt {
            generation_digest: self.generation_digest.clone(),
            restored_files,
            restored_entities,
            restored_edges,
            stale_or_missing_files,
        }
    }

    /// Encode this index's content-hash-verified Open EDB cache as deterministic JSON bytes.
    pub fn encode_open_edb_cache_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.export_open_edb_cache())
    }

    /// Decode a persisted [`WorkspaceOpenEdbCache`] from bytes.
    pub fn decode_open_edb_cache_bytes(
        bytes: &[u8],
    ) -> Result<WorkspaceOpenEdbCache, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Atomically persist the content-hash-verified Open EDB cache to `path`.
    pub fn save_open_edb_cache(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = self
            .encode_open_edb_cache_bytes()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        let tmp_path = path.with_extension("edb-cache.tmp");
        std::fs::write(&tmp_path, bytes)?;
        std::fs::rename(tmp_path, path)
    }

    /// Load an Open EDB cache from `path` (if present) and restore matching files.
    pub fn load_and_restore_open_edb_cache(
        &mut self,
        path: &Path,
    ) -> std::io::Result<WorkspaceOpenEdbRestoreReceipt> {
        let bytes = std::fs::read(path)?;
        let cache = Self::decode_open_edb_cache_bytes(&bytes)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        Ok(self.restore_open_edb_cache(cache))
    }
}
