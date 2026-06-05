//! Prompt-output write-back for replay-safe provider results.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION, CacheArtifactId, CacheExportMethod,
    CacheGenerationId, CacheManifestStatus, CacheStatus, ClientCacheFileHash,
    ClientCacheGeneration, ClientCacheManifest, ClientCachePath, ClientMethod, ClientRequest,
    ProviderCommandReceipt, ProviderRegistrySnapshot, ResolvedProvider, SemanticSchemaId,
    append_syntax_query_plan_args, syntax_query_ast_abi_fingerprint,
};
use agent_semantic_client_db::ClientDb;
use sha2::{Digest, Sha256};

use super::probe::{ProviderCacheProbe, provider_cache_probe};
use super::request::{
    exact_request_fingerprint, has_tree_sitter_query, request_export_method,
    selected_provider_for_request,
};
use crate::cache_replay::{MAX_CACHE_REPLAY_ARTIFACT_BYTES, replay_artifact_path};

const CLIENT_PROMPT_OUTPUT_SCHEMA_ID: &str = "agent.semantic-protocols.client-prompt-output";
const SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-tree-sitter-query";

pub(crate) fn write_prompt_output_cache_after_provider_success(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
    stdout: &[u8],
    provider_commands: &[ProviderCommandReceipt],
) -> Option<ProviderCacheProbe> {
    #[derive(Clone, Copy)]
    enum ArtifactKind {
        PromptOutput,
        SearchPacket,
        QueryPacket,
        SemanticTreeSitterQuery,
    }

    fn request_search_packet_writeback_method(
        request: &ClientRequest,
    ) -> Option<CacheExportMethod> {
        if request.method != ClientMethod::Search
            || !is_seed_search_without_code(&request.forwarded_args)
        {
            return None;
        }
        request_export_method(request)
    }

    fn request_query_packet_writeback_method(request: &ClientRequest) -> Option<CacheExportMethod> {
        if request.method != ClientMethod::Query
            || request.forwarded_args.iter().any(|arg| arg == "--json")
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
    ) -> Option<Vec<u8>> {
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
        let output = std::process::Command::new(program)
            .current_dir(&request.project_root)
            .args(args)
            .output()
            .ok()?;
        if !output.status.success()
            || output.stdout.is_empty()
            || output.stdout.len() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES
        {
            return None;
        }
        Some(output.stdout)
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
        locator_file_hashes(project_root, &[], packet_bytes)
    }

    fn locator_file_hashes(
        project_root: &Path,
        package_roots: &[String],
        packet_bytes: &[u8],
    ) -> Option<Vec<ClientCacheFileHash>> {
        let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
        let mut paths = BTreeSet::new();
        collect_json_locator_paths(&packet, &mut paths, None);
        let mut file_hashes = BTreeMap::new();
        for path in paths {
            for file_hash in hash_locator_file(project_root, package_roots, &path) {
                file_hashes
                    .entry(file_hash.path.clone())
                    .or_insert(file_hash);
            }
        }
        let file_hashes = file_hashes.into_values().collect::<Vec<_>>();
        if file_hashes.is_empty() {
            None
        } else {
            Some(file_hashes)
        }
    }

    fn collect_json_locator_paths(
        value: &serde_json::Value,
        paths: &mut BTreeSet<String>,
        key: Option<&str>,
    ) {
        match value {
            serde_json::Value::String(text) if key.is_some_and(is_locator_key) => {
                collect_locator_paths(text, paths);
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    collect_json_locator_paths(item, paths, None);
                }
            }
            serde_json::Value::Object(map) => {
                for (key, value) in map {
                    collect_json_locator_paths(value, paths, Some(key));
                }
            }
            _ => {}
        }
    }

    fn is_locator_key(key: &str) -> bool {
        matches!(
            key,
            "selector"
                | "read"
                | "exactRead"
                | "path"
                | "target"
                | "ownerPath"
                | "matchLocator"
                | "captureLocator"
        )
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
                locator_file_hashes(project_root, &provider.package_roots, packet_bytes)
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
    let search_packet_writeback =
        request_search_packet_writeback_method(request).and_then(|export_method| {
            let packet_bytes = export_provider_packet(provider, request)?;
            validate_search_packet(&packet_bytes, provider)?;
            Some((
                export_method,
                packet_bytes,
                "search/",
                ".json",
                ArtifactKind::SearchPacket,
            ))
        });

    let (export_method, artifact_bytes, artifact_prefix, artifact_suffix, artifact_kind) =
        if let Some(search_packet_writeback) = search_packet_writeback {
            search_packet_writeback
        } else if let Some(export_method) = request_prompt_output_writeback_method(request) {
            if stdout.is_empty() || stdout.len() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
                return None;
            }
            std::str::from_utf8(stdout).ok()?;
            (
                export_method,
                stdout.to_vec(),
                "prompt-output/",
                ".txt",
                ArtifactKind::PromptOutput,
            )
        } else if let Some(export_method) = request_syntax_query_writeback_method(request) {
            let packet_bytes = export_provider_packet(provider, request)?;
            validate_syntax_query_packet(&packet_bytes, provider)?;
            (
                export_method,
                packet_bytes,
                "semantic-tree-sitter-query/",
                ".json",
                ArtifactKind::SemanticTreeSitterQuery,
            )
        } else {
            let export_method = request_query_packet_writeback_method(request)?;
            let packet_bytes = export_provider_packet(provider, request)?;
            validate_query_packet(&packet_bytes, provider)?;
            (
                export_method,
                packet_bytes,
                "query/",
                ".json",
                ArtifactKind::QueryPacket,
            )
        };

    let cache_report = ClientCacheManifest::inspect_project(project_root);
    let cache_root = cache_report.cache_root.as_ref()?;
    let manifest_path = cache_report.manifest_path.as_ref()?;
    let mut manifest = match cache_report.status {
        CacheManifestStatus::Missing => empty_cache_manifest(cache_root),
        CacheManifestStatus::Present => ClientCacheManifest::load_from_path(manifest_path).ok()?,
        CacheManifestStatus::Unavailable | CacheManifestStatus::Invalid => return None,
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
    };
    let artifact_id = generation.artifact_ids.as_ref()?.first()?.clone();
    let syntax_generation = if matches!(artifact_kind, ArtifactKind::SemanticTreeSitterQuery) {
        Some(generation.clone())
    } else {
        None
    };
    let command_artifact_id =
        if matches!(artifact_kind, ArtifactKind::PromptOutput) && !provider_commands.is_empty() {
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
    if let Some(command_artifact_id) = command_artifact_id {
        let command_artifact_path = replay_artifact_path(
            cache_root,
            &command_artifact_id,
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
        fs::write(command_artifact_path, command_artifact).ok()?;
    }
    upsert_generation(&mut manifest, generation);
    write_cache_manifest(manifest_path, &manifest).ok()?;
    let db_path = ClientDb::default_path(cache_root);
    let mut db = ClientDb::open_or_create(&db_path).ok()?;
    db.import_manifest(&manifest).ok()?;
    let mut sqlite_write_count = 1;
    if let Some(syntax_generation) = syntax_generation {
        db.import_semantic_tree_sitter_query_packet(&syntax_generation, &artifact_bytes)
            .ok()?;
        sqlite_write_count += 1;
    }
    let mut probe = provider_cache_probe(project_root, snapshot, request)?;
    probe.sqlite_write_count = sqlite_write_count;
    Some(probe)
}

pub(crate) fn write_search_packet_cache_after_provider_success(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
    packet_bytes: &[u8],
    rendered_stdout: &[u8],
) -> Option<ProviderCacheProbe> {
    let provider = selected_provider_for_request(snapshot, request)?;
    let export_method = request_prompt_output_writeback_method(request)?;
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
    upsert_generation(&mut manifest, generation);
    write_cache_manifest(manifest_path, &manifest).ok()?;
    let db_path = ClientDb::default_path(cache_root);
    let mut db = ClientDb::open_or_create(&db_path).ok()?;
    db.import_manifest(&manifest).ok()?;
    let mut probe = provider_cache_probe(project_root, snapshot, request)?;
    probe.sqlite_write_count = 1;
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

fn search_output_file_hashes(
    project_root: &Path,
    package_roots: &[String],
    stdout: &[u8],
) -> Option<Vec<ClientCacheFileHash>> {
    if !crate::cache_replay::search_output_artifact_replay_safe(stdout) {
        return None;
    }
    locator_file_hashes_from_text(
        project_root,
        package_roots,
        std::str::from_utf8(stdout).ok()?,
    )
}

fn search_packet_file_hashes_from_packet(
    project_root: &Path,
    provider: &ResolvedProvider,
    request: &ClientRequest,
    packet_bytes: &[u8],
) -> Option<Vec<ClientCacheFileHash>> {
    packet_file_hashes_from_packet(packet_bytes)
        .or_else(|| {
            locator_file_hashes_from_packet(project_root, &provider.package_roots, packet_bytes)
        })
        .or_else(|| {
            if request
                .forwarded_args
                .first()
                .is_none_or(|arg| arg != "prime")
            {
                return None;
            }
            let file_hashes = [
                ".cache/agent-semantic-protocol/hooks/activation.json",
                "Cargo.toml",
                "package.json",
                "tsconfig.json",
                "pyproject.toml",
                "Project.toml",
            ]
            .into_iter()
            .filter_map(|path| hash_project_file(project_root, path))
            .collect::<Vec<_>>();
            if file_hashes.is_empty() {
                None
            } else {
                Some(file_hashes)
            }
        })
}

fn packet_file_hashes_from_packet(packet_bytes: &[u8]) -> Option<Vec<ClientCacheFileHash>> {
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

fn locator_file_hashes_from_packet(
    project_root: &Path,
    package_roots: &[String],
    packet_bytes: &[u8],
) -> Option<Vec<ClientCacheFileHash>> {
    let packet: serde_json::Value = serde_json::from_slice(packet_bytes).ok()?;
    let mut paths = BTreeSet::new();
    collect_json_locator_paths(&packet, &mut paths, None);
    locator_file_hashes_from_paths(project_root, package_roots, paths)
}

fn locator_file_hashes_from_text(
    project_root: &Path,
    package_roots: &[String],
    text: &str,
) -> Option<Vec<ClientCacheFileHash>> {
    let mut paths = BTreeSet::new();
    text.lines()
        .for_each(|line| collect_locator_paths(line, &mut paths));
    locator_file_hashes_from_paths(project_root, package_roots, paths)
}

fn locator_file_hashes_from_paths(
    project_root: &Path,
    package_roots: &[String],
    paths: BTreeSet<String>,
) -> Option<Vec<ClientCacheFileHash>> {
    let file_hashes = paths
        .into_iter()
        .flat_map(|path| hash_locator_file(project_root, package_roots, &path))
        .fold(BTreeMap::new(), |mut file_hashes, file_hash| {
            file_hashes
                .entry(file_hash.path.clone())
                .or_insert(file_hash);
            file_hashes
        })
        .into_values()
        .collect::<Vec<_>>();
    if file_hashes.is_empty() {
        None
    } else {
        Some(file_hashes)
    }
}

fn collect_json_locator_paths(
    value: &serde_json::Value,
    paths: &mut BTreeSet<String>,
    key: Option<&str>,
) {
    match value {
        serde_json::Value::String(text) if key.is_some_and(is_locator_key) => {
            collect_locator_paths(text, paths);
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_json_locator_paths(item, paths, None);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                collect_json_locator_paths(value, paths, Some(key));
            }
        }
        _ => {}
    }
}

fn is_locator_key(key: &str) -> bool {
    matches!(
        key,
        "selector"
            | "read"
            | "exactRead"
            | "path"
            | "target"
            | "ownerPath"
            | "matchLocator"
            | "captureLocator"
    )
}

fn maybe_write_search_output_artifact(
    cache_root: &Path,
    generation: &mut ClientCacheGeneration,
    stdout: &[u8],
) {
    if stdout.is_empty()
        || stdout.len() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES
        || !crate::cache_replay::search_output_artifact_replay_safe(stdout)
    {
        return;
    }
    let artifact_id = CacheArtifactId::from(format!(
        "search-output/{}.txt",
        generation.generation_id.as_str()
    ));
    let Some(artifact_path) =
        replay_artifact_path(cache_root, &artifact_id, "search-output/", ".txt")
    else {
        return;
    };
    let Some(parent) = artifact_path.parent() else {
        return;
    };
    if fs::create_dir_all(parent)
        .and_then(|_| fs::write(&artifact_path, stdout))
        .is_ok()
    {
        generation
            .artifact_ids
            .get_or_insert_with(Vec::new)
            .push(artifact_id);
    }
}

fn request_prompt_output_writeback_method(request: &ClientRequest) -> Option<CacheExportMethod> {
    if request.method != ClientMethod::Search
        || !is_seed_search_without_code(&request.forwarded_args)
    {
        return None;
    }
    request_export_method(request)
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
        file_hashes: prompt_output_file_hashes(project_root, stdout),
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

fn prompt_output_file_hashes(
    project_root: &Path,
    stdout: &[u8],
) -> Option<Vec<ClientCacheFileHash>> {
    let text = std::str::from_utf8(stdout).ok()?;
    let mut paths = BTreeSet::new();
    for line in text.lines() {
        collect_locator_paths(line, &mut paths);
    }
    let file_hashes = paths
        .into_iter()
        .filter_map(|path| hash_project_file(project_root, &path))
        .collect::<Vec<_>>();
    if file_hashes.is_empty() {
        None
    } else {
        Some(file_hashes)
    }
}

fn collect_locator_paths(line: &str, paths: &mut BTreeSet<String>) {
    for token in line.split_whitespace() {
        let token = token.trim_matches(|character: char| {
            matches!(character, ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}')
        });
        if token.contains(';') {
            for segment in token.split(';') {
                collect_locator_paths(segment, paths);
            }
            continue;
        }
        if collect_compact_graph_path_tokens(token, paths) {
            continue;
        }
        let token = token
            .strip_prefix("owner:")
            .or_else(|| token.strip_prefix("path:"))
            .or_else(|| token.strip_prefix("read="))
            .or_else(|| token.strip_prefix("target="))
            .unwrap_or(token);
        let path = strip_locator_suffix(token);
        if looks_like_source_path(path) {
            paths.insert(path.to_string());
        }
    }
}

fn collect_compact_graph_path_tokens(token: &str, paths: &mut BTreeSet<String>) -> bool {
    let mut remaining = token;
    let mut found = false;
    while let Some(index) = remaining.find(":path(") {
        let start = index + ":path(".len();
        let Some(end) = remaining[start..].find(')') else {
            break;
        };
        let path = &remaining[start..start + end];
        if looks_like_source_path(path) {
            paths.insert(path.to_string());
            found = true;
        }
        remaining = &remaining[start + end + 1..];
    }
    found
}

fn strip_locator_suffix(value: &str) -> &str {
    let Some((index, _)) = value
        .char_indices()
        .find(|(_, character)| *character == ':')
    else {
        return value;
    };
    let suffix = &value[index + 1..];
    if !suffix.is_empty()
        && suffix
            .chars()
            .all(|character| character.is_ascii_digit() || character == ':')
    {
        &value[..index]
    } else {
        value
    }
}

fn looks_like_source_path(value: &str) -> bool {
    value.ends_with(".rs")
        || value.ends_with(".ts")
        || value.ends_with(".tsx")
        || value.ends_with(".js")
        || value.ends_with(".jsx")
        || value.ends_with(".py")
        || value.ends_with(".jl")
}

fn hash_project_file(project_root: &Path, path: &str) -> Option<ClientCacheFileHash> {
    let file_path = safe_project_file_path(project_root, path)?;
    let bytes = fs::read(file_path).ok()?;
    let digest = Sha256::digest(&bytes);
    Some(ClientCacheFileHash {
        path: path.to_string(),
        sha256: format!("{digest:x}"),
    })
}

fn hash_locator_file(
    project_root: &Path,
    package_roots: &[String],
    path: &str,
) -> Vec<ClientCacheFileHash> {
    std::iter::once(path.to_string())
        .chain(package_roots.iter().filter_map(|package_root| {
            if package_root == "." || package_root.is_empty() {
                return None;
            }
            Some(format!(
                "{}/{}",
                package_root.trim_end_matches('/'),
                path.trim_start_matches("./")
            ))
        }))
        .filter_map(|candidate_path| hash_project_file(project_root, &candidate_path))
        .collect()
}

fn safe_project_file_path(project_root: &Path, path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute() {
        return None;
    }
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(project_root.join(relative))
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
#[path = "../../tests/unit/cache_cli/writeback.rs"]
mod writeback_tests;
