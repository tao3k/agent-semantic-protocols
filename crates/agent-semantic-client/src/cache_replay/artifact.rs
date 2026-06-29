//! Cache artifact replay implementation.

use std::fs;
use std::path::{Component, Path, PathBuf};

use agent_semantic_client_core::{
    ByteCount, CacheArtifactId, ClientCacheFileHash, ClientMethod, ClientRequest, LanguageId,
    ProviderId, syntax_query_ast_abi_fingerprint,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbEngine, ClientDbGenerationHit, ClientDbSyntaxQueryLookup,
};
use bytes::Bytes;
use serde_json::Value;
use sha2::{Digest, Sha256};

const SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-tree-sitter-query";

use super::limits::MAX_CACHE_REPLAY_ARTIFACT_BYTES;
use super::search_packet::{
    render_search_packet_artifact_stdout, search_output_artifact_replay_safe,
};
use super::syntax_query::{
    render_semantic_tree_sitter_query_rows_stdout, render_semantic_tree_sitter_query_stdout,
};

#[derive(Clone)]
pub(crate) struct ProviderCacheReplay {
    pub(crate) stdout: Bytes,
    pub(crate) syntax_artifact_id: Option<CacheArtifactId>,
    pub(crate) packet_bytes: Option<ByteCount>,
    pub(crate) sqlite_read_count: u64,
}

impl ProviderCacheReplay {
    pub(crate) fn stdout(stdout: impl Into<Bytes>) -> Self {
        Self {
            stdout: stdout.into(),
            syntax_artifact_id: None,
            packet_bytes: None,
            sqlite_read_count: 0,
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
            sqlite_read_count: 0,
        }
    }

    fn syntax_rows(
        stdout: impl Into<Bytes>,
        syntax_artifact_id: Option<CacheArtifactId>,
        packet_bytes: Option<u64>,
    ) -> Self {
        Self {
            stdout: stdout.into(),
            syntax_artifact_id,
            packet_bytes: packet_bytes.map(ByteCount::new),
            sqlite_read_count: 1,
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

    fn prompt_output_request_fingerprint(
        generation_hit: &ClientDbGenerationHit,
        request: &ClientRequest,
    ) -> String {
        let prompt_output_provenance =
            prompt_output_render_abi_provenance(generation_hit.export_method.as_str());
        let seed = format!(
            "{}\0{}\0{}\0{}\0{}\0{}\0{}",
            generation_hit.language_id,
            generation_hit.provider_id,
            normalized_path(&generation_hit.project_root),
            generation_hit.export_method,
            request.forwarded_args.join("\0"),
            "syntax-query-ast-abi:none",
            prompt_output_provenance
        );
        format!("fnv64:{}", stable_hash_hex(&seed))
    }

    fn prompt_output_render_abi_provenance(export_method: &str) -> String {
        if matches!(export_method, "search/prime" | "search/package") {
            return format!(
                "prompt-output-render-abi:fnv64:{}",
                stable_hash_hex(PRIME_DECISION_PRIMER_RENDER_ABI)
            );
        }
        "prompt-output-render-abi:none".to_string()
    }

    const PRIME_DECISION_PRIMER_RENDER_ABI: &str = concat!(
        "semantic-search-prime;",
        "purpose=decision-primer;",
        "answer=false;",
        "code=false;",
        "capabilities=pipe,lexical,fd-query,rg-query,owner-items,selector-code,treesitter-query;",
        "ladder=pipe>lexical>fd-query|rg-query>owner-items>selector-code;",
        "history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath;",
        "risk=broad-direct-read,manual-window-scan,repeat-prime;",
        "next=search pipe <question-or-feature-term> --view seeds"
    );

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
    if is_prime_seed_search_request(request)
        && generation_hit.request_fingerprint.as_deref()?
            != prompt_output_request_fingerprint(generation_hit, request)
    {
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
            load_syntax_query_rows_replay(
                cache_root,
                &generation_hit.language_id,
                &generation_hit.provider_id,
                &generation_hit.project_root,
                request,
            )
        })
        .or_else(|| {
            if is_tree_sitter_query_request(request) || has_structured_evidence_artifact {
                None
            } else {
                load_prompt_output_artifact(cache_root, generation_hit, request)
            }
        })
}

fn prompt_output_artifact_replay_safe(stdout: &str) -> bool {
    if stdout.starts_with("[search-prime]") && !stdout.contains("|decision purpose=decision-primer")
    {
        return false;
    }
    !stdout.contains("alias: graph:{")
        && !stdout
            .contains("legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next")
}

fn is_prime_seed_search_request(request: &ClientRequest) -> bool {
    request.method == ClientMethod::Search
        && request
            .forwarded_args
            .first()
            .is_some_and(|arg| arg == "prime")
        && (request
            .forwarded_args
            .windows(2)
            .any(|window| window[0] == "--view" && window[1] == "seeds")
            || request
                .forwarded_args
                .iter()
                .any(|arg| arg == "--view=seeds"))
}

pub(crate) fn replay_artifact_path(
    cache_root: &Path,
    artifact_id: &CacheArtifactId,
    allowed_prefix: &str,
    allowed_suffix: &str,
) -> Option<PathBuf> {
    let artifact_id = artifact_id.as_str();
    if !artifact_id.starts_with(allowed_prefix) || !artifact_id.ends_with(allowed_suffix) {
        return None;
    }
    let relative = Path::new(artifact_id);
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(replay_artifacts_root(cache_root)?.join(relative))
}

fn replay_artifacts_root(cache_root: &Path) -> Option<PathBuf> {
    let live_dir = cache_root.parent()?;
    if cache_root.file_name().and_then(|name| name.to_str()) == Some("client")
        && live_dir.file_name().and_then(|name| name.to_str()) == Some("live")
    {
        return live_dir
            .parent()
            .map(|workspace_dir| workspace_dir.join("artifacts"));
    }
    None
}

pub(crate) fn structured_evidence_artifact_path(
    cache_root: &Path,
    artifact_id: &CacheArtifactId,
) -> Option<PathBuf> {
    [
        ("relation-plan/", ".json"),
        ("flow-lite/", ".json"),
        ("codeql-evidence/", ".json"),
    ]
    .into_iter()
    .find_map(|(prefix, suffix)| replay_artifact_path(cache_root, artifact_id, prefix, suffix))
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

pub(crate) fn load_syntax_query_rows_replay(
    cache_root: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    project_root: &Path,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    let lookup = syntax_query_rows_lookup(
        &ClientDbEngine::sqlite_path_for_client_dir(cache_root),
        language_id,
        provider_id,
        project_root,
        request,
    )?;
    render_syntax_query_rows_replay(
        ClientDb::lookup_syntax_query_replay(&lookup)
            .ok()
            .flatten()?,
        project_root,
    )
}

pub(crate) fn load_syntax_query_rows_replay_open(
    db: &ClientDb,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    project_root: &Path,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    let lookup =
        syntax_query_rows_lookup(db.path(), language_id, provider_id, project_root, request)?;
    render_syntax_query_rows_replay(
        db.lookup_syntax_query_replay_open(&lookup).ok().flatten()?,
        project_root,
    )
}

fn syntax_query_rows_lookup(
    db_path: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    project_root: &Path,
    request: &ClientRequest,
) -> Option<ClientDbSyntaxQueryLookup> {
    if request.method != ClientMethod::Query
        || request.forwarded_args.iter().any(|arg| arg == "--code")
    {
        return None;
    }
    let query_ast_fingerprint = request_tree_sitter_query_ast_fingerprint(&request.forwarded_args)?;
    Some(ClientDbSyntaxQueryLookup {
        db_path: db_path.to_path_buf(),
        language_id: language_id.clone(),
        provider_id: provider_id.clone(),
        project_root: project_root.to_path_buf(),
        query_ast_fingerprint,
        selector: request_flag_value(&request.forwarded_args, "--selector").map(str::to_string),
    })
}

fn render_syntax_query_rows_replay(
    replay: agent_semantic_client_db::ClientDbSyntaxQueryReplay,
    project_root: &Path,
) -> Option<ProviderCacheReplay> {
    if !replay_file_hashes_match(project_root, &replay.file_hashes) {
        return None;
    }
    let stdout = render_semantic_tree_sitter_query_rows_stdout(&replay);
    Some(ProviderCacheReplay::syntax_rows(
        stdout.into_bytes(),
        replay.artifact_id,
        replay.packet_bytes,
    ))
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
    if request.forwarded_args.iter().any(|arg| arg == "--code") {
        return None;
    }
    if string_field(packet, "schemaId")? != "agent.semantic-protocols.semantic-query-packet" {
        return None;
    }
    if string_field(packet, "method")? != "query/owner-items" {
        return None;
    }
    if string_field(packet, "ownerPath")? != request_owner_path(&request.forwarded_args)? {
        return None;
    }
    if string_field(packet, "query")? != request_query_value(&request.forwarded_args)? {
        return None;
    }
    Some(())
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

fn request_owner_path(forwarded_args: &[String]) -> Option<&str> {
    forwarded_args
        .iter()
        .find(|arg| !arg.starts_with('-') && arg.as_str() != ".")
        .map(String::as_str)
}

fn request_query_value(forwarded_args: &[String]) -> Option<&str> {
    forwarded_args
        .windows(2)
        .find(|window| window[0] == "--query" || window[0] == "--term")
        .map(|window| window[1].as_str())
}

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

fn render_query_packet_stdout(packet: &Value) -> Option<String> {
    if string_field(packet, "schemaId")? != "agent.semantic-protocols.semantic-query-packet" {
        return None;
    }
    if string_field(packet, "method")? != "query/owner-items" {
        return None;
    }

    let query = string_field(packet, "query")?;
    let owner_path = string_field(packet, "ownerPath").unwrap_or(".");
    let package = string_field(packet, "packageName").unwrap_or(".");
    let output_mode = string_field(packet, "outputMode").unwrap_or("code");
    if output_mode == "code" {
        return None;
    }
    let match_mode = string_field(packet, "matchMode").unwrap_or("unknown");
    let matches = packet.get("matches")?.as_array()?;
    let status = query_status(packet, matches);
    let next = query_next_action(output_mode, status);

    let mut output = String::new();
    output.push_str(&format!(
        "[search-owner] q={owner_path} pkg={package} own=1 item={} itemQuery={query}\n",
        matches.len()
    ));
    output.push_str(&format!(
        "|owner {owner_path} role=source source=parser-visible-module\n"
    ));
    output.push_str(&format!(
        "|query itemQuery={query} status={status} match={match_mode} item={} reason=cache-query-packet output={output_mode} next={next}\n",
        matches.len()
    ));

    for item in matches {
        let name = string_field(item, "name")?;
        let kind = string_field(item, "kind")?;
        let read = match_read_locator(item)?;
        output.push_str(&format!(
            "|item {name} kind={kind} next=symbol:{name} read={read}\n"
        ));
        if output_mode == "code"
            && let Some(code) = string_field(item, "code")
        {
            let location = item.get("location")?;
            let path = string_field(location, "path")?;
            let line_range = string_field(location, "lineRange")?;
            let nodes = compact_projection_nodes(item);
            let text = serde_json::to_string(code).ok()?;
            let truncated = bool_field(item, "truncated").unwrap_or(false);
            output.push_str(&format!(
                "|code path={path} lineRange={line_range} reason=query-packet truncated={truncated} nodes={nodes} text={text}\n"
            ));
        }
    }
    Some(output)
}

fn query_status<'a>(packet: &'a Value, matches: &[Value]) -> &'a str {
    packet
        .get("queryCoverage")
        .and_then(Value::as_array)
        .and_then(|coverage| coverage.first())
        .and_then(|entry| string_field(entry, "status"))
        .unwrap_or(if matches.is_empty() { "miss" } else { "hit" })
}

fn query_next_action(output_mode: &str, status: &str) -> &'static str {
    if status == "miss" {
        "revise-query"
    } else if output_mode == "code" {
        "code"
    } else {
        "select-item"
    }
}

fn match_read_locator(item: &Value) -> Option<String> {
    if let Some(read) = string_field(item, "read") {
        return Some(read.to_string());
    }
    let location = item.get("location")?;
    let path = string_field(location, "path")?;
    let line_range = string_field(location, "lineRange")?;
    Some(format!("{path}:{line_range}"))
}

fn compact_projection_nodes(item: &Value) -> String {
    item.get("projection")
        .and_then(|projection| projection.get("nodes"))
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|node| {
                    let id = string_field(node, "id")?;
                    let kind = string_field(node, "kind").unwrap_or("node");
                    let role = string_field(node, "role").unwrap_or("semantic");
                    Some(format!("{id}:{kind}:{role}"))
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|nodes| !nodes.is_empty())
        .unwrap_or_else(|| "-".to_string())
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn bool_field(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}
