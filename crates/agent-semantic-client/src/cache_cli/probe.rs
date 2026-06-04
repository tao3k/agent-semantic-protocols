//! Cache DB probe and receipt decoration.

use std::path::Path;

use agent_semantic_client_core::{
    ByteCount, CacheManifestReport, CacheStatus, ClientCacheManifest, ClientCachePath,
    ClientDbStatus, ClientMethod, ClientReceipt, ClientRequest, ElapsedMillis, NativeProvenance,
    ProviderRegistrySnapshot,
};
use agent_semantic_client_db::{ClientDb, ClientDbGenerationLookup, ClientDbReport};

use crate::cache_cli::request::{request_export_method, selected_provider_for_request};
use crate::cache_replay::{ProviderCacheReplay, load_replay_artifact};

pub(crate) struct ProviderCacheProbe {
    cache_report: CacheManifestReport,
    db_report: ClientDbReport,
    cache_status: CacheStatus,
    provenance: Vec<NativeProvenance>,
    pub(crate) replay: Option<ProviderCacheReplay>,
}

pub(crate) fn provider_cache_probe(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
) -> Option<ProviderCacheProbe> {
    let cache_report = ClientCacheManifest::inspect_project(project_root);
    let cache_root = cache_report.cache_root.as_ref()?;
    let db_path = ClientDb::default_path(cache_root);
    let db_report = ClientDb::inspect(&db_path);
    let selected_provider = selected_provider_for_request(snapshot, request);
    let provenance = selected_provider
        .map(|provider| vec![provider.provenance()])
        .unwrap_or_default();
    let generation_hit = if db_report.status == ClientDbStatus::Present {
        selected_provider
            .zip(request_export_method(request))
            .and_then(|(provider, export_method)| {
                ClientDb::lookup_generation(&ClientDbGenerationLookup {
                    db_path: db_path.clone(),
                    language_id: provider.language_id.clone(),
                    provider_id: provider.provider_id.clone(),
                    project_root: project_root.to_path_buf(),
                    export_method,
                })
                .ok()
                .flatten()
            })
    } else {
        None
    };
    let replay = generation_hit
        .as_ref()
        .and_then(|hit| load_replay_artifact(cache_root, hit, request));
    let cache_status = if replay.is_some() {
        CacheStatus::Hit
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
        replay,
    })
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
    receipt.client_db_raw_source_stored = Some(probe.db_report.raw_source_stored);
}

pub(crate) fn cache_hit_receipt(
    method: ClientMethod,
    probe: &ProviderCacheProbe,
    replay: &ProviderCacheReplay,
) -> ClientReceipt {
    let mut receipt =
        ClientReceipt::cache_report(method, probe.provenance.clone(), &probe.cache_report);
    apply_provider_cache_probe(&mut receipt, probe);
    receipt.cache_status = CacheStatus::Hit;
    receipt.stdout_bytes = ByteCount::from_len(replay.stdout.len());
    receipt.elapsed_ms = ElapsedMillis::new(0);
    receipt
}
