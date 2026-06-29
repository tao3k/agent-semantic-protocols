//! Analyzer-only history metadata sidecars for cache write-back.

use std::fs;
use std::path::Path;

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, ClientCacheGeneration, ClientRequest,
    ProviderCommandReceipt, ResolvedProvider, SemanticSchemaId,
};

use super::writeback_artifact_events::ArtifactKind;
use crate::cache_replay::replay_artifact_path;

const CLIENT_HISTORY_ANALYSIS_METADATA_SCHEMA_ID: &str =
    "agent.semantic-protocols.client-history-analysis-metadata";

pub(super) struct AnalysisMetadataArtifactWriteback<'a> {
    pub(super) cache_root: &'a Path,
    pub(super) generation: &'a mut ClientCacheGeneration,
    pub(super) source_artifact_id: &'a CacheArtifactId,
    pub(super) source_artifact_kind: ArtifactKind,
    pub(super) provider: &'a ResolvedProvider,
    pub(super) project_root: &'a Path,
    pub(super) request: &'a ClientRequest,
    pub(super) export_method: &'a CacheExportMethod,
    pub(super) artifact_bytes: &'a [u8],
    pub(super) rendered_stdout: &'a [u8],
    pub(super) provider_commands: &'a [ProviderCommandReceipt],
    pub(super) writeback_provider_commands: &'a [ProviderCommandReceipt],
}

pub(super) fn maybe_write_analysis_metadata_artifact(
    writeback: AnalysisMetadataArtifactWriteback<'_>,
) -> Option<(CacheArtifactId, u64)> {
    let artifact_id = CacheArtifactId::from(format!(
        "analysis-metadata/{}.json",
        writeback.generation.generation_id.as_str()
    ));
    let artifact_path = replay_artifact_path(
        writeback.cache_root,
        &artifact_id,
        "analysis-metadata/",
        ".json",
    )?;
    let request_method = serde_json::to_value(&writeback.request.method).ok()?;
    let request_language_id = writeback.request.language_id.as_ref().map_or_else(
        || writeback.provider.language_id.as_str(),
        |language_id| language_id.as_str(),
    );
    let query = analysis_metadata_query(&writeback.request.forwarded_args);
    let target = analysis_metadata_target(&writeback.request.forwarded_args);
    let metadata = serde_json::json!({
        "schemaId": CLIENT_HISTORY_ANALYSIS_METADATA_SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "sourceArtifactId": writeback.source_artifact_id.as_str(),
        "sourceArtifactKind": analysis_metadata_source_kind(writeback.source_artifact_kind),
        "languageId": writeback.provider.language_id.as_str(),
        "providerId": writeback.provider.provider_id.as_str(),
        "projectRoot": normalized_path(writeback.project_root),
        "method": writeback.export_method.as_str(),
        "exportMethod": writeback.export_method.as_str(),
        "query": query,
        "target": target,
        "developerMode": {
            "defaultEnabled": true,
            "storageOnly": true,
        },
        "agentFacingOutput": {
            "unchanged": true,
            "metadataSurface": "history-analysis",
        },
        "request": {
            "method": request_method,
            "languageId": request_language_id,
            "forwardedArgs": writeback.request.forwarded_args.clone(),
        },
        "artifact": {
            "bytes": saturating_len(writeback.artifact_bytes),
            "fnv64": stable_hash_bytes(writeback.artifact_bytes),
        },
        "output": {
            "bytes": saturating_len(writeback.rendered_stdout),
            "lineCount": analysis_metadata_line_count(writeback.rendered_stdout),
            "fnv64": stable_hash_bytes(writeback.rendered_stdout),
        },
        "analysis": analysis_metadata_from_output(writeback.rendered_stdout),
        "commands": {
            "captured": writeback.provider_commands,
            "writeback": writeback.writeback_provider_commands,
        },
    });
    let bytes = serde_json::to_vec_pretty(&metadata).ok()?;
    fs::create_dir_all(artifact_path.parent()?).ok()?;
    fs::write(&artifact_path, &bytes).ok()?;
    writeback
        .generation
        .artifact_ids
        .get_or_insert_with(Vec::new)
        .push(artifact_id.clone());
    if !writeback
        .generation
        .schema_ids
        .iter()
        .any(|schema_id| schema_id.as_str() == CLIENT_HISTORY_ANALYSIS_METADATA_SCHEMA_ID)
    {
        writeback.generation.schema_ids.push(SemanticSchemaId::from(
            CLIENT_HISTORY_ANALYSIS_METADATA_SCHEMA_ID,
        ));
    }
    Some((artifact_id, saturating_len(&bytes)))
}

fn analysis_metadata_source_kind(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::PromptOutput => "prompt-output",
        ArtifactKind::SearchPacket => "search",
        ArtifactKind::QueryPacket => "query",
        ArtifactKind::SemanticTreeSitterQuery => "tree-sitter-query",
        ArtifactKind::SemanticStructuralIndex => "structural-index",
    }
}

fn analysis_metadata_from_output(stdout: &[u8]) -> serde_json::Value {
    let mut field_lines = serde_json::Map::new();
    let recognized_line_count = std::str::from_utf8(stdout).map_or(0, |text| {
        text.lines()
            .filter(|line| capture_analysis_line(line, &mut field_lines))
            .count()
            .min(u64::MAX as usize) as u64
    });
    serde_json::json!({
        "recognizedLineCount": recognized_line_count,
        "fieldLines": field_lines,
    })
}

fn capture_analysis_line(
    line: &str,
    field_lines: &mut serde_json::Map<String, serde_json::Value>,
) -> bool {
    let before = field_lines.len();
    if line.starts_with("[search-") {
        insert_analysis_field(field_lines, "header", line);
    }
    ANALYSIS_LINE_PREFIXES.iter().for_each(|(key, prefix)| {
        if let Some(value) = line.strip_prefix(prefix) {
            insert_analysis_field(field_lines, key, value);
        }
    });
    [
        ("queryQuality", "quality="),
        ("risk", "risk="),
        ("decisionNext", "next="),
    ]
    .into_iter()
    .for_each(|(key, token)| {
        if let Some(value) = analysis_line_token(line, token) {
            insert_analysis_field(field_lines, key, value);
        }
    });
    if line.starts_with("|decision ") {
        insert_analysis_field(field_lines, "decision", line);
    }
    field_lines.len() != before
}

const ANALYSIS_LINE_PREFIXES: &[(&str, &str)] = &[
    ("queryPack", "queryPack="),
    ("queryClauses", "queryClauses="),
    ("terms", "terms="),
    ("scopeQuality", "scopeQuality="),
    ("clauseCoverage", "clauseCoverage="),
    ("packageCohesion", "packageCohesion="),
    ("sourceTrace", "sourceTrace="),
    ("ownerCandidates", "ownerCandidates="),
    ("rankedEvidence", "rankedEvidence="),
    ("evidenceFrontier", "evidenceFrontier="),
    ("commandHandles", "commandHandles="),
    ("actionRank", "actionRank="),
    ("actionFrontier", "actionFrontier="),
    ("recommendedNext", "recommendedNext="),
    ("nextCommand", "nextCommand="),
    ("reason", "reason="),
    ("avoid", "avoid="),
    ("risk", "risk="),
    ("rank", "rank="),
];

fn insert_analysis_field(
    fields: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: &str,
) {
    fields
        .entry(key.to_string())
        .or_insert_with(|| serde_json::Value::String(value.to_string()));
}

fn analysis_line_token<'a>(line: &'a str, token: &str) -> Option<&'a str> {
    let token_start = line.find(token)?;
    let value = &line[token_start + token.len()..];
    if let Some(rest) = value.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some(&rest[..end]);
    }
    let end = value.find(char::is_whitespace).unwrap_or(value.len());
    Some(&value[..end])
}

fn analysis_metadata_line_count(stdout: &[u8]) -> u64 {
    std::str::from_utf8(stdout)
        .map(|text| text.lines().count().min(u64::MAX as usize) as u64)
        .unwrap_or(0)
}

fn analysis_metadata_query(args: &[String]) -> String {
    option_value_from_args(args, "--query")
        .or_else(|| option_value_from_args(args, "--term"))
        .unwrap_or("")
        .to_string()
}

fn analysis_metadata_target(args: &[String]) -> String {
    if let Some(selector) = option_value_from_args(args, "--selector") {
        return selector.to_string();
    }
    match args.first().map(String::as_str) {
        Some("owner") => args.get(1).cloned().unwrap_or_default(),
        Some("lexical" | "pipe" | "fd" | "rg") => args.get(1).cloned().unwrap_or_default(),
        Some("deps") => option_value_from_args(args, "--dependency")
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

fn option_value_from_args<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let prefix = format!("{name}=");
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        if arg == name {
            return args.get(index + 1).map(String::as_str);
        }
        if let Some(value) = arg.strip_prefix(&prefix) {
            return Some(value);
        }
        index += 1;
    }
    None
}

fn saturating_len(bytes: &[u8]) -> u64 {
    bytes.len().min(u64::MAX as usize) as u64
}

fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    bytes.iter().for_each(|byte| {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    });
    format!("{hash:016x}")
}

fn normalized_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}
