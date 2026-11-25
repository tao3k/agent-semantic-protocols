// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Disk/memory construction, zero-copy snapshot assembly, incremental delta updates,
//! and compaction for [`super::WorkspaceSearchIndex`].

use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

use ignore::WalkBuilder;
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::SearchGenerationDigest;
use crate::file_locator::FileLocatorIndex;
use crate::trigram_store::{FlatTrigramStore, SnapshotFileRecord, extract_file_trigram_masks};

#[cfg(feature = "tantivy-accelerator")]
use crate::tantivy_ram_engine::TantivyRamEngine;

use super::index::{
    IndexedFile, WorkspaceDeltaReceipt, WorkspaceFileDelta, WorkspaceIndexConfig,
    WorkspaceSearchIndex,
};
use super::symbols::extract_file_semantics;

const PARALLEL_BUILD_THRESHOLD: usize = 64;

struct BuiltFileRecord {
    file: IndexedFile,
    trigrams: Vec<(u32, u8, u8)>,
}

pub(super) fn build_from_disk(root: &Path, config: WorkspaceIndexConfig) -> WorkspaceSearchIndex {
    let started = Instant::now();
    let entries = collect_workspace_entries(root, &config);
    let records: Vec<BuiltFileRecord> = entries
        .into_par_iter()
        .filter_map(|entry| {
            build_file_record_from_disk(
                &entry.abs_path,
                entry.relative_path,
                Some(entry.len),
                entry.mtime_nanos,
            )
        })
        .collect();
    assemble_index(root.to_path_buf(), config, records, started)
}

pub(super) fn build_from_memory<I, P, B>(
    root: PathBuf,
    files: I,
    config: WorkspaceIndexConfig,
) -> WorkspaceSearchIndex
where
    I: IntoIterator<Item = (P, B)>,
    P: Into<String>,
    B: Into<Vec<u8>>,
{
    let started = Instant::now();
    let max_bytes = usize::try_from(config.max_file_bytes.max(1)).unwrap_or(usize::MAX);
    let raw_items: Vec<(String, Vec<u8>)> = files
        .into_iter()
        .filter_map(|(rel, bytes)| {
            let raw = bytes.into();
            if raw.is_empty() || raw.len() > max_bytes {
                return None;
            }
            Some((rel.into(), raw))
        })
        .collect();

    let records: Vec<BuiltFileRecord> = if raw_items.len() >= PARALLEL_BUILD_THRESHOLD {
        raw_items
            .into_par_iter()
            .filter_map(|(rel, raw)| build_file_record_from_bytes(rel, raw))
            .collect()
    } else {
        raw_items
            .into_iter()
            .filter_map(|(rel, raw)| build_file_record_from_bytes(rel, raw))
            .collect()
    };
    assemble_index(root, config, records, started)
}

pub(super) fn assemble_from_snapshot(
    root: PathBuf,
    config: WorkspaceIndexConfig,
    snapshot_files: Vec<SnapshotFileRecord>,
    trigrams: FlatTrigramStore,
    started: Instant,
) -> WorkspaceSearchIndex {
    let mut files = Vec::with_capacity(snapshot_files.len());
    let mut path_to_id =
        FxHashMap::with_capacity_and_hasher(snapshot_files.len(), Default::default());

    for (idx, rec) in snapshot_files.into_iter().enumerate() {
        let file_id = u32::try_from(idx).unwrap_or(u32::MAX);
        let ext = Path::new(&rec.relative_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let language = infer_language(&ext);
        let path_lower = ascii_lower_bytes(rec.relative_path.as_bytes());
        let body_lower = ascii_lower_bytes(rec.body.as_bytes());
        let line_offsets = build_line_offsets(rec.body.as_bytes());
        let semantics = extract_file_semantics(language, &rec.body);
        path_to_id.insert(rec.relative_path.clone(), file_id);
        files.push(IndexedFile {
            relative_path: rec.relative_path,
            path_lower,
            ext,
            language,
            content_hash: rec.content_hash,
            disk_len: None,
            disk_mtime_nanos: None,
            body: rec.body,
            body_lower,
            line_offsets,
            symbols: semantics.symbols,
            imports: semantics.imports,
        });
    }

    finish_index_assembly(root, config, files, path_to_id, trigrams, started)
}

pub(super) fn apply_file_delta(
    index: &mut WorkspaceSearchIndex,
    delta: WorkspaceFileDelta,
) -> WorkspaceDeltaReceipt {
    let started = Instant::now();
    let max_bytes = usize::try_from(index.config.max_file_bytes.max(1)).unwrap_or(usize::MAX);
    let mut deleted_ids: Vec<u32> = Vec::new();
    let mut deleted_count = 0usize;
    let mut upserted_count = 0usize;

    for path in delta.deletes {
        let clean = path.replace('\\', "/");
        if let Some(old_id) = index.path_to_id.remove(&clean)
            && let Some(slot) = index.tombstones.get_mut(old_id as usize)
            && !*slot
        {
            *slot = true;
            index.live_file_count = index.live_file_count.saturating_sub(1);
            deleted_ids.push(old_id);
            deleted_count += 1;
        }
    }

    let mut added_file_ids: Vec<u32> = Vec::new();
    for (raw_path, raw_bytes) in delta.upserts {
        let clean = raw_path.replace('\\', "/");
        if let Some(old_id) = index.path_to_id.remove(&clean)
            && let Some(slot) = index.tombstones.get_mut(old_id as usize)
            && !*slot
        {
            *slot = true;
            index.live_file_count = index.live_file_count.saturating_sub(1);
            deleted_ids.push(old_id);
        }

        if raw_bytes.is_empty() || raw_bytes.len() > max_bytes {
            continue;
        }
        let Some(built) = build_file_record_from_bytes(clean, raw_bytes) else {
            continue;
        };
        let new_id = u32::try_from(index.files.len()).unwrap_or(u32::MAX);
        index.trigrams.insert_delta_file(new_id, &built.trigrams);
        index
            .path_to_id
            .insert(built.file.relative_path.clone(), new_id);
        index.files.push(built.file);
        index.tombstones.push(false);
        index.live_file_count += 1;
        added_file_ids.push(new_id);
        upserted_count += 1;
    }

    let should_compact =
        index.files.len() >= 64 && index.tombstone_count() * 4 >= index.files.len();
    if should_compact {
        compact(index);
    } else {
        refresh_auxiliary_engines_after_delta(index, &deleted_ids, &added_file_ids);
        index.generation_digest = compute_generation_digest_from_live(index.live_files());
    }

    WorkspaceDeltaReceipt {
        generation_digest: index.generation_digest.clone(),
        upserted_files: upserted_count,
        deleted_files: deleted_count,
        live_files: index.live_file_count,
        compacted: should_compact,
        elapsed_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
    }
}

fn refresh_auxiliary_engines_after_delta(
    index: &mut WorkspaceSearchIndex,
    deleted_ids: &[u32],
    added_file_ids: &[u32],
) {
    if index.config.enable_file_locator {
        index.file_locator = Some(FileLocatorIndex::build(
            index
                .live_files()
                .map(|(_, f)| PathBuf::from(&f.relative_path)),
        ));
    }
    #[cfg(feature = "tantivy-accelerator")]
    if index.config.enable_tantivy {
        if let Some(tantivy) = index.tantivy.as_mut() {
            let added_docs: Vec<(u32, &str, &str)> = added_file_ids
                .iter()
                .filter_map(|&fid| {
                    let f = index.files.get(fid as usize)?;
                    Some((fid, f.relative_path.as_str(), f.body.as_str()))
                })
                .collect();
            tantivy.apply_delta(deleted_ids, &added_docs);
        } else if index.live_file_count > 0 {
            let docs: Vec<(u32, &str, &str)> = index
                .live_files()
                .map(|(fid, f)| (fid, f.relative_path.as_str(), f.body.as_str()))
                .collect();
            index.tantivy = TantivyRamEngine::build(&docs);
        }
    }
    #[cfg(not(feature = "tantivy-accelerator"))]
    let _ = (deleted_ids, added_file_ids);
}

pub(super) fn compact(index: &mut WorkspaceSearchIndex) {
    let mut live: Vec<IndexedFile> = Vec::with_capacity(index.live_file_count);
    for (idx, file) in index.files.drain(..).enumerate() {
        if !index.tombstones.get(idx).copied().unwrap_or(true) {
            live.push(file);
        }
    }
    live.sort_unstable_by(|a, b| a.relative_path.cmp(&b.relative_path));
    live.dedup_by(|a, b| a.relative_path == b.relative_path);

    let file_trigrams: Vec<Vec<(u32, u8, u8)>> = if live.len() >= PARALLEL_BUILD_THRESHOLD {
        live.par_iter()
            .map(|f| extract_file_trigram_masks(&f.path_lower, &f.body_lower))
            .collect()
    } else {
        live.iter()
            .map(|f| extract_file_trigram_masks(&f.path_lower, &f.body_lower))
            .collect()
    };

    let mut path_to_id = FxHashMap::with_capacity_and_hasher(live.len(), Default::default());
    for (idx, file) in live.iter().enumerate() {
        path_to_id.insert(
            file.relative_path.clone(),
            u32::try_from(idx).unwrap_or(u32::MAX),
        );
    }

    index.live_file_count = live.len();
    index.tombstones = vec![false; live.len()];
    index.trigrams = FlatTrigramStore::build_from_file_trigrams(&file_trigrams);
    index.files = live;
    index.path_to_id = path_to_id;
    index.generation_digest = compute_generation_digest_from_live(index.live_files());

    index.file_locator = index.config.enable_file_locator.then(|| {
        FileLocatorIndex::build(
            index
                .files
                .iter()
                .map(|file| PathBuf::from(&file.relative_path)),
        )
    });

    #[cfg(feature = "tantivy-accelerator")]
    {
        index.tantivy = if index.config.enable_tantivy && !index.files.is_empty() {
            let docs: Vec<(u32, &str, &str)> = index
                .files
                .iter()
                .enumerate()
                .filter_map(|(idx, file)| {
                    Some((
                        u32::try_from(idx).ok()?,
                        file.relative_path.as_str(),
                        file.body.as_str(),
                    ))
                })
                .collect();
            TantivyRamEngine::build(&docs)
        } else {
            None
        };
    }
}

pub(super) fn snapshot_payload_components(
    index: &WorkspaceSearchIndex,
) -> (Vec<SnapshotFileRecord>, Option<FlatTrigramStore>) {
    let has_tombstones = index.tombstone_count() > 0;
    let mut live_refs: Vec<&IndexedFile> = index.live_files().map(|(_, f)| f).collect();
    let is_already_sorted = live_refs
        .windows(2)
        .all(|w| w[0].relative_path <= w[1].relative_path);

    if !has_tombstones && is_already_sorted {
        let records = live_refs
            .into_iter()
            .map(|f| SnapshotFileRecord {
                relative_path: f.relative_path.clone(),
                content_hash: f.content_hash,
                body: f.body.clone(),
            })
            .collect();
        return (records, None);
    }

    live_refs.sort_unstable_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let file_trigrams: Vec<Vec<(u32, u8, u8)>> = live_refs
        .iter()
        .map(|f| extract_file_trigram_masks(&f.path_lower, &f.body_lower))
        .collect();
    let store = FlatTrigramStore::build_from_file_trigrams(&file_trigrams);
    let records = live_refs
        .into_iter()
        .map(|f| SnapshotFileRecord {
            relative_path: f.relative_path.clone(),
            content_hash: f.content_hash,
            body: f.body.clone(),
        })
        .collect();
    (records, Some(store))
}

fn assemble_index(
    root: PathBuf,
    config: WorkspaceIndexConfig,
    mut records: Vec<BuiltFileRecord>,
    started: Instant,
) -> WorkspaceSearchIndex {
    records.sort_unstable_by(|a, b| a.file.relative_path.cmp(&b.file.relative_path));
    records.dedup_by(|a, b| a.file.relative_path == b.file.relative_path);

    let mut files = Vec::with_capacity(records.len());
    let mut path_to_id = FxHashMap::with_capacity_and_hasher(records.len(), Default::default());
    let mut file_trigrams = Vec::with_capacity(records.len());

    for (idx, rec) in records.into_iter().enumerate() {
        let file_id = u32::try_from(idx).unwrap_or(u32::MAX);
        path_to_id.insert(rec.file.relative_path.clone(), file_id);
        file_trigrams.push(rec.trigrams);
        files.push(rec.file);
    }

    let trigrams = FlatTrigramStore::build_from_file_trigrams(&file_trigrams);
    finish_index_assembly(root, config, files, path_to_id, trigrams, started)
}

fn finish_index_assembly(
    root: PathBuf,
    config: WorkspaceIndexConfig,
    files: Vec<IndexedFile>,
    path_to_id: FxHashMap<String, u32>,
    trigrams: FlatTrigramStore,
    started: Instant,
) -> WorkspaceSearchIndex {
    let live_file_count = files.len();
    let tombstones = vec![false; live_file_count];
    let generation_digest = compute_generation_digest_from_live(
        files
            .iter()
            .enumerate()
            .map(|(i, f)| (u32::try_from(i).unwrap_or(u32::MAX), f)),
    );

    let file_locator = config.enable_file_locator.then(|| {
        FileLocatorIndex::build(files.iter().map(|file| PathBuf::from(&file.relative_path)))
    });

    #[cfg(feature = "tantivy-accelerator")]
    let tantivy = if config.enable_tantivy && !files.is_empty() {
        let docs: Vec<(u32, &str, &str)> = files
            .iter()
            .enumerate()
            .filter_map(|(idx, file)| {
                Some((
                    u32::try_from(idx).ok()?,
                    file.relative_path.as_str(),
                    file.body.as_str(),
                ))
            })
            .collect();
        TantivyRamEngine::build(&docs)
    } else {
        None
    };

    let build_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    WorkspaceSearchIndex {
        root,
        config,
        files,
        tombstones,
        live_file_count,
        path_to_id,
        trigrams,
        file_locator,
        #[cfg(feature = "tantivy-accelerator")]
        tantivy,
        generation_digest,
        build_micros,
    }
}

struct DiskWorkspaceEntry {
    abs_path: PathBuf,
    relative_path: String,
    len: u64,
    mtime_nanos: Option<u128>,
}

enum SyncCheckOutcome {
    Unchanged {
        file_id: u32,
        len: u64,
        mtime_nanos: Option<u128>,
    },
    Upsert {
        relative_path: String,
        bytes: Vec<u8>,
        len: u64,
        mtime_nanos: Option<u128>,
    },
}

pub(super) fn sync_workspace_from_disk(index: &mut WorkspaceSearchIndex) -> WorkspaceDeltaReceipt {
    let started = Instant::now();
    let entries = collect_workspace_entries(&index.root, &index.config);
    let disk_paths: FxHashSet<String> = entries.iter().map(|e| e.relative_path.clone()).collect();

    let deletes: Vec<String> = index
        .path_to_id
        .keys()
        .filter(|path| !disk_paths.contains(*path))
        .cloned()
        .collect();

    // Partition entries into stat-unchanged (zero I/O) vs candidates needing hash check.
    type SyncReadCandidate = (DiskWorkspaceEntry, Option<(u32, [u8; 32])>);
    let mut needs_read: Vec<SyncReadCandidate> = Vec::new();
    for entry in entries {
        if let Some(&fid) = index.path_to_id.get(&entry.relative_path)
            && !index.is_tombstoned(fid)
            && let Some(existing) = index.files.get(fid as usize)
        {
            if existing.disk_len == Some(entry.len)
                && existing.disk_mtime_nanos.is_some()
                && existing.disk_mtime_nanos == entry.mtime_nanos
            {
                continue;
            }
            needs_read.push((entry, Some((fid, existing.content_hash))));
        } else {
            needs_read.push((entry, None));
        }
    }

    let outcomes: Vec<SyncCheckOutcome> = needs_read
        .into_par_iter()
        .filter_map(|(entry, existing)| {
            let bytes = std::fs::read(&entry.abs_path).ok()?;
            if let Some((file_id, prev_hash)) = existing
                && *blake3::hash(&bytes).as_bytes() == prev_hash
            {
                return Some(SyncCheckOutcome::Unchanged {
                    file_id,
                    len: entry.len,
                    mtime_nanos: entry.mtime_nanos,
                });
            }
            Some(SyncCheckOutcome::Upsert {
                relative_path: entry.relative_path,
                bytes,
                len: entry.len,
                mtime_nanos: entry.mtime_nanos,
            })
        })
        .collect();

    let mut upserts: Vec<(String, Vec<u8>)> = Vec::new();
    let mut stat_updates: Vec<(String, u64, Option<u128>)> = Vec::new();
    for outcome in outcomes {
        match outcome {
            SyncCheckOutcome::Unchanged {
                file_id,
                len,
                mtime_nanos,
            } => {
                if let Some(file) = index.files.get_mut(file_id as usize) {
                    file.disk_len = Some(len);
                    file.disk_mtime_nanos = mtime_nanos;
                }
            }
            SyncCheckOutcome::Upsert {
                relative_path,
                bytes,
                len,
                mtime_nanos,
            } => {
                stat_updates.push((relative_path.clone(), len, mtime_nanos));
                upserts.push((relative_path, bytes));
            }
        }
    }

    if upserts.is_empty() && deletes.is_empty() {
        return WorkspaceDeltaReceipt {
            generation_digest: index.generation_digest.clone(),
            upserted_files: 0,
            deleted_files: 0,
            live_files: index.live_file_count,
            compacted: false,
            elapsed_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        };
    }

    let mut receipt = apply_file_delta(index, WorkspaceFileDelta { upserts, deletes });
    for (rel_path, len, mtime_nanos) in stat_updates {
        if let Some(&fid) = index.path_to_id.get(&rel_path)
            && let Some(file) = index.files.get_mut(fid as usize)
        {
            file.disk_len = Some(len);
            file.disk_mtime_nanos = mtime_nanos;
        }
    }
    receipt.elapsed_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    receipt
}

fn collect_workspace_entries(
    root: &Path,
    config: &WorkspaceIndexConfig,
) -> Vec<DiskWorkspaceEntry> {
    let mut entries = Vec::new();
    let skip_dirs = config.skip_dirs.clone();
    let max_file_bytes = config.max_file_bytes.max(1);
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                let name = entry.file_name().to_str().unwrap_or("");
                return !skip_dirs.iter().any(|skip| skip == name);
            }
            true
        })
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.len() == 0 || meta.len() > max_file_bytes {
            continue;
        }
        let mtime_nanos = meta
            .modified()
            .ok()
            .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos());
        let len = meta.len();
        let path = entry.into_path();
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel_path = rel.to_string_lossy().replace('\\', "/");
        entries.push(DiskWorkspaceEntry {
            abs_path: path,
            relative_path: rel_path,
            len,
            mtime_nanos,
        });
    }
    entries
}

fn build_file_record_from_disk(
    abs_path: &Path,
    relative_path: String,
    disk_len: Option<u64>,
    disk_mtime_nanos: Option<u128>,
) -> Option<BuiltFileRecord> {
    let bytes = std::fs::read(abs_path).ok()?;
    let mut rec = build_file_record_from_bytes(relative_path, bytes)?;
    rec.file.disk_len = disk_len;
    rec.file.disk_mtime_nanos = disk_mtime_nanos;
    Some(rec)
}

fn build_file_record_from_bytes(relative_path: String, bytes: Vec<u8>) -> Option<BuiltFileRecord> {
    let clean_path = relative_path.replace('\\', "/");
    if clean_path.is_empty()
        || bytes.is_empty()
        || memchr::memchr(0, &bytes[..bytes.len().min(4096)]).is_some()
    {
        return None;
    }
    let body = String::from_utf8(bytes).ok()?;
    let content_hash = *blake3::hash(body.as_bytes()).as_bytes();
    let ext = Path::new(&clean_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let language = infer_language(&ext);
    let path_lower = ascii_lower_bytes(clean_path.as_bytes());
    let body_lower = ascii_lower_bytes(body.as_bytes());
    let line_offsets = build_line_offsets(body.as_bytes());
    let trigrams = extract_file_trigram_masks(&path_lower, &body_lower);
    let semantics = extract_file_semantics(language, &body);

    Some(BuiltFileRecord {
        file: IndexedFile {
            relative_path: clean_path,
            path_lower,
            ext,
            language,
            content_hash,
            disk_len: None,
            disk_mtime_nanos: None,
            body,
            body_lower,
            line_offsets,
            symbols: semantics.symbols,
            imports: semantics.imports,
        },
        trigrams,
    })
}

fn compute_generation_digest_from_live<'a, I>(live_files: I) -> SearchGenerationDigest
where
    I: Iterator<Item = (u32, &'a IndexedFile)>,
{
    let mut entries: Vec<(&str, &[u8; 32])> = live_files
        .map(|(_, f)| (f.relative_path.as_str(), &f.content_hash))
        .collect();
    entries.sort_unstable_by_key(|(path, _)| *path);

    let mut hasher = blake3::Hasher::new();
    hasher.update(b"asp.converged-resident-index.v3\n");
    for (path, hash) in entries {
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update(hash);
        hasher.update(b"\n");
    }
    let digest = format!("blake3-256:{}", hasher.finalize().to_hex());
    SearchGenerationDigest::new(digest).expect("valid blake3-256 generation digest")
}

pub(super) fn build_line_offsets(bytes: &[u8]) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(64);
    offsets.push(0);
    for pos in memchr::memchr_iter(b'\n', bytes) {
        if let Some(next) = pos.checked_add(1)
            && next < bytes.len()
            && let Ok(off) = u32::try_from(next)
        {
            offsets.push(off);
        }
    }
    offsets
}

pub(super) fn ascii_lower_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        out.push(b.to_ascii_lowercase());
    }
    out
}

fn infer_language(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some("rust"),
        "ts" | "tsx" | "mts" | "cts" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "py" | "pyi" => Some("python"),
        "jl" => Some("julia"),
        "ss" | "scm" => Some("scheme"),
        "org" => Some("org"),
        "md" | "markdown" => Some("markdown"),
        "toml" => Some("toml"),
        "sysml" => Some("sysml"),
        "qnt" => Some("quint"),
        _ => None,
    }
}
