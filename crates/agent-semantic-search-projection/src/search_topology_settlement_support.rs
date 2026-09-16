// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Rendering and JSON-field helpers for Search topology settlement.

use serde_json::{Map, Value};

use crate::search_topology_settlement::{SearchTopologySettlementError, error, invalid};

pub(super) fn digest_json(value: &Value) -> Result<String, SearchTopologySettlementError> {
    let encoded = serde_json::to_vec(value)
        .map_err(|failure| error("rendering-failed", failure.to_string()))?;
    Ok(format!("blake3-256:{}", blake3::hash(&encoded).to_hex()))
}

pub(super) fn org_render_error(
    failure: orgize::ast::OrgSourceBlockDocumentError,
) -> SearchTopologySettlementError {
    error("rendering-failed", failure.to_string())
}

pub(super) fn render_node(
    node: &Map<String, Value>,
) -> Result<String, SearchTopologySettlementError> {
    let id = required_text(node, "id")?;
    let language = required_text(node, "language")?;
    if let Some(annotation) = node.get("annotation") {
        let annotation = required_object(annotation, "nodes[].annotation")?;
        let mut properties = vec![
            format!("text:{}", quoted(required_text(annotation, "text")?)),
            format!("state:{}", quoted(required_text(annotation, "state")?)),
            format!(
                "binding:{}",
                quoted(required_text(annotation, "bindingDigest")?)
            ),
            format!(
                "premises:{}",
                render_text_array(annotation, "premiseWitnesses")?
            ),
            format!(
                "producer:{}",
                quoted(required_text(annotation, "producer")?)
            ),
        ];
        if let Some(reference) = annotation
            .get("admissionReceiptRef")
            .and_then(Value::as_str)
        {
            properties.push(format!("admission:{}", quoted(reference)));
        }
        return Ok(format!(
            "({id}:SemanticAnnotation {{{}}})",
            properties.join(",")
        ));
    }
    if let Some(excerpt) = node.get("excerpt") {
        let excerpt = required_object(excerpt, "nodes[].excerpt")?;
        return Ok(format!(
            "({id}:SourceHit {{language:{},path:{},match:{},read:{},witness:{}}})",
            quoted(language),
            quoted(required_text(excerpt, "path")?),
            serde_json::to_string(
                excerpt
                    .get("match")
                    .ok_or_else(|| error("schema-invalid", "excerpt match is absent"))?
            )
            .map_err(|failure| error("rendering-failed", failure.to_string()))?,
            serde_json::to_string(
                excerpt
                    .get("read")
                    .ok_or_else(|| error("schema-invalid", "excerpt read is absent"))?
            )
            .map_err(|failure| error("rendering-failed", failure.to_string()))?,
            quoted(required_text(excerpt, "witness")?),
        ));
    }

    let (_, body) = render_result_node(node)?;
    Ok(format!(
        "({}:Language)-[:RESULTS]->[{body}]",
        gql_alias(language),
    ))
}

pub(super) fn render_result_node(
    node: &Map<String, Value>,
) -> Result<(String, String), SearchTopologySettlementError> {
    let id = required_text(node, "id")?;
    let language = required_text(node, "language")?;
    let kind = required_text(node, "kind")?;
    let label = format!("{}{}", gql_type_prefix(language), gql_type_prefix(kind));
    let mut properties = Vec::new();
    if let Some(name) = node.get("name").and_then(Value::as_str) {
        properties.push(format!("name:{}", quoted(name)));
    }
    if let Some(selector) = node.get("selector").and_then(Value::as_str) {
        properties.push(format!("selector:{}", quoted(selector)));
    } else {
        properties.push(format!(
            "owner_locator:{}",
            quoted(required_text(node, "ownerLocator")?)
        ));
    }
    if let Some(projection) = node.get("projection") {
        properties.push(format!(
            "projection:{}",
            render_projection(required_object(projection, "nodes[].projection")?)?
        ));
    }
    Ok((
        language.to_owned(),
        format!("({id}:{label} {{{}}})", properties.join(",")),
    ))
}

pub(super) fn render_projection(
    projection: &Map<String, Value>,
) -> Result<String, SearchTopologySettlementError> {
    let mut properties = vec![
        format!("rank:{}", required_u64(projection, "rank")?),
        format!("depth:{}", required_u64(projection, "depth")?),
    ];
    if let Some(hit) = projection.get("hit") {
        let hit = required_object(hit, "projection.hit")?;
        let mut hit_properties = Vec::new();
        if let Some(value) = hit.get("rg") {
            hit_properties.push(format!(
                "rg:{}",
                serde_json::to_string(value)
                    .map_err(|failure| error("rendering-failed", failure.to_string()))?
            ));
        }
        if let Some(value) = hit.get("tantivy") {
            hit_properties.push(format!(
                "tantivy:{}",
                serde_json::to_string(value)
                    .map_err(|failure| error("rendering-failed", failure.to_string()))?
            ));
        }
        if let Some(value) = hit.get("native").and_then(Value::as_bool) {
            hit_properties.push(format!("native:{value}"));
        }
        properties.push(format!("hit:{{{}}}", hit_properties.join(",")));
    }
    if let Some(jq) = projection.get("jq").and_then(Value::as_str) {
        properties.push(format!("jq:{}", quoted(jq)));
    }
    Ok(format!("{{{}}}", properties.join(",")))
}

pub(super) fn validate_projection(
    projection: &Map<String, Value>,
) -> Result<(), SearchTopologySettlementError> {
    if required_u64(projection, "rank")? == 0 {
        return invalid("schema-invalid", "projection rank must be positive");
    }
    required_u64(projection, "depth")?;
    if projection
        .keys()
        .any(|field| !matches!(field.as_str(), "rank" | "depth" | "hit" | "jq"))
    {
        return invalid("schema-invalid", "projection contains an unknown field");
    }
    let has_jq = projection
        .get("jq")
        .and_then(Value::as_str)
        .is_some_and(|jq| !jq.is_empty());
    let has_hit = if let Some(hit) = projection.get("hit") {
        let hit = required_object(hit, "projection.hit")?;
        if hit.is_empty()
            || hit
                .keys()
                .any(|field| !matches!(field.as_str(), "rg" | "tantivy" | "native"))
        {
            return invalid("schema-invalid", "projection hit evidence is invalid");
        }
        for boolean in ["native"] {
            if hit.get(boolean).is_some_and(|value| !value.is_boolean()) {
                return invalid(
                    "schema-invalid",
                    format!("projection.hit.{boolean} must be boolean"),
                );
            }
        }
        if let Some(ranges) = hit.get("rg") {
            let ranges = ranges
                .as_array()
                .filter(|ranges| !ranges.is_empty())
                .ok_or_else(|| error("schema-invalid", "projection.hit.rg must be non-empty"))?;
            for range in ranges {
                let range = range
                    .as_array()
                    .filter(|range| range.len() == 2)
                    .ok_or_else(|| {
                        error(
                            "schema-invalid",
                            "projection.hit.rg range must have two lines",
                        )
                    })?;
                let start = range[0].as_u64().unwrap_or(0);
                let end = range[1].as_u64().unwrap_or(0);
                if start == 0 || start > end {
                    return invalid("schema-invalid", "projection.hit.rg range is invalid");
                }
            }
        }
        if let Some(queries) = hit.get("tantivy") {
            let queries = queries
                .as_array()
                .filter(|queries| !queries.is_empty())
                .ok_or_else(|| {
                    error("schema-invalid", "projection.hit.tantivy must be non-empty")
                })?;
            let values = queries
                .iter()
                .map(|query| {
                    query
                        .as_str()
                        .filter(|query| !query.is_empty())
                        .ok_or_else(|| {
                            error("schema-invalid", "projection.hit.tantivy query is invalid")
                        })
                })
                .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
            if values.len() != queries.len() {
                return invalid(
                    "schema-invalid",
                    "projection.hit.tantivy queries must be unique",
                );
            }
        }
        true
    } else {
        false
    };
    if !has_hit && !has_jq {
        return invalid(
            "schema-invalid",
            "projection requires hit evidence or a jq program",
        );
    }
    Ok(())
}

pub(super) fn render_edge(
    edge: &Map<String, Value>,
) -> Result<String, SearchTopologySettlementError> {
    let modality = required_text(edge, "modality")?;
    let mut properties = vec![format!("modality:{}", quoted(modality))];
    if let Some(derived_by) = edge.get("derivedBy").and_then(Value::as_str) {
        properties.push(format!("derived_by:{}", quoted(derived_by)));
    }
    if let Some(proof) = edge.get("proofRef").and_then(Value::as_str) {
        properties.push(format!("proof:{}", quoted(proof)));
    }
    properties.push(format!(
        "witnesses:{}",
        render_text_array(edge, "witnesses")?
    ));
    Ok(format!(
        "({})-[:{} {{{}}}]->({})",
        required_text(edge, "from")?,
        required_text(edge, "relation")?,
        properties.join(","),
        required_text(edge, "to")?,
    ))
}

pub(super) fn render_text_array(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, SearchTopologySettlementError> {
    serde_json::to_string(&text_array(object, field)?)
        .map_err(|failure| error("rendering-failed", failure.to_string()))
}

pub(super) fn quoted(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

pub(super) fn gql_alias(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

pub(super) fn gql_type_prefix(value: &str) -> String {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            characters
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + characters.as_str())
                .unwrap_or_default()
        })
        .collect()
}

pub(super) fn is_blake3_digest(value: &str) -> bool {
    value.strip_prefix("blake3-256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

pub(super) fn required_object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a Map<String, Value>, SearchTopologySettlementError> {
    value
        .as_object()
        .ok_or_else(|| error("schema-invalid", format!("{field} must be an object")))
}

pub(super) fn required_object_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Map<String, Value>, SearchTopologySettlementError> {
    object
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| error("schema-invalid", format!("{field} must be an object")))
}

pub(super) fn required_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a [Value], SearchTopologySettlementError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| error("schema-invalid", format!("{field} must be an array")))
}

pub(super) fn required_text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, SearchTopologySettlementError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| error("schema-invalid", format!("{field} must be non-empty text")))
}

pub(super) fn required_u64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<u64, SearchTopologySettlementError> {
    object.get(field).and_then(Value::as_u64).ok_or_else(|| {
        error(
            "schema-invalid",
            format!("{field} must be an unsigned integer"),
        )
    })
}

pub(super) fn require_text_eq(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), SearchTopologySettlementError> {
    let observed = required_text(object, field)?;
    if observed != expected {
        return invalid(
            "schema-identity-drift",
            format!("{field} expected {expected} observed {observed}"),
        );
    }
    Ok(())
}

pub(super) fn text_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Vec<&'a str>, SearchTopologySettlementError> {
    required_array(object, field)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    error(
                        "schema-invalid",
                        format!("{field} entries must be non-empty text"),
                    )
                })
        })
        .collect()
}
