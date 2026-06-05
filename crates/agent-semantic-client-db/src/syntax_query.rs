//! Normalized syntax-query row import and replay helpers for the client DB.

use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    CacheArtifactId, CacheGenerationId, ClientCacheFileHash, ClientCacheGeneration, LanguageId,
    ProviderId, compile_query_abi_source, syntax_query_ast_abi_fingerprint,
};
use rusqlite::params;
use serde_json::Value;

pub(crate) const SEMANTIC_TREE_SITTER_QUERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-tree-sitter-query";
pub(crate) const SYNTAX_QUERY_ROW_ABI_META_KEY: &str = "syntaxQueryRowAbiVersion";
pub(crate) const SYNTAX_QUERY_ROW_ABI_VERSION: &str =
    "syntax-query-row-abi.ast-abi-capture-item-node.lookup-v2";

/// Named lookup request for normalized syntax query replay rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSyntaxQueryLookup {
    pub db_path: PathBuf,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub project_root: PathBuf,
    pub query_ast_fingerprint: String,
    pub selector: Option<String>,
}

/// Normalized syntax query rows that can render compact locator/capture output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSyntaxQueryReplay {
    pub generation_id: CacheGenerationId,
    pub language_id: LanguageId,
    pub grammar_id: String,
    pub grammar_profile_version: String,
    pub input_form: String,
    pub input_kind: ClientDbSyntaxQueryInputKind,
    pub compiled_source: String,
    pub captures: Vec<String>,
    pub query_ast_fingerprint: String,
    pub artifact_id: Option<CacheArtifactId>,
    pub packet_bytes: Option<u64>,
    pub file_hashes: Vec<ClientCacheFileHash>,
    pub rows: Vec<ClientDbSyntaxCaptureReplay>,
}

/// Tree-sitter query input family represented by normalized syntax query rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientDbSyntaxQueryInputKind {
    Inline,
    Catalog,
}

impl ClientDbSyntaxQueryInputKind {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Catalog => "catalog",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Self {
        if value == "catalog" {
            Self::Catalog
        } else {
            Self::Inline
        }
    }
}

/// One replayable syntax capture row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSyntaxCaptureReplay {
    pub match_locator: String,
    pub capture_locator: String,
    pub capture_name: String,
    pub capture_node_type: ClientDbSyntaxNodeType,
    pub item_node_type: ClientDbSyntaxNodeType,
    pub field: Option<String>,
    pub text: String,
}

/// Typed syntax node kind observed in replayable syntax query rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSyntaxNodeType(String);

impl ClientDbSyntaxNodeType {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ClientDbSyntaxNodeType {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl PartialEq<&str> for ClientDbSyntaxNodeType {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<ClientDbSyntaxNodeType> for &str {
    fn eq(&self, other: &ClientDbSyntaxNodeType) -> bool {
        *self == other.as_str()
    }
}

pub(crate) struct ParsedSyntaxQueryPacketImport<'a> {
    pub(crate) generation: &'a ClientCacheGeneration,
    request_fingerprint: &'a str,
    project_root: String,
    grammar_id: String,
    grammar_profile_version: String,
    input_form: String,
    input_kind: &'static str,
    query_input: String,
    compiled_source: String,
    query_ast_fingerprint: String,
    item_node_type: String,
    capture_node_type: String,
    query_field: Option<String>,
    selector: Option<String>,
    captures_json: String,
    matches: Vec<Value>,
    truncated: bool,
    packet_bytes: usize,
    artifact_ids: &'a [CacheArtifactId],
}

pub(crate) fn parse_syntax_query_packet_import<'a>(
    generation: &'a ClientCacheGeneration,
    packet_bytes: &'a [u8],
) -> Result<ParsedSyntaxQueryPacketImport<'a>, String> {
    if generation.raw_source_stored {
        return Err("syntax query rows refuse rawSourceStored=true generation".to_string());
    }
    let request_fingerprint = generation
        .request_fingerprint
        .as_deref()
        .ok_or_else(|| "syntax query rows require requestFingerprint".to_string())?;
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
        generation,
        request_fingerprint,
        project_root: normalized_project_root(Path::new(&generation.project_root)),
        grammar_id: string_field(&packet, "grammarId")?.to_string(),
        grammar_profile_version: string_field(&packet, "grammarProfileVersion")?.to_string(),
        input_form: string_field(query, "inputForm")?.to_string(),
        input_kind: if query.get("catalogId").is_some() {
            "catalog"
        } else {
            "inline"
        },
        query_input: query_input.to_string(),
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
        truncated: packet
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        packet_bytes: packet_bytes.len(),
        artifact_ids: generation.artifact_ids.as_deref().unwrap_or(&[]),
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

pub(crate) fn write_syntax_query_import_rows(
    tx: &rusqlite::Transaction<'_>,
    parsed: &ParsedSyntaxQueryPacketImport<'_>,
) -> Result<(), String> {
    let generation_id = parsed.generation.generation_id.as_str();
    delete_syntax_query_rows(tx, generation_id)?;
    write_syntax_query_generation_row(tx, parsed)?;
    write_syntax_query_pattern_row(tx, parsed)?;
    write_syntax_query_artifact_ref_rows(tx, parsed)?;
    for (match_index, item) in parsed.matches.iter().enumerate() {
        import_syntax_match_rows(
            tx,
            generation_id,
            match_index,
            item,
            parsed.item_node_type.as_str(),
            parsed.capture_node_type.as_str(),
            parsed.query_field.as_deref(),
        )?;
    }
    Ok(())
}

fn write_syntax_query_generation_row(
    tx: &rusqlite::Transaction<'_>,
    parsed: &ParsedSyntaxQueryPacketImport<'_>,
) -> Result<(), String> {
    tx.execute(
        "INSERT OR REPLACE INTO syntax_query_generation (
            generation_id,
            language_id,
            provider_id,
            project_root,
            request_fingerprint,
            query_ast_fingerprint,
            grammar_id,
            grammar_profile_version,
            input_form,
            input_kind,
            match_count,
            truncated,
            packet_bytes,
            raw_source_stored
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0)",
        params![
            parsed.generation.generation_id.as_str(),
            parsed.generation.language_id.as_str(),
            parsed.generation.provider_id.as_str(),
            parsed.project_root.as_str(),
            parsed.request_fingerprint,
            parsed.query_ast_fingerprint.as_str(),
            parsed.grammar_id.as_str(),
            parsed.grammar_profile_version.as_str(),
            parsed.input_form.as_str(),
            parsed.input_kind,
            parsed.matches.len().min(i64::MAX as usize) as i64,
            parsed.truncated as i64,
            parsed.packet_bytes.min(i64::MAX as usize) as i64,
        ],
    )
    .map_err(|error| format!("failed to write syntax query generation rows: {error}"))?;
    Ok(())
}

fn write_syntax_query_pattern_row(
    tx: &rusqlite::Transaction<'_>,
    parsed: &ParsedSyntaxQueryPacketImport<'_>,
) -> Result<(), String> {
    tx.execute(
        "INSERT OR REPLACE INTO syntax_query_pattern (
            generation_id,
            pattern_index,
            query_input,
            compiled_source,
            selector,
            captures_json
        ) VALUES (?1, 0, ?2, ?3, ?4, ?5)",
        params![
            parsed.generation.generation_id.as_str(),
            parsed.query_input.as_str(),
            parsed.compiled_source.as_str(),
            parsed.selector.as_deref(),
            parsed.captures_json.as_str(),
        ],
    )
    .map_err(|error| format!("failed to write syntax query pattern row: {error}"))?;
    Ok(())
}

fn write_syntax_query_artifact_ref_rows(
    tx: &rusqlite::Transaction<'_>,
    parsed: &ParsedSyntaxQueryPacketImport<'_>,
) -> Result<(), String> {
    for (artifact_ordinal, artifact_id) in parsed.artifact_ids.iter().enumerate() {
        tx.execute(
            "INSERT OR REPLACE INTO syntax_query_artifact_ref (
                generation_id,
                artifact_ordinal,
                artifact_id
            ) VALUES (?1, ?2, ?3)",
            params![
                parsed.generation.generation_id.as_str(),
                artifact_ordinal.min(i64::MAX as usize) as i64,
                artifact_id.as_str(),
            ],
        )
        .map_err(|error| format!("failed to write syntax artifact ref row: {error}"))?;
    }
    Ok(())
}

fn delete_syntax_query_rows(
    tx: &rusqlite::Transaction<'_>,
    generation_id: &str,
) -> Result<(), String> {
    for table in [
        "syntax_query_capture_native_fact_ref",
        "syntax_query_capture",
        "syntax_query_match",
        "syntax_query_artifact_ref",
        "syntax_query_pattern",
        "syntax_query_generation",
    ] {
        tx.execute(
            &format!("DELETE FROM {table} WHERE generation_id = ?1"),
            params![generation_id],
        )
        .map_err(|error| format!("failed to clear {table} rows: {error}"))?;
    }
    Ok(())
}

fn import_syntax_match_rows(
    tx: &rusqlite::Transaction<'_>,
    generation_id: &str,
    match_index: usize,
    item: &Value,
    query_item_node_type: &str,
    query_capture_node_type: &str,
    query_field: Option<&str>,
) -> Result<(), String> {
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
    let match_ordinal = match_index.min(i64::MAX as usize) as i64;
    let native_fact_refs = string_array_field(item, "nativeFactRefs");
    let native_fact_refs_json = serde_json::to_string(&native_fact_refs)
        .map_err(|error| format!("failed to serialize syntax match native refs: {error}"))?;
    tx.execute(
        "INSERT OR REPLACE INTO syntax_query_match (
            generation_id,
            match_ordinal,
            match_id,
            path,
            start_line,
            end_line,
            native_fact_refs_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            generation_id,
            match_ordinal,
            optional_string_field(item, "id"),
            match_range.0,
            match_range.1,
            match_range.2,
            native_fact_refs_json,
        ],
    )
    .map_err(|error| format!("failed to write syntax match row: {error}"))?;

    for (capture_index, capture) in captures.iter().enumerate() {
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
        let capture_ordinal = capture_index.min(i64::MAX as usize) as i64;
        tx.execute(
            "INSERT OR REPLACE INTO syntax_query_capture (
                generation_id,
                match_ordinal,
                capture_ordinal,
                capture_id,
                capture_name,
                node_type,
                capture_node_type,
                item_node_type,
                field,
                capture_text,
                path,
                start_line,
                end_line
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                generation_id,
                match_ordinal,
                capture_ordinal,
                optional_string_field(capture, "id"),
                optional_string_field(capture, "name").unwrap_or("capture"),
                query_capture_node_type,
                query_capture_node_type,
                syntax_item_node_type(item, capture).unwrap_or(query_item_node_type),
                optional_string_field(capture, "field").or(query_field),
                text,
                capture_range.0,
                capture_range.1,
                capture_range.2,
            ],
        )
        .map_err(|error| format!("failed to write syntax capture row: {error}"))?;
        for (ref_index, native_fact_ref) in string_array_field(capture, "nativeFactRefs")
            .iter()
            .enumerate()
        {
            tx.execute(
                "INSERT OR REPLACE INTO syntax_query_capture_native_fact_ref (
                    generation_id,
                    match_ordinal,
                    capture_ordinal,
                    ref_ordinal,
                    native_fact_ref
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    generation_id,
                    match_ordinal,
                    capture_ordinal,
                    ref_index.min(i64::MAX as usize) as i64,
                    native_fact_ref,
                ],
            )
            .map_err(|error| format!("failed to write syntax capture native ref row: {error}"))?;
        }
    }
    Ok(())
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

fn string_array_field(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn string_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    optional_string_field(value, field).ok_or_else(|| format!("missing string field `{field}`"))
}

fn optional_string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn normalized_project_root(project_root: &Path) -> String {
    project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf())
        .display()
        .to_string()
}
