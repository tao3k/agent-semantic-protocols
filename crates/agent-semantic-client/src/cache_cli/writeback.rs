//! Prompt-output write-back for replay-safe provider results.

use std::fs;
use std::path::Path;

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheManifestStatus, ClientCacheManifest, ClientRequest,
    ElapsedMillis, ProviderCommandReceipt, ProviderRegistrySnapshot,
};
use agent_semantic_client_db::ClientDbEngine;
use bytes::Bytes;

use super::locator_artifact::maybe_write_search_output_artifact;
#[cfg(test)]
use super::locator_artifact::search_output_file_hashes;
use super::probe::{ProviderCacheProbe, provider_cache_probe};
use super::request::{request_export_method, selected_provider_for_request};
use super::writeback_artifact_events::{
    ArtifactEventWriteback, ArtifactKind, artifact_events_for_writeback,
};
#[cfg(test)]
use super::writeback_generation::syntax_query_generation_identity;
use super::writeback_generation::{
    prompt_output_generation, query_packet_generation, query_packet_generation_from_packet,
    search_packet_generation_from_packet, structural_index_generation_from_packet,
    syntax_query_generation,
};
use super::writeback_manifest::{
    load_existing_or_empty_manifest, upsert_generation, write_cache_manifest,
};
#[cfg(test)]
use super::writeback_packet::syntax_query_packet_source;
use super::writeback_packet::{
    validate_query_packet_for_provider, validate_search_packet_for_provider,
    validate_structural_index_packet_for_provider, validate_syntax_query_packet_for_provider,
};
use super::writeback_provider_export::export_provider_packet;
use super::writeback_request::{
    request_prompt_output_writeback_method, request_query_packet_writeback_method,
    request_search_packet_provider_export_method, request_search_packet_writeback_method,
    request_syntax_query_writeback_method,
};
use super::writeback_route_receipt::maybe_write_turso_route_receipt_for_search_packet;
#[cfg(test)]
use crate::cache_replay::ProviderCacheReplay;
use crate::cache_replay::{MAX_CACHE_REPLAY_ARTIFACT_BYTES, replay_artifact_path};

pub(crate) struct CacheWritebackProbe {
    pub(crate) cache_probe: Option<ProviderCacheProbe>,
    #[cfg(test)]
    pub(crate) db_write_count: u64,
    #[cfg(test)]
    pub(crate) replay: Option<ProviderCacheReplay>,
    pub(crate) provider_commands: Vec<ProviderCommandReceipt>,
    pub(crate) provider_elapsed_ms: ElapsedMillis,
}

pub(crate) fn write_prompt_output_cache_after_provider_success(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
    stdout: &[u8],
    provider_commands: &[ProviderCommandReceipt],
) -> Option<CacheWritebackProbe> {
    let provider = selected_provider_for_request(snapshot, request)?;
    let cache_report = ClientCacheManifest::inspect_project(project_root);
    if matches!(cache_report.status, CacheManifestStatus::Unavailable) {
        return None;
    }
    let provider_export_allowed = !matches!(cache_report.status, CacheManifestStatus::Invalid);
    let structural_index_stdout_writeback = if !stdout.is_empty()
        && (stdout.len() as u64) <= MAX_CACHE_REPLAY_ARTIFACT_BYTES
        && validate_structural_index_packet_for_provider(stdout, provider).is_some()
    {
        Some((
            CacheExportMethod::from("index/structural"),
            Bytes::copy_from_slice(stdout),
            "structural-index/",
            ".json",
            ArtifactKind::SemanticStructuralIndex,
            Vec::new(),
            ElapsedMillis::new(0),
        ))
    } else {
        None
    };
    let search_packet_writeback = provider_export_allowed
        .then(|| request_search_packet_provider_export_method(request))
        .flatten()
        .and_then(|export_method| {
            let export = export_provider_packet(provider, request)?;
            validate_search_packet_for_provider(&export.packet_bytes, provider)?;
            Some((
                export_method,
                export.packet_bytes,
                "search/",
                ".json",
                ArtifactKind::SearchPacket,
                vec![export.command],
                export.elapsed_ms,
            ))
        });

    let (
        export_method,
        artifact_bytes,
        artifact_prefix,
        artifact_suffix,
        artifact_kind,
        writeback_provider_commands,
        writeback_provider_elapsed_ms,
    ) = if let Some(structural_index_stdout_writeback) = structural_index_stdout_writeback {
        structural_index_stdout_writeback
    } else if let Some(search_packet_writeback) = search_packet_writeback {
        search_packet_writeback
    } else if let Some(export_method) = request_prompt_output_writeback_method(request) {
        if stdout.is_empty() || stdout.len() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
            return None;
        }
        std::str::from_utf8(stdout).ok()?;
        (
            export_method,
            Bytes::copy_from_slice(stdout),
            "prompt-output/",
            ".txt",
            ArtifactKind::PromptOutput,
            Vec::new(),
            ElapsedMillis::new(0),
        )
    } else if let Some(export_method) = request_syntax_query_writeback_method(request) {
        if !provider_export_allowed {
            return None;
        }
        let export = export_provider_packet(provider, request)?;
        validate_syntax_query_packet_for_provider(&export.packet_bytes, provider)?;
        (
            export_method,
            export.packet_bytes,
            "semantic-tree-sitter-query/",
            ".json",
            ArtifactKind::SemanticTreeSitterQuery,
            vec![export.command],
            export.elapsed_ms,
        )
    } else {
        let export_method = request_query_packet_writeback_method(request)?;
        if !provider_export_allowed {
            return None;
        }
        let export = export_provider_packet(provider, request)?;
        validate_query_packet_for_provider(&export.packet_bytes, provider)?;
        (
            export_method,
            export.packet_bytes,
            "query/",
            ".json",
            ArtifactKind::QueryPacket,
            vec![export.command],
            export.elapsed_ms,
        )
    };

    let cache_probe = (|| {
        let cache_root = cache_report.cache_root.as_ref()?;
        let manifest_path = cache_report.manifest_path.as_ref()?;
        let mut manifest =
            load_existing_or_empty_manifest(cache_root, manifest_path, &cache_report.status);
        let mut generation = match artifact_kind {
            ArtifactKind::PromptOutput => prompt_output_generation(
                project_root,
                provider,
                request,
                &export_method,
                &artifact_bytes,
            ),
            ArtifactKind::SearchPacket => search_packet_generation_from_packet(
                project_root,
                provider,
                request,
                &export_method,
                &artifact_bytes,
                stdout,
            ),
            ArtifactKind::QueryPacket => query_packet_generation(
                project_root,
                provider,
                request,
                &export_method,
                &artifact_bytes,
            ),
            ArtifactKind::SemanticTreeSitterQuery => syntax_query_generation(
                project_root,
                provider,
                request,
                &export_method,
                &artifact_bytes,
            )?,
            ArtifactKind::SemanticStructuralIndex => {
                structural_index_generation_from_packet(project_root, provider, &artifact_bytes)?
            }
        };
        let artifact_id = generation.artifact_ids.as_ref()?.first()?.clone();
        let syntax_generation = if matches!(artifact_kind, ArtifactKind::SemanticTreeSitterQuery) {
            Some(generation.clone())
        } else {
            None
        };
        let structural_generation =
            if matches!(artifact_kind, ArtifactKind::SemanticStructuralIndex) {
                let source_snapshot =
                    crate::source_index::current_source_index_snapshot_with_registry(
                        project_root,
                        snapshot,
                    )
                    .ok()?
                    .source_snapshot;
                Some((generation.clone(), source_snapshot))
            } else {
                None
            };
        let command_artifact_id = if matches!(artifact_kind, ArtifactKind::PromptOutput)
            && !provider_commands.is_empty()
        {
            let command_artifact_id = CacheArtifactId::from(format!(
                "{}.command.json",
                artifact_id.as_str().strip_suffix(".txt")?
            ));
            generation
                .artifact_ids
                .get_or_insert_with(Vec::new)
                .push(command_artifact_id.clone());
            Some(command_artifact_id)
        } else {
            None
        };
        let artifact_path =
            replay_artifact_path(cache_root, &artifact_id, artifact_prefix, artifact_suffix)?;
        fs::create_dir_all(artifact_path.parent()?).ok()?;
        fs::write(&artifact_path, &artifact_bytes).ok()?;
        if matches!(artifact_kind, ArtifactKind::SearchPacket) {
            maybe_write_search_output_artifact(cache_root, &mut generation, stdout);
        }
        if matches!(artifact_kind, ArtifactKind::PromptOutput) {
            maybe_write_search_output_artifact(cache_root, &mut generation, stdout);
        }
        let mut command_artifact_bytes = None;
        if let Some(command_artifact_id) = &command_artifact_id {
            let command_artifact_path = replay_artifact_path(
                cache_root,
                command_artifact_id,
                "prompt-output/",
                ".command.json",
            )?;
            fs::create_dir_all(command_artifact_path.parent()?).ok()?;
            let command_artifact = serde_json::json!({
                "schemaId": "agent.semantic-protocols.client-prompt-output-command",
                "schemaVersion": "1",
                "protocolId": "agent.semantic-protocols.client",
                "protocolVersion": "1",
                "promptOutputArtifactId": artifact_id.as_str(),
                "providerCommands": provider_commands,
            });
            let command_artifact = serde_json::to_vec_pretty(&command_artifact).ok()?;
            command_artifact_bytes = Some(command_artifact.len().min(u64::MAX as usize) as u64);
            fs::write(command_artifact_path, command_artifact).ok()?;
        }
        let artifact_ids_for_events = generation.artifact_ids.clone().unwrap_or_default();
        upsert_generation(&mut manifest, generation);
        write_cache_manifest(manifest_path, &manifest).ok()?;
        ClientDbEngine::import_manifest_from_client_dir(cache_root, &manifest).ok()?;
        let mut db_write_count = 1;
        let artifact_events = artifact_events_for_writeback(ArtifactEventWriteback {
            artifact_kind,
            artifact_id: artifact_id.as_str(),
            artifact_ids: &artifact_ids_for_events,
            artifact_bytes: artifact_bytes.len().min(u64::MAX as usize) as u64,
            command_artifact_id: command_artifact_id.as_ref().map(CacheArtifactId::as_str),
            command_artifact_bytes,
            provider,
            project_root,
            export_method: &export_method,
            artifact_bytes_slice: &artifact_bytes,
            provider_commands,
        });
        if !artifact_events.is_empty() {
            ClientDbEngine::upsert_artifact_events_from_client_dir(cache_root, &artifact_events)
                .ok()?;
            db_write_count += 1;
        }
        if let Some(syntax_generation) = syntax_generation {
            ClientDbEngine::import_semantic_tree_sitter_query_packet_from_client_dir(
                cache_root,
                &syntax_generation,
                &artifact_bytes,
            )
            .ok()?;
            db_write_count += 1;
        }
        if let Some((structural_generation, source_snapshot)) = structural_generation {
            ClientDbEngine::import_semantic_structural_index_refresh_packet_from_client_dir(
                cache_root,
                &structural_generation,
                &artifact_bytes,
                &source_snapshot,
            )
            .ok()?;
            db_write_count += 1;
        }
        let mut probe = provider_cache_probe(project_root, snapshot, request)?;
        probe.db_write_count = db_write_count;
        Some(probe)
    })();
    if cache_probe.is_none() && writeback_provider_commands.is_empty() {
        return None;
    }
    Some(CacheWritebackProbe {
        #[cfg(test)]
        db_write_count: cache_probe.as_ref().map_or(0, |probe| probe.db_write_count),
        #[cfg(test)]
        replay: cache_probe.as_ref().and_then(|probe| probe.replay.clone()),
        cache_probe,
        provider_commands: writeback_provider_commands,
        provider_elapsed_ms: writeback_provider_elapsed_ms,
    })
}

pub(crate) fn write_search_packet_cache_after_provider_success(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
    packet_bytes: &[u8],
    rendered_stdout: &[u8],
) -> Option<ProviderCacheProbe> {
    let provider = selected_provider_for_request(snapshot, request)?;
    let export_method = request_search_packet_writeback_method(request)?;
    validate_search_packet_for_provider(packet_bytes, provider)?;

    let cache_report = ClientCacheManifest::inspect_project(project_root);
    let cache_root = cache_report.cache_root.as_ref()?;
    let manifest_path = cache_report.manifest_path.as_ref()?;
    let mut manifest =
        load_existing_or_empty_manifest(cache_root, manifest_path, &cache_report.status);
    let mut generation = search_packet_generation_from_packet(
        project_root,
        provider,
        request,
        &export_method,
        packet_bytes,
        rendered_stdout,
    );
    let artifact_id = generation.artifact_ids.as_ref()?.first()?.clone();
    let artifact_path = replay_artifact_path(cache_root, &artifact_id, "search/", ".json")?;
    fs::create_dir_all(artifact_path.parent()?).ok()?;
    fs::write(&artifact_path, packet_bytes).ok()?;
    maybe_write_search_output_artifact(cache_root, &mut generation, rendered_stdout);
    let artifact_ids_for_events = generation.artifact_ids.clone().unwrap_or_default();
    upsert_generation(&mut manifest, generation);
    write_cache_manifest(manifest_path, &manifest).ok()?;
    ClientDbEngine::import_manifest_from_client_dir(cache_root, &manifest).ok()?;
    let mut probe = provider_cache_probe(project_root, snapshot, request)?;
    let artifact_events = artifact_events_for_writeback(ArtifactEventWriteback {
        artifact_kind: ArtifactKind::SearchPacket,
        artifact_id: artifact_id.as_str(),
        artifact_ids: &artifact_ids_for_events,
        artifact_bytes: packet_bytes.len().min(u64::MAX as usize) as u64,
        command_artifact_id: None,
        command_artifact_bytes: None,
        provider,
        project_root,
        export_method: &export_method,
        artifact_bytes_slice: packet_bytes,
        provider_commands: &[],
    });
    let mut db_write_count = 1;
    if !artifact_events.is_empty() {
        ClientDbEngine::upsert_artifact_events_from_client_dir(cache_root, &artifact_events)
            .ok()?;
        db_write_count += 1;
    }
    maybe_write_turso_route_receipt_for_search_packet(project_root, packet_bytes, rendered_stdout);
    probe.db_write_count = db_write_count;
    Some(probe)
}

pub(crate) fn write_query_packet_cache_after_provider_success(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
    packet_bytes: &[u8],
) -> Option<ProviderCacheProbe> {
    let provider = selected_provider_for_request(snapshot, request)?;
    let export_method = request_export_method(request)?;
    if export_method.as_str() != "query/owner-items" {
        return None;
    }
    validate_query_packet_for_provider(packet_bytes, provider)?;

    let cache_report = ClientCacheManifest::inspect_project(project_root);
    let cache_root = cache_report.cache_root.as_ref()?;
    let manifest_path = cache_report.manifest_path.as_ref()?;
    let mut manifest =
        load_existing_or_empty_manifest(cache_root, manifest_path, &cache_report.status);
    let mut generation = query_packet_generation_from_packet(
        project_root,
        provider,
        request,
        &export_method,
        packet_bytes,
    );
    let artifact_id = generation.artifact_ids.as_ref()?.first()?.clone();
    let artifact_path = replay_artifact_path(cache_root, &artifact_id, "query/", ".json")?;
    fs::create_dir_all(artifact_path.parent()?).ok()?;
    fs::write(&artifact_path, packet_bytes).ok()?;
    let artifact_ids_for_events = generation.artifact_ids.clone().unwrap_or_default();
    upsert_generation(&mut manifest, generation);
    write_cache_manifest(manifest_path, &manifest).ok()?;
    ClientDbEngine::import_manifest_from_client_dir(cache_root, &manifest).ok()?;
    let mut probe = provider_cache_probe(project_root, snapshot, request)?;
    let artifact_events = artifact_events_for_writeback(ArtifactEventWriteback {
        artifact_kind: ArtifactKind::QueryPacket,
        artifact_id: artifact_id.as_str(),
        artifact_ids: &artifact_ids_for_events,
        artifact_bytes: packet_bytes.len().min(u64::MAX as usize) as u64,
        command_artifact_id: None,
        command_artifact_bytes: None,
        provider,
        project_root,
        export_method: &export_method,
        artifact_bytes_slice: packet_bytes,
        provider_commands: &[],
    });
    let mut db_write_count = 1;
    if !artifact_events.is_empty() {
        ClientDbEngine::upsert_artifact_events_from_client_dir(cache_root, &artifact_events)
            .ok()?;
        db_write_count += 1;
    }
    probe.db_write_count = db_write_count;
    Some(probe)
}

#[cfg(test)]
#[path = "../../tests/unit/cache_cli/writeback/mod.rs"]
mod writeback_tests;
