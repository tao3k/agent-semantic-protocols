//! Cache generation construction for write-back artifacts.

use std::path::Path;

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheGenerationId, CacheStatus, ClientCacheFileHash,
    ClientCacheGeneration, ClientRequest, ResolvedProvider, SemanticSchemaId,
    syntax_query_ast_abi_fingerprint,
};

use super::locator_artifact::{
    locator_file_hashes_from_packet, prompt_output_file_hashes, query_selector_file_hashes,
    search_output_file_hashes, search_packet_file_hashes_from_packet,
};
use super::request::exact_request_fingerprint;
use super::writeback_common::{
    normalized_path, slugify_cache_component, stable_hash_bytes, stable_hash_hex,
};
use super::writeback_packet::{
    SEMANTIC_STRUCTURAL_INDEX_SCHEMA_ID, SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID, packet_file_hashes,
    structural_index_file_hashes, syntax_query_packet_source,
};

const CLIENT_PROMPT_OUTPUT_SCHEMA_ID: &str = "agent.semantic-protocols.client-prompt-output";

pub(super) fn prompt_output_generation(
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

pub(super) fn search_packet_generation_from_packet(
    project_root: &Path,
    provider: &ResolvedProvider,
    request: &ClientRequest,
    export_method: &CacheExportMethod,
    packet_bytes: &[u8],
    stdout: &[u8],
) -> ClientCacheGeneration {
    let seed = generation_seed(project_root, provider, request, export_method, packet_bytes);
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

pub(super) fn query_packet_generation_from_packet(
    project_root: &Path,
    provider: &ResolvedProvider,
    request: &ClientRequest,
    export_method: &CacheExportMethod,
    packet_bytes: &[u8],
) -> ClientCacheGeneration {
    let seed = generation_seed(project_root, provider, request, export_method, packet_bytes);
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

pub(super) fn query_packet_generation(
    project_root: &Path,
    provider: &ResolvedProvider,
    request: &ClientRequest,
    export_method: &CacheExportMethod,
    packet_bytes: &[u8],
) -> ClientCacheGeneration {
    query_packet_generation_from_packet(
        project_root,
        provider,
        request,
        export_method,
        packet_bytes,
    )
}

pub(super) fn syntax_query_generation(
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

pub(super) fn structural_index_generation_from_packet(
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

pub(super) fn syntax_query_generation_identity(
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

fn syntax_query_file_hashes(
    project_root: &Path,
    packet_bytes: &[u8],
) -> Option<Vec<ClientCacheFileHash>> {
    packet_file_hashes(packet_bytes)
        .or_else(|| locator_file_hashes_from_packet(project_root, &[], packet_bytes))
}

fn syntax_query_file_hashes_fingerprint(file_hashes: &[ClientCacheFileHash]) -> String {
    let mut entries = file_hashes
        .iter()
        .map(|file_hash| format!("{}\0{}", file_hash.path, file_hash.sha256))
        .collect::<Vec<_>>();
    entries.sort();
    stable_hash_hex(&entries.join("\0"))
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

fn generation_seed(
    project_root: &Path,
    provider: &ResolvedProvider,
    request: &ClientRequest,
    export_method: &CacheExportMethod,
    packet_bytes: &[u8],
) -> String {
    format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        provider.language_id,
        provider.provider_id,
        normalized_path(project_root),
        export_method,
        request.forwarded_args.join("\0"),
        stable_hash_bytes(packet_bytes)
    )
}
