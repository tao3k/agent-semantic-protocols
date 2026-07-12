//! Cache DB probe and receipt decoration.

use std::fs;
use std::path::{Component, Path};

use agent_semantic_client_core::{
    ByteCount, CacheExportMethod, CacheManifestReport, CacheStatus, ClientCacheFileHash,
    ClientCacheManifest, ClientCachePath, ClientDbStatus, ClientMethod, ClientReceipt,
    ClientRequest, ElapsedMillis, NativeProvenance, ProviderRegistrySnapshot, ResolvedProvider,
};
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbEngineReadSession, ClientDbGenerationHit, ClientDbReport,
};

use crate::cache_cli::request::{
    request_export_method, request_lookup_fingerprint, selected_provider_for_request,
};
use crate::cache_replay::{
    ProviderCacheReplay, load_replay_artifact, search_lexical_generation_matches_request,
};

const FRESH_LEXICAL_CANDIDATE_LIMIT: u32 = 20;

pub(crate) struct ProviderCacheProbe {
    cache_report: CacheManifestReport,
    db_report: ClientDbReport,
    pub(crate) cache_status: CacheStatus,
    provenance: Vec<NativeProvenance>,
    pub(crate) db_read_count: u64,
    pub(crate) db_write_count: u64,
    pub(crate) replay: Option<ProviderCacheReplay>,
}

pub(crate) fn provider_cache_probe(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
) -> Option<ProviderCacheProbe> {
    if request.is_hook_direct_source_read()
        || request.is_source_content_output()
        || is_structural_item_code_query(request)
    {
        return None;
    }
    let effective_project_root = cache_project_root_for_request(project_root, request);
    let project_root = effective_project_root.as_path();
    let cache_report = ClientCacheManifest::inspect_project(project_root);
    let cache_root = cache_report.cache_root.as_ref()?;
    let db_session = ClientDbEngine::open_read_session_client_dir(cache_root)
        .ok()
        .flatten();
    let db_report = db_session
        .as_ref()
        .and_then(|db_session| db_session.inspect().ok())
        .unwrap_or_else(|| ClientDbEngine::inspect_client_dir(cache_root));
    let mut db_read_count = if db_report.status == ClientDbStatus::Present {
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
        db_session
            .as_ref()
            .zip(selected_provider)
            .zip(export_method.clone())
            .and_then(|((db_session, provider), export_method)| {
                db_read_count += 1;
                let request_fingerprint =
                    request_lookup_fingerprint(provider, project_root, &export_method, request);
                db_session
                    .lookup_generation_request(
                        &provider.language_id,
                        &provider.provider_id,
                        project_root,
                        &export_method,
                        request_fingerprint,
                    )
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
            let db_session = db_session.as_ref()?;
            let provider = selected_provider?;
            let export_method = export_method.as_ref()?;
            if db_report.status != ClientDbStatus::Present
                || !is_fresh_prime_reuse_request(request, export_method)
            {
                return None;
            }
            db_read_count += 1;
            load_fresh_prime_replay(
                db_session,
                cache_root,
                project_root,
                provider,
                export_method,
                request,
            )
        })
        .or_else(|| {
            let db_session = db_session.as_ref()?;
            let provider = selected_provider?;
            let export_method = export_method.as_ref()?;
            if db_report.status != ClientDbStatus::Present
                || !is_fresh_lexical_reuse_request(request, export_method)
            {
                return None;
            }
            db_read_count += 1;
            load_fresh_lexical_replay(
                db_session,
                cache_root,
                project_root,
                provider,
                export_method,
                request,
            )
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
        db_read_count: db_read_count + replay.as_ref().map_or(0, |replay| replay.db_read_count),
        db_write_count: 0,
        replay,
    })
}

fn is_structural_item_code_query(request: &ClientRequest) -> bool {
    if !request.forwarded_args.iter().any(|arg| arg == "--code") {
        return false;
    }
    request
        .forwarded_args
        .windows(2)
        .any(|window| window[0] == "--selector" && is_structural_item_selector(&window[1]))
}

fn is_structural_item_selector(selector: &str) -> bool {
    selector.contains("://") && selector.contains("#item/")
}

fn cache_project_root_for_request(project_root: &Path, request: &ClientRequest) -> PathBuf {
    let Some(candidate) = request.forwarded_args.last() else {
        return project_root.to_path_buf();
    };
    if candidate == "." || candidate.starts_with('-') {
        return project_root.to_path_buf();
    }
    let path = PathBuf::from(candidate);
    if !path.is_absolute() {
        return project_root.to_path_buf();
    }
    path.canonicalize()
        .ok()
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| project_root.to_path_buf())
}

fn load_fresh_prime_replay(
    db_session: &ClientDbEngineReadSession,
    cache_root: &Path,
    project_root: &Path,
    provider: &ResolvedProvider,
    export_method: &CacheExportMethod,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    let hit = db_session
        .lookup_generation_request(
            &provider.language_id,
            &provider.provider_id,
            project_root,
            export_method,
            None,
        )
        .ok()
        .flatten()?;
    if generation_file_hashes_match(project_root, &hit) {
        load_replay_artifact(cache_root, &hit, request)
    } else {
        None
    }
}

fn load_fresh_lexical_replay(
    db_session: &ClientDbEngineReadSession,
    cache_root: &Path,
    project_root: &Path,
    provider: &ResolvedProvider,
    export_method: &CacheExportMethod,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    let hits = db_session
        .lookup_recent_generations_request(
            &provider.language_id,
            &provider.provider_id,
            project_root,
            export_method,
            None,
            FRESH_LEXICAL_CANDIDATE_LIMIT,
        )
        .ok()?;
    for hit in hits {
        if generation_file_hashes_match(project_root, &hit)
            && search_lexical_generation_matches_request(cache_root, &hit, request).is_some()
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

fn is_fresh_lexical_reuse_request(
    request: &ClientRequest,
    export_method: &CacheExportMethod,
) -> bool {
    request.method == ClientMethod::Search
        && export_method.as_str() == "search/lexical"
        && request
            .forwarded_args
            .first()
            .is_some_and(|arg| arg == "lexical")
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
    receipt.db_read_count = Some(probe.db_read_count);
    receipt.db_write_count = Some(probe.db_write_count);
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
use std::path::PathBuf;
