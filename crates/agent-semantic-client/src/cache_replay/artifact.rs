//! Cache artifact replay implementation.

use std::fs;
use std::path::{Component, Path, PathBuf};

use agent_semantic_client_core::{
    ByteCount, CacheArtifactId, ClientCacheFileHash, ClientMethod, ClientRequest,
    replay_artifact_path, structured_evidence_artifact_path, syntax_query_ast_abi_fingerprint,
};
use agent_semantic_client_db::ClientDbGenerationHit;
use agent_semantic_search::{
    PromptOutputFingerprintRequest, QueryPacketReplayRequest, prompt_output_artifact_replay_safe,
    prompt_output_request_fingerprint as search_prompt_output_request_fingerprint,
    query_packet_matches_request as search_query_packet_matches, render_query_packet_stdout,
    search_output_artifact_replay_safe,
};
use bytes::Bytes;
use serde_json::Value;
use sha2::{Digest, Sha256};

const SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-tree-sitter-query";

use super::limits::MAX_CACHE_REPLAY_ARTIFACT_BYTES;
use super::search_packet::render_search_packet_artifact_stdout;
use super::syntax_query::render_semantic_tree_sitter_query_stdout;

#[derive(Clone)]
pub(crate) struct ProviderCacheReplay {
    pub(crate) stdout: Bytes,
    pub(crate) syntax_artifact_id: Option<CacheArtifactId>,
    pub(crate) packet_bytes: Option<ByteCount>,
    pub(crate) db_read_count: u64,
}

impl ProviderCacheReplay {
    pub(crate) fn stdout(stdout: impl Into<Bytes>) -> Self {
        Self {
            stdout: stdout.into(),
            syntax_artifact_id: None,
            packet_bytes: None,
            db_read_count: 0,
        }
    }

    fn syntax_packet(
        stdout: impl Into<Bytes>,
        syntax_artifact_id: CacheArtifactId,
        packet_bytes: usize,
    ) -> Self {
        Self {
            stdout: stdout.into(),
            syntax_artifact_id: Some(syntax_artifact_id),
            packet_bytes: Some(ByteCount::from_len(packet_bytes)),
            db_read_count: 0,
        }
    }
}

pub(crate) fn load_replay_artifact(
    cache_root: &Path,
    generation_hit: &ClientDbGenerationHit,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    fn normalized_path(path: &Path) -> String {
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .display()
            .to_string()
    }

    fn prompt_output_request_fingerprint(
        generation_hit: &ClientDbGenerationHit,
        request: &ClientRequest,
    ) -> String {
        let forwarded_args = crate::cache_cli::search_cache_forwarded_args(&request.forwarded_args);
        search_prompt_output_request_fingerprint(PromptOutputFingerprintRequest {
            language_id: generation_hit.language_id.as_str(),
            provider_id: generation_hit.provider_id.as_str(),
            normalized_project_root: &normalized_path(&generation_hit.project_root),
            export_method: generation_hit.export_method.as_str(),
            forwarded_args: forwarded_args.as_ref(),
        })
    }

    fn load_prompt_output_artifact(
        cache_root: &Path,
        generation_hit: &ClientDbGenerationHit,
        request: &ClientRequest,
    ) -> Option<ProviderCacheReplay> {
        let expected_fingerprint = prompt_output_request_fingerprint(generation_hit, request);
        if generation_hit.request_fingerprint.as_deref()? != expected_fingerprint {
            return None;
        }

        for artifact_id in &generation_hit.artifact_ids {
            let artifact_path =
                replay_artifact_path(cache_root, artifact_id, "prompt-output/", ".txt")?;
            let metadata = fs::metadata(&artifact_path).ok()?;
            if !metadata.is_file() || metadata.len() > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
                continue;
            }
            let stdout = fs::read(artifact_path).ok()?;
            let stdout_text = std::str::from_utf8(&stdout).ok()?;
            if !prompt_output_artifact_replay_safe(stdout_text) {
                continue;
            }
            return Some(ProviderCacheReplay::stdout(stdout));
        }
        None
    }

    if request.is_hook_direct_source_read() || request.is_source_content_output() {
        return None;
    }
    if !replay_file_hashes_match(&generation_hit.project_root, &generation_hit.file_hashes) {
        return None;
    }
    let has_structured_evidence_artifact = generation_hit
        .artifact_ids
        .iter()
        .any(|artifact_id| structured_evidence_artifact_path(cache_root, artifact_id).is_some());

    load_search_packet_artifact(cache_root, generation_hit, request)
        .or_else(|| load_query_packet_artifact(cache_root, generation_hit, request))
        .or_else(|| load_syntax_query_packet_artifact(cache_root, generation_hit, request))
        .or_else(|| {
            if is_tree_sitter_query_request(request) || has_structured_evidence_artifact {
                None
            } else {
                load_prompt_output_artifact(cache_root, generation_hit, request)
            }
        })
}

fn load_search_packet_artifact(
    cache_root: &Path,
    generation_hit: &ClientDbGenerationHit,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    if request.method != ClientMethod::Search {
        return None;
    }
    generation_hit
        .artifact_ids
        .iter()
        .find_map(|artifact_id| load_search_output_artifact(cache_root, artifact_id))
        .or_else(|| {
            generation_hit
                .artifact_ids
                .iter()
                .find_map(|artifact_id| render_search_packet_artifact(cache_root, artifact_id))
        })
}

fn load_search_output_artifact(
    cache_root: &Path,
    artifact_id: &CacheArtifactId,
) -> Option<ProviderCacheReplay> {
    let artifact_path = replay_artifact_path(cache_root, artifact_id, "search-output/", ".txt")?;
    let metadata = fs::metadata(&artifact_path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return None;
    }
    let stdout = fs::read(artifact_path).ok()?;
    if !search_output_artifact_replay_safe(&stdout) {
        return None;
    }
    Some(ProviderCacheReplay::stdout(stdout))
}

fn render_search_packet_artifact(
    cache_root: &Path,
    artifact_id: &CacheArtifactId,
) -> Option<ProviderCacheReplay> {
    let artifact_path = replay_artifact_path(cache_root, artifact_id, "search/", ".json")?;
    render_search_packet_artifact_stdout(&artifact_path).map(ProviderCacheReplay::stdout)
}

fn load_query_packet_artifact(
    cache_root: &Path,
    generation_hit: &ClientDbGenerationHit,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    if request.method != ClientMethod::Query {
        return None;
    }
    generation_hit
        .artifact_ids
        .iter()
        .find_map(|artifact_id| render_query_packet_artifact(cache_root, artifact_id, request))
}

fn load_syntax_query_packet_artifact(
    cache_root: &Path,
    generation_hit: &ClientDbGenerationHit,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    if request.method != ClientMethod::Query {
        return None;
    }
    generation_hit.artifact_ids.iter().find_map(|artifact_id| {
        render_syntax_query_packet_artifact(cache_root, artifact_id, request)
    })
}

fn render_syntax_query_packet_artifact(
    cache_root: &Path,
    artifact_id: &CacheArtifactId,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    let artifact_path = replay_artifact_path(
        cache_root,
        artifact_id,
        "semantic-tree-sitter-query/",
        ".json",
    )?;
    let metadata = fs::metadata(&artifact_path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return None;
    }
    let packet_bytes = fs::read(artifact_path).ok()?;
    let packet: Value = serde_json::from_slice(&packet_bytes).ok()?;
    semantic_tree_sitter_query_packet_matches_request(&packet, request)?;
    render_semantic_tree_sitter_query_stdout(&packet).map(|stdout| {
        ProviderCacheReplay::syntax_packet(
            stdout.into_bytes(),
            artifact_id.clone(),
            packet_bytes.len(),
        )
    })
}

fn render_query_packet_artifact(
    cache_root: &Path,
    artifact_id: &CacheArtifactId,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    let artifact_path = replay_artifact_path(cache_root, artifact_id, "query/", ".json")?;
    let metadata = fs::metadata(&artifact_path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return None;
    }
    let packet: Value = serde_json::from_slice(&fs::read(artifact_path).ok()?).ok()?;
    query_packet_matches_request(&packet, request)?;
    render_query_packet_stdout(&packet)
        .map(|stdout| ProviderCacheReplay::stdout(stdout.into_bytes()))
}

pub(crate) fn render_query_packet_bytes(packet_bytes: Bytes) -> Option<Bytes> {
    if packet_bytes.is_empty() || packet_bytes.len() as u64 > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return None;
    }
    let packet: Value = serde_json::from_slice(&packet_bytes).ok()?;
    render_query_packet_stdout(&packet).map(Bytes::from)
}

pub(crate) fn query_packet_matches_request(packet: &Value, request: &ClientRequest) -> Option<()> {
    search_query_packet_matches(
        packet,
        QueryPacketReplayRequest {
            is_query_method: request.method == ClientMethod::Query,
            forwarded_args: &request.forwarded_args,
        },
    )
    .then_some(())
}

pub(crate) fn semantic_tree_sitter_query_packet_matches_request(
    packet: &Value,
    request: &ClientRequest,
) -> Option<()> {
    if request.forwarded_args.iter().any(|arg| arg == "--code") {
        return None;
    }
    if string_field(packet, "schemaId")? != SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID {
        return None;
    }
    if string_field(packet, "method")? != "query" {
        return None;
    }
    if !semantic_tree_sitter_query_execution_is_complete(packet) {
        return None;
    }
    let query = packet.get("query")?;
    let request_query_ast_fingerprint =
        request_tree_sitter_query_ast_fingerprint(&request.forwarded_args)?;
    let packet_source =
        string_field(query, "compiledSource").or_else(|| string_field(query, "input"))?;
    let packet_query_ast_fingerprint = syntax_query_ast_abi_fingerprint(packet_source).ok()?;
    if packet_query_ast_fingerprint != request_query_ast_fingerprint {
        return None;
    }
    let packet_selector = query
        .get("fields")
        .and_then(|fields| string_field(fields, "selector"));
    if packet_selector != request_flag_value(&request.forwarded_args, "--selector") {
        return None;
    }
    if query
        .get("fields")
        .and_then(|fields| bool_field(fields, "codeOutput"))
        .unwrap_or(false)
    {
        return None;
    }
    Some(())
}

fn semantic_tree_sitter_query_execution_is_complete(packet: &Value) -> bool {
    let Some(execution) = packet.get("execution") else {
        return false;
    };
    string_field(execution, "engine") == Some("tree-sitter-querycursor")
        && string_field(execution, "predicateEvaluator") == Some("asp-tree-sitter-predicate-v1")
        && string_field(execution, "matchStatus").is_some()
}

#[cfg(test)]
#[path = "../../tests/unit/cache_replay/artifact_execution.rs"]
mod artifact_execution;

fn request_tree_sitter_query_value(forwarded_args: &[String]) -> Option<&str> {
    request_flag_value(forwarded_args, "--treesitter-query")
}

fn is_tree_sitter_query_request(request: &ClientRequest) -> bool {
    request.method == ClientMethod::Query
        && request_tree_sitter_query_value(&request.forwarded_args).is_some()
}

fn request_tree_sitter_query_ast_fingerprint(forwarded_args: &[String]) -> Option<String> {
    request_tree_sitter_query_value(forwarded_args)
        .and_then(|source| syntax_query_ast_abi_fingerprint(source).ok())
}

fn request_flag_value<'a>(forwarded_args: &'a [String], flag: &str) -> Option<&'a str> {
    let prefix = format!("{flag}=");
    let mut iter = forwarded_args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().map(String::as_str);
        }
        if let Some(value) = arg.strip_prefix(&prefix) {
            return Some(value);
        }
    }
    None
}

fn replay_file_hashes_match(project_root: &Path, file_hashes: &[ClientCacheFileHash]) -> bool {
    !file_hashes.is_empty()
        && file_hashes
            .iter()
            .all(|file_hash| replay_file_hash_matches(project_root, file_hash))
}

fn replay_file_hash_matches(project_root: &Path, file_hash: &ClientCacheFileHash) -> bool {
    let Some(path) = safe_project_file_path(project_root, &file_hash.path) else {
        return false;
    };
    let Ok(metadata) = fs::metadata(&path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let digest = Sha256::digest(&bytes);
    format!("{digest:x}").eq_ignore_ascii_case(&file_hash.sha256)
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

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn bool_field(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}
