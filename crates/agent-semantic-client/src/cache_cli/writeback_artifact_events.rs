//! Rust SQLite artifact-event rows for provider write-back.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, ProviderCommandReceipt, ResolvedProvider,
};
use agent_semantic_client_db::ClientDbArtifactEvent;

#[derive(Clone, Copy)]
pub(super) enum ArtifactKind {
    PromptOutput,
    SearchPacket,
    QueryPacket,
    SemanticTreeSitterQuery,
    SemanticStructuralIndex,
}

pub(super) struct ArtifactEventWriteback<'a> {
    pub(super) artifact_kind: ArtifactKind,
    pub(super) artifact_id: &'a str,
    pub(super) artifact_ids: &'a [CacheArtifactId],
    pub(super) artifact_bytes: u64,
    pub(super) command_artifact_id: Option<&'a str>,
    pub(super) command_artifact_bytes: Option<u64>,
    pub(super) analysis_metadata_artifact_id: Option<&'a str>,
    pub(super) analysis_metadata_artifact_bytes: Option<u64>,
    pub(super) provider: &'a ResolvedProvider,
    pub(super) project_root: &'a Path,
    pub(super) export_method: &'a CacheExportMethod,
    pub(super) artifact_bytes_slice: &'a [u8],
    pub(super) provider_commands: &'a [ProviderCommandReceipt],
}

pub(super) fn artifact_events_for_writeback(
    input: ArtifactEventWriteback<'_>,
) -> Vec<ClientDbArtifactEvent> {
    let timestamp_ms = current_timestamp_ms();
    let mut events = vec![packet_or_prompt_artifact_event(&input, timestamp_ms)];
    for artifact_id in input.artifact_ids {
        let artifact_id = artifact_id.as_str();
        if artifact_id != input.artifact_id && Some(artifact_id) != input.command_artifact_id {
            events.push(text_artifact_event(&input, artifact_id, timestamp_ms));
        }
    }
    if let Some(command_artifact_id) = input.command_artifact_id {
        for (index, command) in input.provider_commands.iter().enumerate() {
            events.push(command_artifact_event(
                command_artifact_id,
                index.min(u32::MAX as usize) as u32,
                input.command_artifact_bytes.unwrap_or(0),
                input.project_root,
                command,
                timestamp_ms,
            ));
        }
    }
    events
}

fn text_artifact_event(
    input: &ArtifactEventWriteback<'_>,
    artifact_id: &str,
    timestamp_ms: i64,
) -> ClientDbArtifactEvent {
    ClientDbArtifactEvent {
        artifact_path: artifact_id.to_string(),
        event_ordinal: 0,
        timestamp_ms,
        kind: text_artifact_event_kind(artifact_id).to_string(),
        language: input.provider.language_id.as_str().to_string(),
        method: input.export_method.as_str().to_string(),
        target: String::new(),
        query: String::new(),
        project_root: normalized_path(input.project_root),
        project_root_arg: ".".to_string(),
        bytes: if Some(artifact_id) == input.analysis_metadata_artifact_id {
            input.analysis_metadata_artifact_bytes.unwrap_or(0)
        } else {
            0
        },
    }
}

fn packet_or_prompt_artifact_event(
    input: &ArtifactEventWriteback<'_>,
    timestamp_ms: i64,
) -> ClientDbArtifactEvent {
    let packet = serde_json::from_slice::<serde_json::Value>(input.artifact_bytes_slice).ok();
    ClientDbArtifactEvent {
        artifact_path: input.artifact_id.to_string(),
        event_ordinal: 0,
        timestamp_ms,
        kind: artifact_event_kind(input.artifact_kind).to_string(),
        language: packet
            .as_ref()
            .and_then(|value| value.get("languageId"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(input.provider.language_id.as_str())
            .to_string(),
        method: packet
            .as_ref()
            .and_then(|value| value.get("method"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(input.export_method.as_str())
            .to_string(),
        target: packet.as_ref().map_or_else(String::new, packet_target),
        query: packet
            .as_ref()
            .and_then(|value| value.get("query"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        project_root: normalized_path(input.project_root),
        project_root_arg: ".".to_string(),
        bytes: input.artifact_bytes,
    }
}

fn text_artifact_event_kind(artifact_id: &str) -> &'static str {
    if artifact_id.starts_with("search-output/") {
        "search-output"
    } else if artifact_id.starts_with("analysis-metadata/") {
        "analysis-metadata"
    } else {
        "prompt-output"
    }
}

fn command_artifact_event(
    artifact_id: &str,
    event_ordinal: u32,
    artifact_bytes: u64,
    project_root: &Path,
    command: &ProviderCommandReceipt,
    timestamp_ms: i64,
) -> ClientDbArtifactEvent {
    ClientDbArtifactEvent {
        artifact_path: artifact_id.to_string(),
        event_ordinal,
        timestamp_ms,
        kind: "command".to_string(),
        language: command.language_id.as_str().to_string(),
        method: command_method(&command.argv),
        target: command_target(&command.argv),
        query: command_query(&command.argv),
        project_root: normalized_path(project_root),
        project_root_arg: ".".to_string(),
        bytes: artifact_bytes,
    }
}

fn artifact_event_kind(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::PromptOutput => "prompt-output",
        ArtifactKind::SearchPacket => "search",
        ArtifactKind::QueryPacket => "query",
        ArtifactKind::SemanticTreeSitterQuery => "tree-sitter-query",
        ArtifactKind::SemanticStructuralIndex => "structural-index",
    }
}

fn packet_target(packet: &serde_json::Value) -> String {
    if let Some(owner) = packet.get("ownerPath").and_then(serde_json::Value::as_str) {
        return owner.to_string();
    }
    packet
        .get("owners")
        .and_then(serde_json::Value::as_array)
        .and_then(|owners| owners.first())
        .and_then(|owner| owner.get("path").or_else(|| owner.get("ownerPath")))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn command_method(argv: &[String]) -> String {
    if let Some(search_index) = argv.iter().position(|arg| arg == "search") {
        let (surface, _) = command_surface(argv, search_index + 1);
        return format!("search/{}", surface.unwrap_or("unknown"));
    }
    if argv.iter().any(|arg| arg == "query") {
        if command_is_direct_source_read(argv) {
            return "query/direct-source-read".to_string();
        }
        if command_is_tree_sitter_query(argv) {
            return "query/tree-sitter".to_string();
        }
        if command_is_selector_code_query(argv) {
            return "query/code".to_string();
        }
        return "query".to_string();
    }
    "command/unknown".to_string()
}

fn command_target(argv: &[String]) -> String {
    if let Some(search_index) = argv.iter().position(|arg| arg == "search") {
        let (_, surface_index) = command_surface(argv, search_index + 1);
        return next_positional(argv, surface_index + 1)
            .unwrap_or("")
            .to_string();
    }
    if let Some(query_index) = argv.iter().position(|arg| arg == "query") {
        return next_positional(argv, query_index + 1)
            .unwrap_or("")
            .to_string();
    }
    String::new()
}

fn command_query(argv: &[String]) -> String {
    if command_is_tree_sitter_query(argv) {
        return option_value(argv, "--treesitter-query")
            .or_else(|| option_value(argv, "--tree-sitter-query"))
            .or_else(|| option_value(argv, "--query-catalog"))
            .unwrap_or("")
            .to_string();
    }
    let Some(search_index) = argv.iter().position(|arg| arg == "search") else {
        return String::new();
    };
    let (surface, surface_index) = command_surface(argv, search_index + 1);
    if surface == Some("lexical") {
        next_positional(argv, surface_index + 1)
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    }
}

fn command_is_direct_source_read(argv: &[String]) -> bool {
    argv.iter().any(|arg| arg == "--from-hook")
        && argv.iter().any(|arg| arg == "direct-source-read")
}

fn command_is_tree_sitter_query(argv: &[String]) -> bool {
    option_value(argv, "--treesitter-query")
        .or_else(|| option_value(argv, "--tree-sitter-query"))
        .or_else(|| option_value(argv, "--query-catalog"))
        .is_some()
}

fn command_is_selector_code_query(argv: &[String]) -> bool {
    option_value(argv, "--selector").is_some() && argv.iter().any(|arg| arg == "--code")
}

fn command_surface(argv: &[String], start: usize) -> (Option<&str>, usize) {
    let mut index = start;
    while index < argv.len() {
        let item = argv[index].as_str();
        if item == "--" {
            index += 1;
        } else if item.starts_with('-') {
            index += if command_option_has_value(item) { 2 } else { 1 };
        } else {
            return (Some(item), index);
        }
    }
    (None, argv.len())
}

fn next_positional(argv: &[String], start: usize) -> Option<&str> {
    argv.iter()
        .skip(start)
        .find(|item| !item.starts_with('-'))
        .map(String::as_str)
}

fn command_option_has_value(option: &str) -> bool {
    matches!(
        option,
        "--dependency"
            | "--from-hook"
            | "--format"
            | "--owner"
            | "--package"
            | "--query"
            | "--query-set"
            | "--query-catalog"
            | "--seeds"
            | "--selector"
            | "--tree-sitter-query"
            | "--treesitter-query"
            | "--view"
    )
}

fn option_value<'a>(argv: &'a [String], option: &str) -> Option<&'a str> {
    argv.windows(2).find_map(|window| {
        if window[0] == option {
            Some(window[1].as_str())
        } else {
            None
        }
    })
}

fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn normalized_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}
