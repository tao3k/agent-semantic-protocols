//! Replay provider cache artifacts into compact prompt stdout.

use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use agent_semantic_client_core::{ByteCount, CacheArtifactId, ClientMethod, ClientRequest};
use agent_semantic_client_db::{
    ClientDb, ClientDbGenerationHit, ClientDbSyntaxQueryLookup, ClientDbSyntaxQueryReplay,
};
use serde_json::Value;

const SEMANTIC_AGENT_PROTOCOL_BIN_ENV: &str = "SEMANTIC_AGENT_PROTOCOL_BIN";
const SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-tree-sitter-query";
pub(crate) const MAX_CACHE_REPLAY_ARTIFACT_BYTES: u64 = 1_048_576;

pub(crate) struct ProviderCacheReplay {
    pub(crate) stdout: Vec<u8>,
    pub(crate) syntax_artifact_id: Option<CacheArtifactId>,
    pub(crate) packet_bytes: Option<ByteCount>,
}

impl ProviderCacheReplay {
    fn stdout(stdout: Vec<u8>) -> Self {
        Self {
            stdout,
            syntax_artifact_id: None,
            packet_bytes: None,
        }
    }

    fn syntax_packet(
        stdout: Vec<u8>,
        syntax_artifact_id: CacheArtifactId,
        packet_bytes: usize,
    ) -> Self {
        Self {
            stdout,
            syntax_artifact_id: Some(syntax_artifact_id),
            packet_bytes: Some(ByteCount::from_len(packet_bytes)),
        }
    }

    fn syntax_rows(
        stdout: Vec<u8>,
        syntax_artifact_id: Option<CacheArtifactId>,
        packet_bytes: Option<u64>,
    ) -> Self {
        Self {
            stdout,
            syntax_artifact_id,
            packet_bytes: packet_bytes.map(ByteCount::new),
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
        let seed = format!(
            "{}\0{}\0{}\0{}\0{}",
            generation_hit.language_id,
            generation_hit.provider_id,
            normalized_path(&generation_hit.project_root),
            generation_hit.export_method,
            request.forwarded_args.join("\0")
        );
        format!("fnv64:{}", stable_hash_hex(&seed))
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
            std::str::from_utf8(&stdout).ok()?;
            return Some(ProviderCacheReplay::stdout(stdout));
        }
        None
    }

    load_search_packet_artifact(cache_root, generation_hit, request)
        .or_else(|| load_query_packet_artifact(cache_root, generation_hit, request))
        .or_else(|| load_syntax_query_packet_artifact(cache_root, generation_hit, request))
        .or_else(|| load_syntax_query_rows(cache_root, generation_hit, request))
        .or_else(|| load_prompt_output_artifact(cache_root, generation_hit, request))
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
    Some(cache_root.parent()?.join("artifacts").join(relative))
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
        .find_map(|artifact_id| render_search_packet_artifact(cache_root, artifact_id))
}

fn render_search_packet_artifact(
    cache_root: &Path,
    artifact_id: &CacheArtifactId,
) -> Option<ProviderCacheReplay> {
    let artifact_path = replay_artifact_path(cache_root, artifact_id, "search/", ".json")?;
    let metadata = fs::metadata(&artifact_path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_CACHE_REPLAY_ARTIFACT_BYTES {
        return None;
    }
    let output = Command::new(protocol_graph_renderer_binary())
        .args(["graph", "render", "--packet"])
        .arg(&artifact_path)
        .args(["--view", "seeds"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(ProviderCacheReplay::stdout(output.stdout))
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

fn load_syntax_query_rows(
    cache_root: &Path,
    generation_hit: &ClientDbGenerationHit,
    request: &ClientRequest,
) -> Option<ProviderCacheReplay> {
    if request.method != ClientMethod::Query
        || request.forwarded_args.iter().any(|arg| arg == "--code")
    {
        return None;
    }
    let request_fingerprint = generation_hit.request_fingerprint.as_ref()?.clone();
    let replay = ClientDb::lookup_syntax_query_replay(&ClientDbSyntaxQueryLookup {
        db_path: ClientDb::default_path(cache_root),
        language_id: generation_hit.language_id.clone(),
        provider_id: generation_hit.provider_id.clone(),
        project_root: generation_hit.project_root.clone(),
        request_fingerprint,
    })
    .ok()
    .flatten()?;
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

pub(crate) fn query_packet_matches_request(packet: &Value, request: &ClientRequest) -> Option<()> {
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
    if string_field(query, "input")? != request_tree_sitter_query_value(&request.forwarded_args)? {
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

pub(crate) fn render_semantic_tree_sitter_query_stdout(packet: &Value) -> Option<String> {
    if string_field(packet, "schemaId")? != SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID {
        return None;
    }
    let matches = packet.get("matches")?.as_array()?;
    if matches.is_empty() {
        return render_syntax_query_miss_stdout(packet);
    }

    let mut output = String::new();
    for item in matches {
        let match_locator = item.get("range").and_then(syntax_range_locator);
        let captures = item.get("captures")?.as_array()?;
        for capture in captures {
            let Some(text) = syntax_capture_text(capture).or_else(|| syntax_capture_text(item))
            else {
                continue;
            };
            let locator = match_locator
                .clone()
                .or_else(|| capture.get("range").and_then(syntax_range_locator))?;
            output.push_str(&locator);
            output.push('\n');
            output.push_str(text);
            output.push('\n');
        }
    }
    if output.is_empty() {
        render_syntax_query_miss_stdout(packet)
    } else {
        Some(output)
    }
}

pub(crate) fn render_semantic_tree_sitter_query_rows_stdout(
    replay: &ClientDbSyntaxQueryReplay,
) -> String {
    if replay.rows.is_empty() {
        return render_syntax_query_miss_line(
            &replay.input_form,
            replay.input_kind.as_str(),
            &replay.grammar_id,
            &replay.grammar_profile_version,
            &replay.captures,
        );
    }
    let mut output = String::new();
    for row in &replay.rows {
        output.push_str(&row.locator);
        output.push('\n');
        output.push_str(&row.text);
        output.push('\n');
    }
    output
}

fn render_syntax_query_miss_stdout(packet: &Value) -> Option<String> {
    let query = packet.get("query")?;
    let input_form = string_field(query, "inputForm").unwrap_or("s-expression");
    let input = if string_field(query, "catalogId").is_some() {
        "catalog"
    } else {
        "inline"
    };
    let grammar = string_field(packet, "grammarId").unwrap_or("unknown");
    let grammar_profile = string_field(packet, "grammarProfileVersion").unwrap_or("unknown");
    let captures = query
        .get("fields")
        .and_then(|fields| fields.get("captures"))
        .and_then(Value::as_array)
        .map(|captures| {
            captures
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(render_syntax_query_miss_line(
        input_form,
        input,
        grammar,
        grammar_profile,
        &captures,
    ))
}

fn render_syntax_query_miss_line(
    input_form: &str,
    input: &str,
    grammar: &str,
    grammar_profile: &str,
    captures: &[String],
) -> String {
    let captures_display = captures.join(",");
    let capture_count = if captures_display.is_empty() {
        0
    } else {
        captures.len()
    };
    format!(
        "|syntax-query inputForm={input_form} input={input} grammar={grammar} grammarProfile={grammar_profile} dialect=tree-sitter-query matchStatus=miss match=0 rows=0 truncated=false captureCount={capture_count} captures={captures_display}\n"
    )
}

fn syntax_capture_text(value: &Value) -> Option<&str> {
    value
        .get("fields")
        .and_then(|fields| string_field(fields, "symbol").or_else(|| string_field(fields, "name")))
        .or_else(|| string_field(value, "text"))
        .or_else(|| string_field(value, "name"))
}

fn syntax_range_locator(range: &Value) -> Option<String> {
    let path = string_field(range, "path")?;
    let line_range = range.get("lineRange")?;
    let (start, end) = syntax_line_range_bounds(line_range)?;
    if start == end {
        Some(format!("{path}:{start}"))
    } else {
        Some(format!("{path}:{start}:{end}"))
    }
}

fn syntax_line_range_bounds(line_range: &Value) -> Option<(String, String)> {
    if let Some(line_range) = line_range.as_str() {
        let (start, end) = line_range.split_once(':')?;
        return Some((start.to_string(), end.to_string()));
    }
    let start = line_range.get("start").and_then(Value::as_u64)?;
    let end = line_range.get("end").and_then(Value::as_u64)?;
    Some((start.to_string(), end.to_string()))
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

fn protocol_graph_renderer_binary() -> PathBuf {
    env::var_os(SEMANTIC_AGENT_PROTOCOL_BIN_ENV)
        .map(PathBuf::from)
        .or_else(|| env::current_exe().ok())
        .unwrap_or_else(|| PathBuf::from("asp"))
}
