//! Cache DB probe and receipt decoration.

use std::fs;
use std::path::{Component, Path};

use agent_semantic_client_core::{
    ByteCount, CacheExportMethod, CacheManifestReport, CacheStatus, ClientCacheFileHash,
    ClientCacheManifest, ClientCachePath, ClientDbStatus, ClientMethod, ClientReceipt,
    ClientRequest, ElapsedMillis, NativeProvenance, ProviderRegistrySnapshot, ResolvedProvider,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbEngine, ClientDbGenerationHit, ClientDbGenerationLookup, ClientDbReport,
};

use crate::cache_cli::request::{
    request_export_method, request_lookup_fingerprint, selected_provider_for_request,
};
use crate::cache_replay::{
    ProviderCacheReplay, load_replay_artifact, load_syntax_query_rows_replay,
    load_syntax_query_rows_replay_open, search_fzf_generation_matches_request,
};

const FRESH_FZF_CANDIDATE_LIMIT: u32 = 20;

pub(crate) struct ProviderCacheProbe {
    cache_report: CacheManifestReport,
    db_report: ClientDbReport,
    pub(crate) cache_status: CacheStatus,
    provenance: Vec<NativeProvenance>,
    pub(crate) sqlite_read_count: u64,
    pub(crate) sqlite_write_count: u64,
    pub(crate) replay: Option<ProviderCacheReplay>,
}

pub(crate) fn provider_cache_probe(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
) -> Option<ProviderCacheProbe> {
    if request.is_hook_direct_source_read() || request.is_source_content_output() {
        return None;
    }
    let cache_report = ClientCacheManifest::inspect_project(project_root);
    let cache_root = cache_report.cache_root.as_ref()?;
    let db_path = ClientDbEngine::sqlite_path_for_client_dir(cache_root);
    let db = ClientDb::open_read_only_existing(&db_path).ok().flatten();
    let db_report = db
        .as_ref()
        .and_then(|db| db.inspect_open().ok())
        .unwrap_or_else(|| ClientDb::inspect(&db_path));
    let mut sqlite_read_count = if db_report.status == ClientDbStatus::Present {
        1
    } else {
        0
    };
    let selected_provider = selected_provider_for_request(snapshot, request);
    let provenance = selected_provider
        .map(|provider| vec![provider.provenance()])
        .unwrap_or_default();
    let export_method = request_export_method(request);
    let generation_hit = if db_report.status == ClientDbStatus::Present {
        db.as_ref()
            .zip(selected_provider)
            .zip(export_method.clone())
            .and_then(|((db, provider), export_method)| {
                sqlite_read_count += 1;
                let request_fingerprint =
                    request_lookup_fingerprint(provider, project_root, &export_method, request);
                db.lookup_generation_open(&ClientDbGenerationLookup {
                    db_path: db_path.clone(),
                    language_id: provider.language_id.clone(),
                    provider_id: provider.provider_id.clone(),
                    project_root: project_root.to_path_buf(),
                    export_method,
                    request_fingerprint,
                })
                .ok()
                .flatten()
            })
    } else {
        None
    };
    let generation_fresh = generation_hit
        .as_ref()
        .is_some_and(|hit| generation_file_hashes_match(project_root, hit));
    let replay = generation_hit
        .as_ref()
        .and_then(|hit| {
            if generation_fresh {
                load_replay_artifact(cache_root, hit, request)
            } else {
                None
            }
        })
        .or_else(|| {
            let db = db.as_ref()?;
            let provider = selected_provider?;
            let export_method = export_method.as_ref()?;
            if db_report.status != ClientDbStatus::Present
                || !is_fresh_prime_reuse_request(request, export_method)
            {
                return None;
            }
            sqlite_read_count += 1;
            load_fresh_prime_replay(
                db,
                cache_root,
                project_root,
                provider,
                export_method,
                request,
            )
        })
        .or_else(|| {
            let db = db.as_ref()?;
            let provider = selected_provider?;
            let export_method = export_method.as_ref()?;
            if db_report.status != ClientDbStatus::Present
                || !is_fresh_fzf_reuse_request(request, export_method)
            {
                return None;
            }
            sqlite_read_count += 1;
            load_fresh_fzf_replay(
                db,
                cache_root,
                project_root,
                provider,
                export_method,
                request,
            )
        })
        .or_else(|| {
            let provider = selected_provider?;
            let export_method = export_method.as_ref()?;
            if db_report.status != ClientDbStatus::Present
                || export_method.as_str() != "query/tree-sitter"
            {
                return None;
            }
            if let Some(db) = db.as_ref() {
                load_syntax_query_rows_replay_open(
                    db,
                    &provider.language_id,
                    &provider.provider_id,
                    project_root,
                    request,
                )
            } else {
                load_syntax_query_rows_replay(
                    cache_root,
                    &provider.language_id,
                    &provider.provider_id,
                    project_root,
                    request,
                )
            }
        });
    let cache_status = if replay.is_some() {
        CacheStatus::Hit
    } else if generation_hit.is_some() && !generation_fresh {
        CacheStatus::Stale
    } else if generation_hit.is_some() {
        CacheStatus::WarmProvider
    } else {
        CacheStatus::Miss
    };
    Some(ProviderCacheProbe {
        cache_report,
        db_report,
        cache_status,
        provenance,
        sqlite_read_count: sqlite_read_count
            + replay.as_ref().map_or(0, |replay| replay.sqlite_read_count),
        sqlite_write_count: 0,
        replay,
    })
}

fn load_fresh_prime_replay(
    db: &ClientDb,
    cache_root: &Path,
    project_root: &Path,
    provider: &ResolvedProvider,
    export_method: &CacheExportMethod,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    let hit = db
        .lookup_generation_open(&ClientDbGenerationLookup {
            db_path: db.path().to_path_buf(),
            language_id: provider.language_id.clone(),
            provider_id: provider.provider_id.clone(),
            project_root: project_root.to_path_buf(),
            export_method: export_method.clone(),
            request_fingerprint: None,
        })
        .ok()
        .flatten()?;
    if generation_file_hashes_match(project_root, &hit) {
        load_replay_artifact(cache_root, &hit, request)
    } else {
        None
    }
}

fn load_fresh_fzf_replay(
    db: &ClientDb,
    cache_root: &Path,
    project_root: &Path,
    provider: &ResolvedProvider,
    export_method: &CacheExportMethod,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    let hits = db
        .lookup_recent_generations_open(
            &ClientDbGenerationLookup {
                db_path: db.path().to_path_buf(),
                language_id: provider.language_id.clone(),
                provider_id: provider.provider_id.clone(),
                project_root: project_root.to_path_buf(),
                export_method: export_method.clone(),
                request_fingerprint: None,
            },
            FRESH_FZF_CANDIDATE_LIMIT,
        )
        .ok()?;
    for hit in hits {
        if generation_file_hashes_match(project_root, &hit)
            && search_fzf_generation_matches_request(cache_root, &hit, request).is_some()
        {
            return load_replay_artifact(cache_root, &hit, request);
        }
    }
    None
}

fn is_fresh_prime_reuse_request(
    request: &ClientRequest,
    export_method: &CacheExportMethod,
) -> bool {
    request.method == ClientMethod::Search
        && export_method.as_str() == "search/prime"
        && request
            .forwarded_args
            .first()
            .is_some_and(|arg| arg == "prime")
        && request_wants_seed_view(&request.forwarded_args)
        && !request
            .forwarded_args
            .iter()
            .any(|arg| arg == "--json" || arg == "--code")
}

fn is_fresh_fzf_reuse_request(request: &ClientRequest, export_method: &CacheExportMethod) -> bool {
    request.method == ClientMethod::Search
        && export_method.as_str() == "search/fzf"
        && request
            .forwarded_args
            .first()
            .is_some_and(|arg| arg == "fzf")
        && request_wants_seed_view(&request.forwarded_args)
        && !request
            .forwarded_args
            .iter()
            .any(|arg| arg == "--json" || arg == "--code")
}

fn request_wants_seed_view(args: &[String]) -> bool {
    args.windows(2)
        .any(|window| window[0] == "--view" && window[1] == "seeds")
        || args.iter().any(|arg| arg == "--view=seeds")
}

pub(crate) fn generation_file_hashes_match(
    project_root: &Path,
    hit: &ClientDbGenerationHit,
) -> bool {
    !hit.file_hashes.is_empty()
        && hit
            .file_hashes
            .iter()
            .all(|file_hash| file_hash_matches(project_root, file_hash))
}

fn file_hash_matches(project_root: &Path, file_hash: &ClientCacheFileHash) -> bool {
    let Some(path) = safe_project_file_path(project_root, &file_hash.path) else {
        return false;
    };
    let Ok(metadata) = fs::metadata(&path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    file_hash_metadata_matches(&metadata, file_hash)
}

fn file_hash_metadata_matches(metadata: &fs::Metadata, file_hash: &ClientCacheFileHash) -> bool {
    if metadata.len() != file_hash.byte_len {
        return false;
    }
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64 == file_hash.mtime_ms)
        .unwrap_or(false)
}

fn safe_project_file_path(project_root: &Path, path: &str) -> Option<std::path::PathBuf> {
    let path = Path::new(path);
    if path.is_absolute() {
        return None;
    }
    let mut relative = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(project_root.join(relative))
}

pub(crate) fn apply_provider_cache_probe(receipt: &mut ClientReceipt, probe: &ProviderCacheProbe) {
    receipt.cache_status = probe.cache_status;
    receipt.cache_root = probe
        .cache_report
        .cache_root
        .as_ref()
        .map(|path| ClientCachePath::from_path(path));
    receipt.cache_manifest_path = probe
        .cache_report
        .manifest_path
        .as_ref()
        .map(|path| ClientCachePath::from_path(path));
    receipt.cache_manifest_status = Some(probe.cache_report.status.clone());
    receipt.cache_generation_count = Some(probe.cache_report.generation_count);
    receipt.raw_source_stored = Some(probe.cache_report.raw_source_stored);
    receipt.client_db_path = Some(ClientCachePath::from_path(&probe.db_report.db_path));
    receipt.client_db_status = Some(probe.db_report.status.clone());
    receipt.client_db_generation_count = Some(probe.db_report.generation_count);
    receipt.client_db_syntax_row_generation_count =
        Some(probe.db_report.syntax_row_generation_count);
    receipt.client_db_syntax_row_match_count = Some(probe.db_report.syntax_row_match_count);
    receipt.client_db_syntax_row_capture_count = Some(probe.db_report.syntax_row_capture_count);
    receipt.client_db_raw_source_stored = Some(probe.db_report.raw_source_stored);
    if let Some(pragmas) = &probe.db_report.runtime_pragmas {
        receipt.client_db_journal_mode = Some(pragmas.journal_mode.as_str().into());
        receipt.client_db_synchronous = Some(pragmas.synchronous);
        receipt.client_db_busy_timeout_ms = u64::try_from(pragmas.busy_timeout_ms).ok();
        receipt.client_db_foreign_keys = Some(pragmas.foreign_keys);
    }
    receipt.sqlite_read_count = Some(probe.sqlite_read_count);
    receipt.sqlite_write_count = Some(probe.sqlite_write_count);
}

pub(crate) fn cache_hit_receipt(
    method: ClientMethod,
    probe: &ProviderCacheProbe,
    replay: &ProviderCacheReplay,
    elapsed_ms: ElapsedMillis,
) -> ClientReceipt {
    let mut receipt =
        ClientReceipt::cache_report(method, probe.provenance.clone(), &probe.cache_report);
    apply_provider_cache_probe(&mut receipt, probe);
    receipt.cache_status = CacheStatus::Hit;
    receipt.stdout_bytes = ByteCount::from_len(replay.stdout.len());
    receipt.syntax_artifact_id = replay.syntax_artifact_id.clone();
    receipt.packet_bytes = replay.packet_bytes;
    receipt.elapsed_ms = elapsed_ms;
    receipt
}
