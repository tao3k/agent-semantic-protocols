//! Prompt-output write-back for replay-safe provider results.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION, ByteCount, CacheArtifactId,
    CacheExportMethod, CacheGenerationId, CacheManifestStatus, CacheStatus, ClientCacheFileHash,
    ClientCacheGeneration, ClientCacheManifest, ClientCachePath, ClientMethod, ClientRequest,
    ElapsedMillis, ProviderCommandReceipt, ProviderRegistrySnapshot, ResolvedProvider,
    SemanticSchemaId, append_syntax_query_plan_args, syntax_query_ast_abi_fingerprint,
};
use agent_semantic_client_db::ClientDb;
use agent_semantic_provider_transport::{
    OutputMode, ProviderProcessLimits, ProviderProcessSpec, StdinMode,
    run_provider_process as run_transport_process,
};
use bytes::Bytes;

use super::locator_artifact::{
    locator_file_hashes_from_packet, maybe_write_search_output_artifact, prompt_output_file_hashes,
    query_selector_file_hashes, search_output_file_hashes, search_packet_file_hashes_from_packet,
};
use super::probe::{ProviderCacheProbe, provider_cache_probe};
use super::request::{
    exact_request_fingerprint, has_tree_sitter_query, request_export_method,
    selected_provider_for_request,
};
use super::writeback_analysis_metadata::maybe_write_analysis_metadata_artifact;
use super::writeback_artifact_events::{
    ArtifactEventWriteback, ArtifactKind, artifact_events_for_writeback,
};
#[cfg(test)]
use crate::cache_replay::ProviderCacheReplay;
use crate::cache_replay::{MAX_CACHE_REPLAY_ARTIFACT_BYTES, replay_artifact_path};

const CLIENT_PROMPT_OUTPUT_SCHEMA_ID: &str = "agent.semantic-protocols.client-prompt-output";
const SEMANTIC_STRUCTURAL_INDEX_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-structural-index";
const SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-tree-sitter-query";

pub(crate) struct CacheWritebackProbe {
    pub(crate) cache_probe: Option<ProviderCacheProbe>,
    #[cfg(test)]
    pub(crate) sqlite_write_count: u64,
    #[cfg(test)]
    pub(crate) replay: Option<ProviderCacheReplay>,
    pub(crate) provider_commands: Vec<ProviderCommandReceipt>,
    pub(crate) provider_elapsed_ms: ElapsedMillis,
}

struct ProviderPacketExport {
    packet_bytes: Bytes,
    command: ProviderCommandReceipt,
    elapsed_ms: ElapsedMillis,
}

pub(crate) fn write_prompt_output_cache_after_provider_success(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
    stdout: &[u8],
    provider_commands: &[ProviderCommandReceipt],
) -> Option<CacheWritebackProbe> {
    fn request_query_packet_writeback_method(request: &ClientRequest) -> Option<CacheExportMethod> {
        if request.method != ClientMethod::Query
            || request
                .forwarded_args
                .iter()
                .any(|arg| arg == "--json" || arg == "--code")
        {
            return None;
        }
        let export_method = request_export_method(request)?;
        if export_method.as_str() == "query/owner-items" {
            Some(export_method)
        } else {
            None
        }
    }

    fn request_syntax_query_writeback_method(request: &ClientRequest) -> Option<CacheExportMethod> {
        if request.method != ClientMethod::Query
            || request
                .forwarded_args
                .iter()
                .any(|arg| arg == "--json" || arg == "--code")
            || !has_tree_sitter_query(&request.forwarded_args)
        {
            return None;
        }
        let export_method = request_export_method(request)?;
        if export_method.as_str() == "query/tree-sitter" {
            Some(export_method)
        } else {
            None
        }
    }

    fn export_provider_packet(
        provider: &ResolvedProvider,
        request: &ClientRequest,
    ) -> Option<ProviderPacketExport> {
        let invocation = provider.command_prefix();
        let (program, prefix_args) = invocation.split_first()?;
        let mut args = prefix_args.to_vec();
        let provider_method = match request.method {
            ClientMethod::Search => "search",
            ClientMethod::Query => "query",
            _ => return None,
        };
        args.push(provider_method.to_string());
        let mut forwarded_args = append_syntax_query_plan_args(
            &request.method,
            Some(&provider.language_id),
            request.forwarded_args.clone(),
        )
        .ok()?;
        insert_json_flag_before_project_root(&mut forwarded_args);
        args.extend(forwarded_args);
        let argv = std::iter::once(program.clone())
            .chain(args.iter().cloned())
            .collect::<Vec<_>>();
        let started = Instant::now();
        let output = run_transport_process(ProviderProcessSpec {
            program: program.clone(),
            args,
            cwd: request.project_root.clone(),
            env: BTreeMap::new(),
            stdin: StdinMode::Closed,
            stdout: OutputMode::Capture,
            stderr: OutputMode::Capture,
            limits: ProviderProcessLimits::default(),
        })
        .ok()?;
        if !output.status.success()
            || output.stdout.is_empty()
            || output.receipt.stdout_bytes as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES
        {
            return None;
        }
        Some(ProviderPacketExport {
            packet_bytes: output.stdout,
            command: ProviderCommandReceipt {
                language_id: provider.language_id.clone(),
                provider_id: provider.provider_id.clone(),
                argv,
                exit_code: output.status.code().unwrap_or(1),
                stdout_bytes: ByteCount::from_len(output.receipt.stdout_bytes),
                stderr_bytes: ByteCount::from_len(output.receipt.stderr_bytes),
                stdout_sha256: output.receipt.stdout_sha256.clone(),
                stderr_sha256: output.receipt.stderr_sha256.clone(),
                stdout_truncated: output.receipt.stdout_truncated,
                stderr_truncated: output.receipt.stderr_truncated,
                timed_out: output.receipt.timed_out,
                elapsed_ms: ElapsedMillis::from_duration(output.receipt.elapsed),
            },
            elapsed_ms: ElapsedMillis::from_duration(started.elapsed()),
        })
    }

    fn validate_search_packet(packet_bytes: &[u8], provider: &ResolvedProvider) -> Option<()> {
        let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
        if packet.get("schemaId")?.as_str()? != "agent.semantic-protocols.semantic-search-packet" {
            return None;
        }
        if packet.get("languageId")?.as_str()? != provider.language_id.as_str() {
            return None;
        }
        if packet.get("providerId")?.as_str()? != provider.provider_id.as_str() {
            return None;
        }
        let has_search_synthesis = packet
            .get("searchSynthesis")
            .and_then(|value| value.as_object())
            .is_some();
        let has_graph = packet
            .get("nodes")
            .and_then(|value| value.as_array())
            .is_some()
            && packet
                .get("edges")
                .and_then(|value| value.as_array())
                .is_some();
        let has_frontier_lists = packet
            .get("owners")
            .and_then(|value| value.as_array())
            .is_some()
            || packet
                .get("hits")
                .and_then(|value| value.as_array())
                .is_some();
        if !has_search_synthesis && !has_graph && !has_frontier_lists {
            return None;
        }
        Some(())
    }

    fn validate_query_packet(packet_bytes: &[u8], provider: &ResolvedProvider) -> Option<()> {
        let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
        if packet.get("schemaId")?.as_str()? != "agent.semantic-protocols.semantic-query-packet" {
            return None;
        }
        if packet.get("method")?.as_str()? != "query/owner-items" {
            return None;
        }
        if packet.get("languageId")?.as_str()? != provider.language_id.as_str() {
            return None;
        }
        if packet.get("providerId")?.as_str()? != provider.provider_id.as_str() {
            return None;
        }
        packet.get("matches")?.as_array()?;
        Some(())
    }

    fn validate_syntax_query_packet(
        packet_bytes: &[u8],
        provider: &ResolvedProvider,
    ) -> Option<()> {
        let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
        if packet.get("schemaId")?.as_str()? != SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID {
            return None;
        }
        if packet.get("languageId")?.as_str()? != provider.language_id.as_str() {
            return None;
        }
        if packet.get("providerId")?.as_str()? != provider.provider_id.as_str() {
            return None;
        }
        packet.get("grammarId")?.as_str()?;
        packet.get("grammarProfileVersion")?.as_str()?;
        let query_source = syntax_query_packet_source(&packet)?;
        syntax_query_ast_abi_fingerprint(query_source).ok()?;
        packet.get("query")?.as_object()?;
        packet.get("matches")?.as_array()?;
        if packet
            .pointer("/cache/artifactKind")
            .and_then(serde_json::Value::as_str)
            != Some("semantic-tree-sitter-query")
        {
            return None;
        }
        Some(())
    }

    fn packet_file_hashes(packet_bytes: &[u8]) -> Option<Vec<ClientCacheFileHash>> {
        let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
        let hashes = packet.pointer("/cache/fileHashes")?.as_array()?;
        let mut file_hashes = Vec::with_capacity(hashes.len());
        for hash in hashes {
            file_hashes.push(ClientCacheFileHash {
                path: hash.get("path")?.as_str()?.to_string(),
                sha256: hash.get("sha256")?.as_str()?.to_string(),
            });
        }
        if file_hashes.is_empty() {
            None
        } else {
            Some(file_hashes)
        }
    }

    fn syntax_query_file_hashes(
        project_root: &Path,
        packet_bytes: &[u8],
    ) -> Option<Vec<ClientCacheFileHash>> {
        packet_file_hashes(packet_bytes)
            .or_else(|| syntax_query_locator_file_hashes(project_root, packet_bytes))
    }

    fn syntax_query_locator_file_hashes(
        project_root: &Path,
        packet_bytes: &[u8],
    ) -> Option<Vec<ClientCacheFileHash>> {
        locator_file_hashes_from_packet(project_root, &[], packet_bytes)
    }

    fn query_packet_generation(
        project_root: &Path,
        provider: &ResolvedProvider,
        request: &ClientRequest,
        export_method: &CacheExportMethod,
        packet_bytes: &[u8],
    ) -> ClientCacheGeneration {
        let seed = format!(
            "{}\0{}\0{}\0{}\0{}\0{}",
            provider.language_id,
            provider.provider_id,
            normalized_path(project_root),
            export_method,
            request.forwarded_args.join("\0"),
            stable_hash_bytes(packet_bytes)
        );
        let hash = stable_hash_hex(&seed);
        let slug = slugify_cache_component(export_method.as_str());
        let generation_id = format!("{}-{slug}-{}", provider.language_id, &hash[..12]);
        let artifact_id = format!("query/{generation_id}.json");
        ClientCacheGeneration {
            generation_id: CacheGenerationId::from(generation_id),
            language_id: provider.language_id.clone(),
            provider_id: provider.provider_id.clone(),
            provider_version: None,
            export_method: Some(export_method.as_str().to_string()),
            project_root: normalized_path(project_root),
            package_root: Some(".".to_string()),
            schema_ids: vec![SemanticSchemaId::from(
                "agent.semantic-protocols.semantic-query-packet",
            )],
            cache_status: CacheStatus::Hit,
            raw_source_stored: false,
            request_fingerprint: Some(exact_request_fingerprint(
                provider,
                project_root,
                export_method,
                &request.forwarded_args,
            )),
            file_hashes: packet_file_hashes(packet_bytes).or_else(|| {
                locator_file_hashes_from_packet(project_root, &provider.package_roots, packet_bytes)
            }),
            artifact_ids: Some(vec![CacheArtifactId::from(artifact_id)]),
        }
    }

    fn syntax_query_generation(
        project_root: &Path,
        provider: &ResolvedProvider,
        request: &ClientRequest,
        export_method: &CacheExportMethod,
        packet_bytes: &[u8],
    ) -> Option<ClientCacheGeneration> {
        let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
        let file_hashes = syntax_query_file_hashes(project_root, packet_bytes);
        let (generation_id, artifact_id) = syntax_query_generation_identity(
            project_root,
            provider,
            export_method,
            &packet,
            file_hashes.as_deref(),
        )?;
        Some(ClientCacheGeneration {
            generation_id: CacheGenerationId::from(generation_id),
            language_id: provider.language_id.clone(),
            provider_id: provider.provider_id.clone(),
            provider_version: None,
            export_method: Some(export_method.as_str().to_string()),
            project_root: normalized_path(project_root),
            package_root: Some(".".to_string()),
            schema_ids: vec![SemanticSchemaId::from(SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID)],
            cache_status: CacheStatus::Hit,
            raw_source_stored: false,
            request_fingerprint: Some(exact_request_fingerprint(
                provider,
                project_root,
                export_method,
                &request.forwarded_args,
            )),
            file_hashes,
            artifact_ids: Some(vec![CacheArtifactId::from(artifact_id)]),
        })
    }

    let provider = selected_provider_for_request(snapshot, request)?;
    let cache_report = ClientCacheManifest::inspect_project(project_root);
    if matches!(
        cache_report.status,
        CacheManifestStatus::Unavailable | CacheManifestStatus::Invalid
    ) {
        return None;
    }
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
    let search_packet_writeback =
        request_search_packet_provider_export_method(request).and_then(|export_method| {
            let export = export_provider_packet(provider, request)?;
            validate_search_packet(&export.packet_bytes, provider)?;
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
        let export = export_provider_packet(provider, request)?;
        validate_syntax_query_packet(&export.packet_bytes, provider)?;
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
        let export = export_provider_packet(provider, request)?;
        validate_query_packet(&export.packet_bytes, provider)?;
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
        let mut manifest = match cache_report.status {
            CacheManifestStatus::Missing => empty_cache_manifest(cache_root),
            CacheManifestStatus::Present => {
                ClientCacheManifest::load_from_path(manifest_path).ok()?
            }
            CacheManifestStatus::Unavailable | CacheManifestStatus::Invalid => unreachable!(),
        };
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
                Some(generation.clone())
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
        let analysis_metadata_artifact = maybe_write_analysis_metadata_artifact(
            cache_root,
            &mut generation,
            &artifact_id,
            artifact_kind,
            provider,
            project_root,
            request,
            &export_method,
            &artifact_bytes,
            stdout,
            provider_commands,
            &writeback_provider_commands,
        );
        let artifact_ids_for_events = generation.artifact_ids.clone().unwrap_or_default();
        upsert_generation(&mut manifest, generation);
        write_cache_manifest(manifest_path, &manifest).ok()?;
        let db_path = ClientDb::default_path(cache_root);
        let mut db = ClientDb::open_or_create(&db_path).ok()?;
        db.import_manifest(&manifest).ok()?;
        let mut sqlite_write_count = 1;
        let artifact_events = artifact_events_for_writeback(ArtifactEventWriteback {
            artifact_kind,
            artifact_id: artifact_id.as_str(),
            artifact_ids: &artifact_ids_for_events,
            artifact_bytes: artifact_bytes.len().min(u64::MAX as usize) as u64,
            command_artifact_id: command_artifact_id.as_ref().map(CacheArtifactId::as_str),
            command_artifact_bytes,
            analysis_metadata_artifact_id: analysis_metadata_artifact
                .as_ref()
                .map(|(artifact_id, _)| artifact_id.as_str()),
            analysis_metadata_artifact_bytes: analysis_metadata_artifact
                .as_ref()
                .map(|(_, bytes)| *bytes),
            provider,
            project_root,
            export_method: &export_method,
            artifact_bytes_slice: &artifact_bytes,
            provider_commands,
        });
        if !artifact_events.is_empty() {
            db.upsert_artifact_events(&artifact_events).ok()?;
            sqlite_write_count += 1;
        }
        if let Some(syntax_generation) = syntax_generation {
            db.import_semantic_tree_sitter_query_packet(&syntax_generation, &artifact_bytes)
                .ok()?;
            sqlite_write_count += 1;
        }
        if let Some(structural_generation) = structural_generation {
            db.import_semantic_structural_index_refresh_packet(
                &structural_generation,
                &artifact_bytes,
            )
            .ok()?;
            sqlite_write_count += 1;
        }
        let mut probe = provider_cache_probe(project_root, snapshot, request)?;
        probe.sqlite_write_count = sqlite_write_count;
        Some(probe)
    })();
    if cache_probe.is_none() && writeback_provider_commands.is_empty() {
        return None;
    }
    Some(CacheWritebackProbe {
        #[cfg(test)]
        sqlite_write_count: cache_probe
            .as_ref()
            .map_or(0, |probe| probe.sqlite_write_count),
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
    let mut manifest = match cache_report.status {
        CacheManifestStatus::Missing => empty_cache_manifest(cache_root),
        CacheManifestStatus::Present => ClientCacheManifest::load_from_path(manifest_path).ok()?,
        CacheManifestStatus::Unavailable | CacheManifestStatus::Invalid => return None,
    };
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
    let analysis_metadata_artifact = maybe_write_analysis_metadata_artifact(
        cache_root,
        &mut generation,
        &artifact_id,
        ArtifactKind::SearchPacket,
        provider,
        project_root,
        request,
        &export_method,
        packet_bytes,
        rendered_stdout,
        &[],
        &[],
    );
    let artifact_ids_for_events = generation.artifact_ids.clone().unwrap_or_default();
    upsert_generation(&mut manifest, generation);
    write_cache_manifest(manifest_path, &manifest).ok()?;
    let db_path = ClientDb::default_path(cache_root);
    let mut db = ClientDb::open_or_create(&db_path).ok()?;
    db.import_manifest(&manifest).ok()?;
    let mut probe = provider_cache_probe(project_root, snapshot, request)?;
    let artifact_events = artifact_events_for_writeback(ArtifactEventWriteback {
        artifact_kind: ArtifactKind::SearchPacket,
        artifact_id: artifact_id.as_str(),
        artifact_ids: &artifact_ids_for_events,
        artifact_bytes: packet_bytes.len().min(u64::MAX as usize) as u64,
        command_artifact_id: None,
        command_artifact_bytes: None,
        analysis_metadata_artifact_id: analysis_metadata_artifact
            .as_ref()
            .map(|(artifact_id, _)| artifact_id.as_str()),
        analysis_metadata_artifact_bytes: analysis_metadata_artifact
            .as_ref()
            .map(|(_, bytes)| *bytes),
        provider,
        project_root,
        export_method: &export_method,
        artifact_bytes_slice: packet_bytes,
        provider_commands: &[],
    });
    let sqlite_write_count = if artifact_events.is_empty() {
        1
    } else {
        db.upsert_artifact_events(&artifact_events).ok()?;
        2
    };
    probe.sqlite_write_count = sqlite_write_count;
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
    let mut manifest = match cache_report.status {
        CacheManifestStatus::Missing => empty_cache_manifest(cache_root),
        CacheManifestStatus::Present => ClientCacheManifest::load_from_path(manifest_path).ok()?,
        CacheManifestStatus::Unavailable | CacheManifestStatus::Invalid => return None,
    };
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
    let analysis_metadata_artifact = maybe_write_analysis_metadata_artifact(
        cache_root,
        &mut generation,
        &artifact_id,
        ArtifactKind::QueryPacket,
        provider,
        project_root,
        request,
        &export_method,
        packet_bytes,
        &[],
        &[],
        &[],
    );
    let artifact_ids_for_events = generation.artifact_ids.clone().unwrap_or_default();
    upsert_generation(&mut manifest, generation);
    write_cache_manifest(manifest_path, &manifest).ok()?;
    let db_path = ClientDb::default_path(cache_root);
    let mut db = ClientDb::open_or_create(&db_path).ok()?;
    db.import_manifest(&manifest).ok()?;
    let mut probe = provider_cache_probe(project_root, snapshot, request)?;
    let artifact_events = artifact_events_for_writeback(ArtifactEventWriteback {
        artifact_kind: ArtifactKind::QueryPacket,
        artifact_id: artifact_id.as_str(),
        artifact_ids: &artifact_ids_for_events,
        artifact_bytes: packet_bytes.len().min(u64::MAX as usize) as u64,
        command_artifact_id: None,
        command_artifact_bytes: None,
        analysis_metadata_artifact_id: analysis_metadata_artifact
            .as_ref()
            .map(|(artifact_id, _)| artifact_id.as_str()),
        analysis_metadata_artifact_bytes: analysis_metadata_artifact
            .as_ref()
            .map(|(_, bytes)| *bytes),
        provider,
        project_root,
        export_method: &export_method,
        artifact_bytes_slice: packet_bytes,
        provider_commands: &[],
    });
    let sqlite_write_count = if artifact_events.is_empty() {
        1
    } else {
        db.upsert_artifact_events(&artifact_events).ok()?;
        2
    };
    probe.sqlite_write_count = sqlite_write_count;
    Some(probe)
}

fn validate_search_packet_for_provider(
    packet_bytes: &[u8],
    provider: &ResolvedProvider,
) -> Option<()> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    if packet.get("schemaId")?.as_str()? != "agent.semantic-protocols.semantic-search-packet" {
        return None;
    }
    if packet.get("languageId")?.as_str()? != provider.language_id.as_str() {
        return None;
    }
    if packet.get("providerId")?.as_str()? != provider.provider_id.as_str() {
        return None;
    }
    let has_search_synthesis = packet
        .get("searchSynthesis")
        .and_then(|value| value.as_object())
        .is_some();
    let has_graph = packet
        .get("nodes")
        .and_then(|value| value.as_array())
        .is_some()
        && packet
            .get("edges")
            .and_then(|value| value.as_array())
            .is_some();
    let has_frontier_lists = packet
        .get("owners")
        .and_then(|value| value.as_array())
        .is_some()
        || packet
            .get("hits")
            .and_then(|value| value.as_array())
            .is_some();
    if !has_search_synthesis && !has_graph && !has_frontier_lists {
        return None;
    }
    Some(())
}

fn validate_query_packet_for_provider(
    packet_bytes: &[u8],
    provider: &ResolvedProvider,
) -> Option<()> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    if packet.get("schemaId")?.as_str()? != "agent.semantic-protocols.semantic-query-packet" {
        return None;
    }
    if packet.get("method")?.as_str()? != "query/owner-items" {
        return None;
    }
    if packet.get("languageId")?.as_str()? != provider.language_id.as_str() {
        return None;
    }
    if packet.get("providerId")?.as_str()? != provider.provider_id.as_str() {
        return None;
    }
    packet.get("matches")?.as_array()?;
    Some(())
}

fn validate_structural_index_packet_for_provider(
    packet_bytes: &[u8],
    provider: &ResolvedProvider,
) -> Option<()> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    if packet.get("schemaId")?.as_str()? != SEMANTIC_STRUCTURAL_INDEX_SCHEMA_ID {
        return None;
    }
    if packet.get("schemaVersion")?.as_str()? != "1" {
        return None;
    }
    if packet.get("languageId")?.as_str()? != provider.language_id.as_str() {
        return None;
    }
    if packet.get("providerId")?.as_str()? != provider.provider_id.as_str() {
        return None;
    }
    if packet
        .get("rawSourceStored")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
    {
        return None;
    }
    if structural_index_file_hashes(&packet)?.is_empty() {
        return None;
    }
    packet.get("owners")?.as_array()?;
    packet.get("symbols")?.as_array()?;
    packet.get("dependencyUsages")?.as_array()?;
    Some(())
}

fn search_packet_generation_from_packet(
    project_root: &Path,
    provider: &ResolvedProvider,
    request: &ClientRequest,
    export_method: &CacheExportMethod,
    packet_bytes: &[u8],
    stdout: &[u8],
) -> ClientCacheGeneration {
    let seed = format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        provider.language_id,
        provider.provider_id,
        normalized_path(project_root),
        export_method,
        request.forwarded_args.join("\0"),
        stable_hash_bytes(packet_bytes)
    );
    let hash = stable_hash_hex(&seed);
    let slug = slugify_cache_component(export_method.as_str());
    let generation_id = format!("{}-{slug}-{}", provider.language_id, &hash[..12]);
    let artifact_id = format!("search/{generation_id}.json");
    ClientCacheGeneration {
        generation_id: CacheGenerationId::from(generation_id),
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        provider_version: None,
        export_method: Some(export_method.as_str().to_string()),
        project_root: normalized_path(project_root),
        package_root: Some(".".to_string()),
        schema_ids: vec![SemanticSchemaId::from(
            "agent.semantic-protocols.semantic-search-packet",
        )],
        cache_status: CacheStatus::Hit,
        raw_source_stored: false,
        request_fingerprint: Some(exact_request_fingerprint(
            provider,
            project_root,
            export_method,
            &request.forwarded_args,
        )),
        file_hashes: search_output_file_hashes(project_root, &provider.package_roots, stdout)
            .or_else(|| {
                search_packet_file_hashes_from_packet(project_root, provider, request, packet_bytes)
            }),
        artifact_ids: Some(vec![CacheArtifactId::from(artifact_id)]),
    }
}

fn query_packet_generation_from_packet(
    project_root: &Path,
    provider: &ResolvedProvider,
    request: &ClientRequest,
    export_method: &CacheExportMethod,
    packet_bytes: &[u8],
) -> ClientCacheGeneration {
    let seed = format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        provider.language_id,
        provider.provider_id,
        normalized_path(project_root),
        export_method,
        request.forwarded_args.join("\0"),
        stable_hash_bytes(packet_bytes)
    );
    let hash = stable_hash_hex(&seed);
    let slug = slugify_cache_component(export_method.as_str());
    let generation_id = format!("{}-{slug}-{}", provider.language_id, &hash[..12]);
    let artifact_id = format!("query/{generation_id}.json");
    ClientCacheGeneration {
        generation_id: CacheGenerationId::from(generation_id),
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        provider_version: None,
        export_method: Some(export_method.as_str().to_string()),
        project_root: normalized_path(project_root),
        package_root: Some(".".to_string()),
        schema_ids: vec![SemanticSchemaId::from(
            "agent.semantic-protocols.semantic-query-packet",
        )],
        cache_status: CacheStatus::Hit,
        raw_source_stored: false,
        request_fingerprint: Some(exact_request_fingerprint(
            provider,
            project_root,
            export_method,
            &request.forwarded_args,
        )),
        file_hashes: query_packet_file_hashes(packet_bytes).or_else(|| {
            locator_file_hashes_from_packet(project_root, &provider.package_roots, packet_bytes)
        }),
        artifact_ids: Some(vec![CacheArtifactId::from(artifact_id)]),
    }
}

fn structural_index_generation_from_packet(
    project_root: &Path,
    provider: &ResolvedProvider,
    packet_bytes: &[u8],
) -> Option<ClientCacheGeneration> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    let generation_id = packet.get("generationId")?.as_str()?.to_string();
    let artifact_id = packet
        .get("sourceArtifactId")
        .and_then(serde_json::Value::as_str)
        .filter(|artifact_id| {
            artifact_id.starts_with("structural-index/") && artifact_id.ends_with(".json")
        })
        .map_or_else(
            || format!("structural-index/{generation_id}.json"),
            ToString::to_string,
        );
    Some(ClientCacheGeneration {
        generation_id: CacheGenerationId::from(generation_id),
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        provider_version: packet
            .get("providerVersion")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        export_method: Some("index/structural".to_string()),
        project_root: packet
            .get("projectRoot")
            .and_then(serde_json::Value::as_str)
            .map_or_else(|| normalized_path(project_root), ToString::to_string),
        package_root: packet
            .get("packageRoot")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string)
            .or_else(|| Some(".".to_string())),
        schema_ids: vec![SemanticSchemaId::from(SEMANTIC_STRUCTURAL_INDEX_SCHEMA_ID)],
        cache_status: CacheStatus::Hit,
        raw_source_stored: false,
        request_fingerprint: Some(structural_index_request_fingerprint(provider, &packet)),
        file_hashes: Some(structural_index_file_hashes(&packet)?),
        artifact_ids: Some(vec![CacheArtifactId::from(artifact_id)]),
    })
}

fn query_packet_file_hashes(packet_bytes: &[u8]) -> Option<Vec<ClientCacheFileHash>> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    let hashes = packet.pointer("/cache/fileHashes")?.as_array()?;
    let mut file_hashes = Vec::with_capacity(hashes.len());
    for hash in hashes {
        file_hashes.push(ClientCacheFileHash {
            path: hash.get("path")?.as_str()?.to_string(),
            sha256: hash.get("sha256")?.as_str()?.to_string(),
        });
    }
    if file_hashes.is_empty() {
        None
    } else {
        Some(file_hashes)
    }
}

fn structural_index_file_hashes(packet: &serde_json::Value) -> Option<Vec<ClientCacheFileHash>> {
    let hashes = packet.get("fileHashes")?.as_array()?;
    let file_hashes = hashes
        .iter()
        .map(|hash| {
            Some(ClientCacheFileHash {
                path: hash.get("path")?.as_str()?.to_string(),
                sha256: hash.get("sha256")?.as_str()?.to_string(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if file_hashes.is_empty() {
        None
    } else {
        Some(file_hashes)
    }
}

fn structural_index_request_fingerprint(
    provider: &ResolvedProvider,
    packet: &serde_json::Value,
) -> String {
    let generation_id = packet
        .get("generationId")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let index_fingerprint = packet
        .get("indexFingerprint")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(generation_id);
    let seed = format!(
        "{}\0{}\0{}\0{}",
        provider.language_id, provider.provider_id, generation_id, index_fingerprint
    );
    format!("fnv64:{}", stable_hash_hex(&seed))
}

fn request_prompt_output_writeback_method(request: &ClientRequest) -> Option<CacheExportMethod> {
    match request.method {
        ClientMethod::Search if is_replayable_search_prompt_output(&request.forwarded_args) => {
            request_export_method(request)
        }
        _ => None,
    }
}

fn request_search_packet_writeback_method(request: &ClientRequest) -> Option<CacheExportMethod> {
    if request.method != ClientMethod::Search
        || request
            .forwarded_args
            .iter()
            .any(|arg| arg == "items" || arg == "ingest" || arg == "--code" || arg == "--json")
        || !(is_seed_search_without_code(&request.forwarded_args)
            || is_dependency_search(&request.forwarded_args))
    {
        return None;
    }
    request_export_method(request)
}

fn request_search_packet_provider_export_method(
    request: &ClientRequest,
) -> Option<CacheExportMethod> {
    if is_prime_seed_search(&request.forwarded_args) {
        return None;
    }
    if !is_search_packet_seed_search(&request.forwarded_args)
        && !is_dependency_search(&request.forwarded_args)
    {
        return None;
    }
    request_search_packet_writeback_method(request)
}

fn is_replayable_search_prompt_output(args: &[String]) -> bool {
    if args.iter().any(|arg| arg == "--code" || arg == "--json") {
        return false;
    }
    is_seed_search_without_code(args) || is_owner_items_search(args) || is_dependency_search(args)
}

fn is_dependency_search(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "deps")
}

fn is_owner_items_search(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "owner") && args.iter().any(|arg| arg == "items")
}

fn is_search_packet_seed_search(args: &[String]) -> bool {
    args.first()
        .is_some_and(|arg| arg == "fzf" || arg == "pipe")
        && is_seed_search_without_code(args)
}

fn insert_json_flag_before_project_root(args: &mut Vec<String>) {
    let insert_at = if args.last().is_some_and(|arg| arg == ".") {
        args.len().saturating_sub(1)
    } else {
        args.len()
    };
    args.insert(insert_at, "--json".to_string());
}

fn is_seed_search_without_code(args: &[String]) -> bool {
    if args
        .iter()
        .any(|arg| arg == "items" || arg == "--code" || arg == "--json")
    {
        return false;
    }
    args.windows(2)
        .any(|window| window[0] == "--view" && window[1] == "seeds")
        || args.iter().any(|arg| arg == "--view=seeds")
}

fn is_prime_seed_search(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "prime") && is_seed_search_without_code(args)
}

fn empty_cache_manifest(cache_root: &Path) -> ClientCacheManifest {
    ClientCacheManifest {
        schema_id: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID.into(),
        schema_version: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION.into(),
        protocol_id: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID.into(),
        protocol_version: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION.into(),
        cache_root: ClientCachePath::from_path(cache_root),
        generations: Vec::new(),
    }
}

fn prompt_output_generation(
    project_root: &Path,
    provider: &ResolvedProvider,
    request: &ClientRequest,
    export_method: &CacheExportMethod,
    stdout: &[u8],
) -> ClientCacheGeneration {
    let seed = format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        provider.language_id,
        provider.provider_id,
        normalized_path(project_root),
        export_method,
        request.forwarded_args.join("\0"),
        stable_hash_bytes(stdout)
    );
    let hash = stable_hash_hex(&seed);
    let slug = slugify_cache_component(export_method.as_str());
    let generation_id = format!("{}-{slug}-{}", provider.language_id, &hash[..12]);
    let artifact_id = format!("prompt-output/{generation_id}.txt");
    ClientCacheGeneration {
        generation_id: CacheGenerationId::from(generation_id),
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        provider_version: None,
        export_method: Some(export_method.as_str().to_string()),
        project_root: normalized_path(project_root),
        package_root: Some(".".to_string()),
        schema_ids: vec![SemanticSchemaId::from(CLIENT_PROMPT_OUTPUT_SCHEMA_ID)],
        cache_status: CacheStatus::Hit,
        raw_source_stored: false,
        request_fingerprint: Some(exact_request_fingerprint(
            provider,
            project_root,
            export_method,
            &request.forwarded_args,
        )),
        file_hashes: prompt_output_file_hashes(project_root, stdout)
            .or_else(|| query_selector_file_hashes(project_root, &request.forwarded_args)),
        artifact_ids: Some(vec![CacheArtifactId::from(artifact_id)]),
    }
}

fn syntax_query_generation_identity(
    project_root: &Path,
    provider: &ResolvedProvider,
    export_method: &CacheExportMethod,
    packet: &serde_json::Value,
    file_hashes: Option<&[ClientCacheFileHash]>,
) -> Option<(String, String)> {
    let query_ast_fingerprint =
        syntax_query_ast_abi_fingerprint(syntax_query_packet_source(packet)?).ok()?;
    let grammar_id = packet.get("grammarId")?.as_str()?;
    let grammar_profile_version = packet.get("grammarProfileVersion")?.as_str()?;
    let selector = packet
        .pointer("/query/fields/selector")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let file_hashes_fingerprint = file_hashes
        .map(syntax_query_file_hashes_fingerprint)
        .unwrap_or_else(|| "none".to_string());
    let seed = format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        provider.language_id,
        provider.provider_id,
        normalized_path(project_root),
        export_method,
        grammar_id,
        grammar_profile_version,
        query_ast_fingerprint,
        selector,
        file_hashes_fingerprint
    );
    let hash = stable_hash_hex(&seed);
    let slug = slugify_cache_component(export_method.as_str());
    let generation_id = format!("{}-{slug}-{}", provider.language_id, &hash[..12]);
    let artifact_id = format!("semantic-tree-sitter-query/{generation_id}.json");
    Some((generation_id, artifact_id))
}

fn syntax_query_packet_source(packet: &serde_json::Value) -> Option<&str> {
    let query = packet.get("query")?;
    query
        .get("compiledSource")
        .and_then(serde_json::Value::as_str)
        .or_else(|| query.get("input").and_then(serde_json::Value::as_str))
}

fn syntax_query_file_hashes_fingerprint(file_hashes: &[ClientCacheFileHash]) -> String {
    let mut entries = file_hashes
        .iter()
        .map(|file_hash| format!("{}\0{}", file_hash.path, file_hash.sha256))
        .collect::<Vec<_>>();
    entries.sort();
    stable_hash_hex(&entries.join("\0"))
}

fn upsert_generation(manifest: &mut ClientCacheManifest, generation: ClientCacheGeneration) {
    manifest
        .generations
        .retain(|existing| existing.generation_id != generation.generation_id);
    manifest.generations.push(generation);
}

fn write_cache_manifest(
    manifest_path: &Path,
    manifest: &ClientCacheManifest,
) -> Result<(), String> {
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create agent semantic client cache manifest directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let text = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("failed to serialize cache manifest: {error}"))?;
    fs::write(manifest_path, text).map_err(|error| {
        format!(
            "failed to write agent semantic client cache manifest at {}: {error}",
            manifest_path.display()
        )
    })
}

fn normalized_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

fn slugify_cache_component(value: &str) -> String {
    let slug = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if slug.is_empty() {
        "request".to_string()
    } else {
        slug
    }
}

fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn stable_hash_hex(value: &str) -> String {
    stable_hash_bytes(value.as_bytes())
}

#[cfg(test)]
#[path = "../../tests/unit/cache_cli/writeback/mod.rs"]
mod writeback_tests;
