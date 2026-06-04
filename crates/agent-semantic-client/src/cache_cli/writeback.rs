//! Prompt-output write-back for replay-safe provider results.

use std::fs;
use std::path::Path;

use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION, CacheArtifactId, CacheExportMethod,
    CacheGenerationId, CacheManifestStatus, CacheStatus, ClientCacheGeneration,
    ClientCacheManifest, ClientCachePath, ClientMethod, ClientRequest, ProviderRegistrySnapshot,
    ResolvedProvider, SemanticSchemaId,
};
use agent_semantic_client_db::ClientDb;

use super::probe::{ProviderCacheProbe, provider_cache_probe};
use super::request::{request_export_method, selected_provider_for_request};
use crate::cache_replay::{MAX_CACHE_REPLAY_ARTIFACT_BYTES, replay_artifact_path};

const CLIENT_PROMPT_OUTPUT_SCHEMA_ID: &str = "agent.semantic-protocols.client-prompt-output";

pub(crate) fn write_prompt_output_cache_after_provider_success(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
    request: &ClientRequest,
    stdout: &[u8],
) -> Option<ProviderCacheProbe> {
    #[derive(Clone, Copy)]
    enum ArtifactKind {
        PromptOutput,
        SearchPacket,
        QueryPacket,
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
        args.extend(request.forwarded_args.iter().cloned());
        args.push("--json".to_string());
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
        packet.get("searchSynthesis")?.as_object()?;
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

    fn search_packet_generation(
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
                "agent.semantic-protocols.semantic-query-packet",
            )],
            cache_status: CacheStatus::Hit,
            raw_source_stored: false,
            request_fingerprint: None,
            file_hashes: None,
            artifact_ids: Some(vec![CacheArtifactId::from(artifact_id)]),
        }
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
            request_fingerprint: None,
            file_hashes: None,
            artifact_ids: Some(vec![CacheArtifactId::from(artifact_id)]),
        }
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
    let generation = match artifact_kind {
        ArtifactKind::PromptOutput => prompt_output_generation(
            project_root,
            provider,
            request,
            &export_method,
            &artifact_bytes,
        ),
        ArtifactKind::SearchPacket => search_packet_generation(
            project_root,
            provider,
            request,
            &export_method,
            &artifact_bytes,
        ),
        ArtifactKind::QueryPacket => query_packet_generation(
            project_root,
            provider,
            request,
            &export_method,
            &artifact_bytes,
        ),
    };
    let artifact_id = generation.artifact_ids.as_ref()?.first()?.clone();
    let artifact_path =
        replay_artifact_path(cache_root, &artifact_id, artifact_prefix, artifact_suffix)?;
    fs::create_dir_all(artifact_path.parent()?).ok()?;
    fs::write(&artifact_path, artifact_bytes).ok()?;
    upsert_generation(&mut manifest, generation);
    write_cache_manifest(manifest_path, &manifest).ok()?;
    let db_path = ClientDb::default_path(cache_root);
    let mut db = ClientDb::open_or_create(&db_path).ok()?;
    db.import_manifest(&manifest).ok()?;
    provider_cache_probe(project_root, snapshot, request)
}

fn request_prompt_output_writeback_method(request: &ClientRequest) -> Option<CacheExportMethod> {
    if request.method != ClientMethod::Search
        || !is_seed_search_without_code(&request.forwarded_args)
    {
        return None;
    }
    request_export_method(request)
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
    let request_seed = format!(
        "{}\0{}\0{}\0{}\0{}",
        provider.language_id,
        provider.provider_id,
        normalized_path(project_root),
        export_method,
        request.forwarded_args.join("\0")
    );
    let request_fingerprint = format!("fnv64:{}", stable_hash_hex(&request_seed));
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
        request_fingerprint: Some(request_fingerprint),
        file_hashes: None,
        artifact_ids: Some(vec![CacheArtifactId::from(artifact_id)]),
    }
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
