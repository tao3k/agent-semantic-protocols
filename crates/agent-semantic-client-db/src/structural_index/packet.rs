use std::path::PathBuf;

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, ClientCacheFileHash, ClientCacheGeneration,
    SemanticSchemaId, SemanticSchemaVersion,
};
use serde_json::Value;

use super::types::{
    ClientDbStructuralDependencyUsage, ClientDbStructuralHash, ClientDbStructuralIndexImport,
    ClientDbStructuralKind, ClientDbStructuralLocator, ClientDbStructuralName,
    ClientDbStructuralOwner, ClientDbStructuralPath, ClientDbStructuralQueryKey,
    ClientDbStructuralSource, ClientDbStructuralSymbol,
};

const SEMANTIC_STRUCTURAL_INDEX_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-structural-index";
const SEMANTIC_STRUCTURAL_INDEX_SCHEMA_VERSION: &str = "1";
const SEMANTIC_STRUCTURAL_INDEX_PROTOCOL_ID: &str = "agent.semantic-protocols.semantic-language";
const SEMANTIC_STRUCTURAL_INDEX_PROTOCOL_VERSION: &str = "1";

pub(crate) fn parse_structural_index_packet_import(
    generation: &ClientCacheGeneration,
    packet_bytes: &[u8],
) -> Result<ClientDbStructuralIndexImport, String> {
    if generation.raw_source_stored {
        return Err("structural index rows refuse rawSourceStored=true generation".to_string());
    }
    let packet: Value = serde_json::from_slice(packet_bytes)
        .map_err(|error| format!("failed to parse semantic structural index packet: {error}"))?;
    validate_structural_index_packet(&packet)?;

    let generation_id = string_field(&packet, "generationId")?;
    if generation_id != generation.generation_id.as_str() {
        return Err("structural index packet generationId does not match manifest".to_string());
    }
    let language_id = string_field(&packet, "languageId")?;
    if language_id != generation.language_id.as_str() {
        return Err("structural index packet languageId does not match manifest".to_string());
    }
    let provider_id = string_field(&packet, "providerId")?;
    if provider_id != generation.provider_id.as_str() {
        return Err("structural index packet providerId does not match manifest".to_string());
    }

    Ok(ClientDbStructuralIndexImport {
        generation_id: generation.generation_id.clone(),
        language_id: generation.language_id.clone(),
        provider_id: generation.provider_id.clone(),
        provider_version: optional_string_field(&packet, "providerVersion")
            .or(generation.provider_version.as_deref())
            .map(ClientDbStructuralName::new),
        export_method: optional_string_field(&packet, "exportMethod")
            .or(generation.export_method.as_deref())
            .map(CacheExportMethod::from),
        project_root: PathBuf::from(string_field(&packet, "projectRoot")?),
        package_root: optional_string_field(&packet, "packageRoot")
            .or(generation.package_root.as_deref())
            .map(ClientDbStructuralPath::new),
        schema_id: SemanticSchemaId::from(SEMANTIC_STRUCTURAL_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(SEMANTIC_STRUCTURAL_INDEX_SCHEMA_VERSION),
        source_artifact_id: optional_string_field(&packet, "sourceArtifactId")
            .map(CacheArtifactId::from)
            .or_else(|| {
                generation
                    .artifact_ids
                    .as_deref()
                    .and_then(|artifact_ids| artifact_ids.first())
                    .cloned()
            }),
        file_hashes: parse_file_hashes(&packet)?,
        owners: parse_owner_rows(&packet)?,
        symbols: parse_symbol_rows(&packet)?,
        dependency_usages: parse_dependency_rows(&packet)?,
    })
}

fn validate_structural_index_packet(packet: &Value) -> Result<(), String> {
    if string_field(packet, "schemaId")? != SEMANTIC_STRUCTURAL_INDEX_SCHEMA_ID {
        return Err("structural index rows require semantic-structural-index packet".to_string());
    }
    if string_field(packet, "schemaVersion")? != SEMANTIC_STRUCTURAL_INDEX_SCHEMA_VERSION {
        return Err("structural index rows require schemaVersion=1".to_string());
    }
    if string_field(packet, "protocolId")? != SEMANTIC_STRUCTURAL_INDEX_PROTOCOL_ID {
        return Err("structural index rows require semantic-language protocol".to_string());
    }
    if string_field(packet, "protocolVersion")? != SEMANTIC_STRUCTURAL_INDEX_PROTOCOL_VERSION {
        return Err("structural index rows require protocolVersion=1".to_string());
    }
    if packet
        .get("rawSourceStored")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        return Err("structural index rows refuse packet rawSourceStored=true".to_string());
    }
    Ok(())
}

fn parse_file_hashes(packet: &Value) -> Result<Vec<ClientCacheFileHash>, String> {
    let values = array_field(packet, "fileHashes")?;
    if values.is_empty() {
        return Err("structural index import requires file hash evidence".to_string());
    }
    values
        .iter()
        .map(|value| {
            Ok(ClientCacheFileHash {
                path: string_field(value, "path")?.to_string(),
                sha256: string_field(value, "sha256")?.to_string(),
                byte_len: value.get("byteLen").and_then(Value::as_u64).unwrap_or(0),
                mtime_ms: value.get("mtimeMs").and_then(Value::as_u64).unwrap_or(0),
            })
        })
        .collect()
}

fn parse_owner_rows(packet: &Value) -> Result<Vec<ClientDbStructuralOwner>, String> {
    array_field(packet, "owners")?
        .iter()
        .map(|value| {
            reject_raw_source_fields(value, "owner")?;
            let (start_line, end_line) = value
                .get("location")
                .and_then(parse_location_lines)
                .unwrap_or((None, None));
            Ok(ClientDbStructuralOwner {
                owner_path: ClientDbStructuralPath::new(string_field(value, "ownerPath")?),
                owner_kind: ClientDbStructuralKind::new(string_field(value, "ownerKind")?),
                source_authority: ClientDbStructuralSource::new(string_field(
                    value,
                    "sourceAuthority",
                )?),
                start_line,
                end_line,
                query_keys: parse_query_key_values(value)?,
            })
        })
        .collect()
}

fn parse_symbol_rows(packet: &Value) -> Result<Vec<ClientDbStructuralSymbol>, String> {
    array_field(packet, "symbols")?
        .iter()
        .map(|value| {
            reject_raw_source_fields(value, "symbol")?;
            let mut query_keys = parse_query_key_values(value)?;
            append_query_key_if_missing(
                &mut query_keys,
                optional_string_field(value, "qualifiedName"),
            );
            Ok(ClientDbStructuralSymbol {
                owner_path: ClientDbStructuralPath::new(string_field(value, "ownerPath")?),
                name: ClientDbStructuralName::new(string_field(value, "name")?),
                kind: ClientDbStructuralKind::new(string_field(value, "kind")?),
                visibility: optional_string_field(value, "visibility")
                    .map(ClientDbStructuralKind::new),
                source_locator: optional_string_field(value, "sourceLocator")
                    .map(ClientDbStructuralLocator::new),
                query_keys,
            })
        })
        .collect()
}

fn parse_dependency_rows(packet: &Value) -> Result<Vec<ClientDbStructuralDependencyUsage>, String> {
    array_field(packet, "dependencyUsages")?
        .iter()
        .map(|value| {
            reject_raw_source_fields(value, "dependency usage")?;
            Ok(ClientDbStructuralDependencyUsage {
                owner_path: ClientDbStructuralPath::new(string_field(value, "ownerPath")?),
                package_name: ClientDbStructuralName::new(string_field(value, "packageName")?),
                package_version: optional_string_field(value, "packageVersion")
                    .map(ClientDbStructuralName::new),
                api_name: optional_string_field(value, "apiName").map(ClientDbStructuralName::new),
                import_path: optional_string_field(value, "importPath")
                    .map(ClientDbStructuralPath::new),
                manifest_path: optional_string_field(value, "manifestPath")
                    .map(ClientDbStructuralPath::new),
                lockfile_hash: optional_string_field(value, "lockfileHash")
                    .map(ClientDbStructuralHash::new),
                source: ClientDbStructuralSource::new(string_field(value, "source")?),
                source_locator: optional_string_field(value, "sourceLocator")
                    .map(ClientDbStructuralLocator::new),
                query_keys: parse_query_key_values(value)?,
            })
        })
        .collect()
}

fn reject_raw_source_fields(value: &Value, row_kind: &str) -> Result<(), String> {
    for field in [
        "body",
        "code",
        "content",
        "rawSource",
        "rawSourceText",
        "snippet",
        "sourceText",
        "text",
    ] {
        if value.get(field).is_some() {
            return Err(format!(
                "structural index {row_kind} row refuses raw source field `{field}`"
            ));
        }
    }
    Ok(())
}

fn append_query_key_if_missing(
    query_keys: &mut Vec<ClientDbStructuralQueryKey>,
    value: Option<&str>,
) {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return;
    };
    if !query_keys
        .iter()
        .any(|query_key| query_key.as_str() == value)
    {
        query_keys.push(ClientDbStructuralQueryKey::new(value));
    }
}

fn parse_query_key_values(value: &Value) -> Result<Vec<ClientDbStructuralQueryKey>, String> {
    array_field(value, "queryKeys")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ClientDbStructuralQueryKey::new)
                .ok_or_else(|| "structural index queryKeys must be strings".to_string())
        })
        .collect()
}

fn parse_location_lines(value: &Value) -> Option<(Option<u32>, Option<u32>)> {
    let line_range = value.get("lineRange")?.as_str()?;
    let (start, end) = line_range.split_once(':')?;
    Some((Some(start.parse().ok()?), Some(end.parse().ok()?)))
}

fn array_field<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing array field `{field}`"))
}

fn string_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    optional_string_field(value, field).ok_or_else(|| format!("missing string field `{field}`"))
}

fn optional_string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}
