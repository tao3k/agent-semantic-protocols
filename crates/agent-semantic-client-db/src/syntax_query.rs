//! Normalized syntax-query row import and replay helpers for the client DB.

use agent_semantic_client_core::{
    CacheArtifactId, ClientCacheGeneration, compile_query_abi_source,
    syntax_query_ast_abi_fingerprint,
};
use serde_json::Value;

use crate::types::{
    ClientDbSyntaxCaptureReplay, ClientDbSyntaxNodeType, ClientDbSyntaxQueryInputKind,
    ClientDbSyntaxQueryReplay,
};

pub(crate) const SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-tree-sitter-query";

pub(crate) struct ParsedSyntaxQueryPacketImport<'a> {
    artifact_ids: &'a [CacheArtifactId],
    grammar_id: String,
    grammar_profile_version: String,
    input_form: String,
    input_kind: &'static str,
    compiled_source: String,
    query_ast_fingerprint: String,
    item_node_type: String,
    capture_node_type: String,
    query_field: Option<String>,
    selector: Option<String>,
    captures_json: String,
    matches: Vec<Value>,
    packet_bytes: usize,
}

pub(crate) fn parse_syntax_query_packet_import<'a>(
    generation: &'a ClientCacheGeneration,
    packet_bytes: &'a [u8],
) -> Result<ParsedSyntaxQueryPacketImport<'a>, String> {
    if generation.raw_source_stored {
        return Err("syntax query rows refuse rawSourceStored=true generation".to_string());
    }
    let packet: Value = serde_json::from_slice(packet_bytes)
        .map_err(|error| format!("failed to parse semantic tree-sitter query packet: {error}"))?;
    validate_syntax_query_packet_for_rows(&packet)?;

    let query = packet
        .get("query")
        .ok_or_else(|| "syntax query packet is missing query".to_string())?;
    let captures = query
        .get("fields")
        .and_then(|fields| fields.get("captures"))
        .and_then(Value::as_array)
        .map(|captures| {
            captures
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let matches = packet
        .get("matches")
        .and_then(Value::as_array)
        .ok_or_else(|| "syntax query packet is missing matches".to_string())?;
    let query_input = optional_string_field(query, "input").unwrap_or("");
    let compiled_source = optional_string_field(query, "compiledSource")
        .unwrap_or(query_input)
        .to_string();
    let query_plan = compile_query_abi_source(&compiled_source)
        .map_err(|error| format!("syntax query rows require AST/ABI plan: {}", error.message))?;
    let query_ast_fingerprint = syntax_query_ast_abi_fingerprint(&compiled_source)
        .map_err(|error| format!("syntax query rows require AST/ABI fingerprint: {error}"))?;
    let item_node_type = query_plan
        .node_types
        .first()
        .cloned()
        .ok_or_else(|| "syntax query rows require an AST/ABI item node type".to_string())?;
    let capture_node_type = query_plan
        .node_types
        .last()
        .cloned()
        .ok_or_else(|| "syntax query rows require an AST/ABI capture node type".to_string())?;
    Ok(ParsedSyntaxQueryPacketImport {
        artifact_ids: generation.artifact_ids.as_deref().unwrap_or(&[]),
        grammar_id: string_field(&packet, "grammarId")?.to_string(),
        grammar_profile_version: string_field(&packet, "grammarProfileVersion")?.to_string(),
        input_form: string_field(query, "inputForm")?.to_string(),
        input_kind: if query.get("catalogId").is_some() {
            "catalog"
        } else {
            "inline"
        },
        query_ast_fingerprint,
        item_node_type,
        capture_node_type,
        query_field: query_plan.fields.first().cloned(),
        compiled_source,
        selector: query
            .get("fields")
            .and_then(|fields| optional_string_field(fields, "selector"))
            .map(str::to_string),
        captures_json: serde_json::to_string(&captures)
            .map_err(|error| format!("failed to serialize syntax captures: {error}"))?,
        matches: matches.clone(),
        packet_bytes: packet_bytes.len(),
    })
}

fn validate_syntax_query_packet_for_rows(packet: &Value) -> Result<(), String> {
    if string_field(packet, "schemaId")? != SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID {
        return Err("syntax query rows require semantic-tree-sitter-query packet".to_string());
    }
    if packet
        .pointer("/cache/rawSourceStored")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err("syntax query rows refuse packet rawSourceStored=true".to_string());
    }
    if packet
        .pointer("/query/fields/codeOutput")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err("syntax query rows do not store --code packet output".to_string());
    }
    Ok(())
}

pub(crate) fn syntax_query_replay_and_selector_from_packet_import(
    generation: &ClientCacheGeneration,
    packet_bytes: &[u8],
) -> Result<(ClientDbSyntaxQueryReplay, Option<String>), String> {
    let parsed = parse_syntax_query_packet_import(generation, packet_bytes)?;
    let captures = serde_json::from_str(&parsed.captures_json)
        .map_err(|error| format!("failed to parse syntax capture names: {error}"))?;
    let rows = syntax_query_replay_rows_from_parsed(&parsed)?;
    let selector = parsed.selector.clone();
    Ok((
        ClientDbSyntaxQueryReplay {
            generation_id: generation.generation_id.clone(),
            language_id: generation.language_id.clone(),
            grammar_id: parsed.grammar_id,
            grammar_profile_version: parsed.grammar_profile_version,
            input_form: parsed.input_form,
            input_kind: ClientDbSyntaxQueryInputKind::from_wire(parsed.input_kind),
            compiled_source: parsed.compiled_source,
            captures,
            query_ast_fingerprint: parsed.query_ast_fingerprint,
            artifact_id: parsed.artifact_ids.first().cloned(),
            packet_bytes: Some(parsed.packet_bytes.min(u64::MAX as usize) as u64),
            file_hashes: generation.file_hashes.as_deref().unwrap_or(&[]).to_vec(),
            rows,
        },
        selector,
    ))
}

fn syntax_query_replay_rows_from_parsed(
    parsed: &ParsedSyntaxQueryPacketImport<'_>,
) -> Result<Vec<ClientDbSyntaxCaptureReplay>, String> {
    let mut rows = Vec::new();
    for item in &parsed.matches {
        let captures = item
            .get("captures")
            .and_then(Value::as_array)
            .ok_or_else(|| "syntax match is missing captures".to_string())?;
        let first_capture_range = captures
            .iter()
            .find_map(|capture| capture.get("range").and_then(parse_syntax_range));
        let match_range = item
            .get("range")
            .and_then(parse_syntax_range)
            .or(first_capture_range)
            .ok_or_else(|| "syntax match is missing a replayable range".to_string())?;
        for capture in captures {
            let Some(text) =
                safe_syntax_capture_text(capture).or_else(|| safe_syntax_capture_text(item))
            else {
                continue;
            };
            let capture_range = capture
                .get("range")
                .and_then(parse_syntax_range)
                .or_else(|| item.get("range").and_then(parse_syntax_range))
                .ok_or_else(|| "syntax capture is missing a replayable range".to_string())?;
            rows.push(ClientDbSyntaxCaptureReplay {
                match_locator: compact_source_locator(&match_range.0, match_range.1, match_range.2),
                capture_locator: compact_source_locator(
                    &capture_range.0,
                    capture_range.1,
                    capture_range.2,
                ),
                capture_name: optional_string_field(capture, "name")
                    .unwrap_or("capture")
                    .to_string(),
                capture_node_type: ClientDbSyntaxNodeType::from(parsed.capture_node_type.clone()),
                item_node_type: ClientDbSyntaxNodeType::from(
                    syntax_item_node_type(item, capture)
                        .unwrap_or(&parsed.item_node_type)
                        .to_string(),
                ),
                field: optional_string_field(capture, "field")
                    .or(parsed.query_field.as_deref())
                    .map(ToOwned::to_owned),
                text: text.to_string(),
            });
        }
    }
    Ok(rows)
}

fn parse_syntax_range(range: &Value) -> Option<(String, i64, i64)> {
    let path = optional_string_field(range, "path")?.to_string();
    let line_range = range.get("lineRange")?;
    let (start, end) = if let Some(line_range) = line_range.as_str() {
        let (start, end) = line_range.split_once(':')?;
        (start.parse::<i64>().ok()?, end.parse::<i64>().ok()?)
    } else {
        (
            line_range.get("start")?.as_i64()?,
            line_range.get("end")?.as_i64()?,
        )
    };
    Some((path, start.max(1), end.max(start).max(1)))
}

pub(crate) fn compact_source_locator(path: &str, start_line: i64, end_line: i64) -> String {
    let start_line = start_line.max(1);
    let end_line = end_line.max(start_line);
    if start_line == end_line {
        format!("{path}:{start_line}")
    } else {
        format!("{path}:{start_line}:{end_line}")
    }
}

fn safe_syntax_capture_text(value: &Value) -> Option<&str> {
    value.get("fields").and_then(|fields| {
        optional_string_field(fields, "symbol").or_else(|| optional_string_field(fields, "name"))
    })
}

fn syntax_item_node_type<'a>(item: &'a Value, capture: &'a Value) -> Option<&'a str> {
    item.get("fields")
        .and_then(|fields| optional_string_field(fields, "nodeType"))
        .or_else(|| optional_string_field(item, "nodeType"))
        .or_else(|| {
            capture
                .get("fields")
                .and_then(|fields| optional_string_field(fields, "nativeNodeType"))
        })
}

fn string_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    optional_string_field(value, field).ok_or_else(|| format!("missing string field `{field}`"))
}

fn optional_string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}
