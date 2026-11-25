// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! High-performance sub-millisecond converged 4-axis resident workspace search engine,
//! upgraded with `microsoft/tgrep`-grade indexing and query planning.

use super::{build, query};

use std::path::{Path, PathBuf};
use std::time::Instant;

use rustc_hash::FxHashMap;

use crate::file_locator::FileLocatorIndex;
use crate::trigram_query::{TrigramQuery, compile_regex_trigram_query};
use crate::trigram_store::{FlatTrigramStore, TrigramSnapshotError};
use crate::{ConvergedLineMatch, SearchGenerationDigest};

#[cfg(feature = "tantivy-accelerator")]
use crate::tantivy_ram_engine::TantivyRamEngine;

const DEFAULT_MAX_INDEXED_FILE_BYTES: u64 = 2 * 1024 * 1024;
const DEFAULT_MAX_LINE_MATCHES_PER_HIT: usize = 6;
const DEFAULT_MAX_LINE_PREVIEW_CHARS: usize = 220;

/// Declarative indexing policy supplied by the host/config plane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceIndexConfig {
    pub skip_dirs: Vec<String>,
    pub max_file_bytes: u64,
    pub max_line_matches_per_hit: usize,
    pub max_line_preview_chars: usize,
    pub fuzzy_distance: u8,
    pub enable_tantivy: bool,
    pub enable_file_locator: bool,
    pub enable_rg_regex: bool,
}

impl Default for WorkspaceIndexConfig {
    fn default() -> Self {
        Self {
            skip_dirs: vec![".git".to_owned()],
            max_file_bytes: DEFAULT_MAX_INDEXED_FILE_BYTES,
            max_line_matches_per_hit: DEFAULT_MAX_LINE_MATCHES_PER_HIT,
            max_line_preview_chars: DEFAULT_MAX_LINE_PREVIEW_CHARS,
            fuzzy_distance: 1,
            enable_tantivy: true,
            enable_file_locator: true,
            enable_rg_regex: true,
        }
    }
}

/// Incremental file-level update batch applied via [`WorkspaceSearchIndex::apply_file_delta`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceFileDelta {
    /// Added or modified files as `(relative_path, utf8_body_bytes)`.
    pub upserts: Vec<(String, Vec<u8>)>,
    /// Workspace-relative paths removed from the index.
    pub deletes: Vec<String>,
}

/// Deterministic receipt produced by [`WorkspaceSearchIndex::apply_file_delta`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceDeltaReceipt {
    pub generation_digest: SearchGenerationDigest,
    pub upserted_files: usize,
    pub deleted_files: usize,
    pub live_files: usize,
    pub compacted: bool,
    pub elapsed_micros: u64,
}

/// One line of surrounding source context extracted directly from RAM.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceContextLine {
    pub line_number: usize,
    pub text: String,
    pub is_match: bool,
}

/// One ranked converged workspace hit produced by [`WorkspaceSearchIndex::query`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchHit {
    pub path: String,
    pub language: Option<String>,
    pub score: u32,
    pub matched_axes: Vec<String>,
    pub first_line: Option<usize>,
    pub first_line_text: String,
    pub line_matches: Vec<ConvergedLineMatch>,
    pub context_lines: Vec<WorkspaceContextLine>,
}

/// Query parameters for sub-millisecond converged resident search execution.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceSearchQuery<'a> {
    pub pattern: &'a str,
    pub languages: &'a [String],
    pub documents: &'a [String],
    pub path_prefix: Option<&'a str>,
    pub extension: Option<&'a str>,
    pub max_hits: usize,
    pub context_radius: usize,
}

/// Sub-millisecond converged query report with exact microsecond timing and generation digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchReport {
    pub generation_digest: SearchGenerationDigest,
    pub indexed_files: usize,
    pub candidate_files: usize,
    pub matched_files: usize,
    pub elapsed_micros: u64,
    pub hits: Vec<WorkspaceSearchHit>,
}

pub(super) struct IndexedFile {
    pub(super) relative_path: String,
    pub(super) path_lower: Vec<u8>,
    pub(super) ext: String,
    pub(super) language: Option<&'static str>,
    pub(super) content_hash: [u8; 32],
    pub(super) body: String,
    pub(super) body_lower: Vec<u8>,
    pub(super) line_offsets: Vec<u32>,
}

/// Resident, generation-stamped 4-axis search index owned by ASP.
pub struct WorkspaceSearchIndex {
    pub(super) root: PathBuf,
    pub(super) config: WorkspaceIndexConfig,
    pub(super) files: Vec<IndexedFile>,
    pub(super) tombstones: Vec<bool>,
    pub(super) live_file_count: usize,
    pub(super) path_to_id: FxHashMap<String, u32>,
    pub(super) trigrams: FlatTrigramStore,
    pub(super) file_locator: Option<FileLocatorIndex>,
    #[cfg(feature = "tantivy-accelerator")]
    pub(super) tantivy: Option<TantivyRamEngine>,
    pub(super) generation_digest: SearchGenerationDigest,
    pub(super) build_micros: u64,
}

impl WorkspaceSearchIndex {
    /// Build a fresh resident index for `root` using default minimal config.
    #[must_use]
    pub fn build(root: &Path) -> Self {
        Self::build_with_config(root, WorkspaceIndexConfig::default())
    }

    /// Build a fresh resident index for `root` governed by caller-supplied `config`.
    #[must_use]
    pub fn build_with_config(root: &Path, config: WorkspaceIndexConfig) -> Self {
        build::build_from_disk(root, config)
    }

    /// Build a resident index directly from in-memory `(relative_path, body_bytes)` pairs.
    #[must_use]
    pub fn build_from_memory<I, P, B>(root: PathBuf, files: I, config: WorkspaceIndexConfig) -> Self
    where
        I: IntoIterator<Item = (P, B)>,
        P: Into<String>,
        B: Into<Vec<u8>>,
    {
        build::build_from_memory(root, files, config)
    }

    /// Persist a deterministic, BLAKE3-verified binary snapshot (`ASPTGRP1` layout)
    /// to `path` for zero-copy `mmap` cold-start loading.
    pub fn save_snapshot(&self, path: &Path) -> Result<(), TrigramSnapshotError> {
        let (live_files, compacted_store) = build::snapshot_payload_components(self);
        match compacted_store {
            Some(store) => store.save_snapshot(path, &live_files),
            None => self.trigrams.save_snapshot(path, &live_files),
        }
    }

    /// Encode this index into deterministic, BLAKE3-verified binary snapshot bytes.
    #[must_use]
    pub fn encode_snapshot_bytes(&self) -> Vec<u8> {
        let (live_files, compacted_store) = build::snapshot_payload_components(self);
        match compacted_store {
            Some(store) => store.encode_snapshot_bytes(&live_files),
            None => self.trigrams.encode_snapshot_bytes(&live_files),
        }
    }

    /// Open a zero-copy `mmap`-backed index snapshot from `path` with full BLAKE3 validation.
    pub fn load_mmap_snapshot(
        root: PathBuf,
        path: &Path,
        config: WorkspaceIndexConfig,
    ) -> Result<Self, TrigramSnapshotError> {
        let started = Instant::now();
        let (snapshot_files, trigrams) = FlatTrigramStore::load_mmap_snapshot(path)?;
        Ok(build::assemble_from_snapshot(
            root,
            config,
            snapshot_files,
            trigrams,
            started,
        ))
    }

    /// Decode an index snapshot directly from raw bytes (e.g. CAS blob or fuzz test).
    pub fn decode_snapshot_bytes(
        root: PathBuf,
        bytes: &[u8],
        config: WorkspaceIndexConfig,
    ) -> Result<Self, TrigramSnapshotError> {
        let started = Instant::now();
        let (snapshot_files, trigrams) = FlatTrigramStore::decode_snapshot_bytes(bytes)?;
        Ok(build::assemble_from_snapshot(
            root,
            config,
            snapshot_files,
            trigrams,
            started,
        ))
    }

    /// Apply incremental file-level upserts and deletes without rebuilding the full index.
    pub fn apply_file_delta(&mut self, delta: WorkspaceFileDelta) -> WorkspaceDeltaReceipt {
        build::apply_file_delta(self, delta)
    }

    /// Compact tombstones and delta overlays into a clean contiguous flat base table.
    pub fn compact(&mut self) {
        build::compact(self);
    }

    /// Workspace root directory bound to this index.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Number of live indexed files.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.live_file_count
    }

    /// Number of tombstoned file slots awaiting compaction.
    #[must_use]
    pub fn tombstone_count(&self) -> usize {
        self.files.len().saturating_sub(self.live_file_count)
    }

    /// Number of unique trigrams in the base lookup directory.
    #[must_use]
    pub fn unique_trigram_count(&self) -> usize {
        self.trigrams.unique_trigram_count()
    }

    /// Number of 6-byte postings in the base posting arena.
    #[must_use]
    pub fn base_posting_count(&self) -> usize {
        self.trigrams.base_posting_count()
    }

    /// Whether the base trigram store is backed by a zero-copy `mmap`.
    #[must_use]
    pub fn is_mmap_backed(&self) -> bool {
        self.trigrams.is_mmap_backed()
    }

    /// Wall-clock build time in microseconds.
    #[must_use]
    pub fn build_micros(&self) -> u64 {
        self.build_micros
    }

    /// Deterministic BLAKE3 content digest over all live indexed files.
    #[must_use]
    pub fn generation_digest(&self) -> &SearchGenerationDigest {
        &self.generation_digest
    }

    /// Execute a sub-millisecond converged query across resident `tgrep`-grade `rg`,
    /// ASP `FileLocatorIndex`, fuzzy path matching, and in-RAM Tantivy BM25/Fuzzy.
    #[must_use]
    pub fn query(&self, query_req: &WorkspaceSearchQuery<'_>) -> WorkspaceSearchReport {
        query::execute_converged_query(self, query_req)
    }

    /// Compile a regex pattern into its sound [`TrigramQuery`] and return candidate IDs.
    #[must_use]
    pub fn plan_regex_candidates(&self, expr: &str) -> (TrigramQuery, Vec<u32>) {
        let smart_case_insensitive = !expr.chars().any(char::is_uppercase);
        let plan = compile_regex_trigram_query(expr, smart_case_insensitive);
        let candidates = self.trigrams.evaluate_query(&plan, &self.tombstones);
        (plan, candidates)
    }

    #[inline]
    pub(super) fn is_tombstoned(&self, file_id: u32) -> bool {
        self.tombstones
            .get(file_id as usize)
            .copied()
            .unwrap_or(true)
    }

    pub(super) fn live_files(&self) -> impl Iterator<Item = (u32, &IndexedFile)> {
        self.files.iter().enumerate().filter_map(|(idx, file)| {
            if self.tombstones.get(idx).copied().unwrap_or(true) {
                None
            } else {
                Some((u32::try_from(idx).unwrap_or(u32::MAX), file))
            }
        })
    }
}
