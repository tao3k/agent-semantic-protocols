// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Zero-regex adapters translating existing ASP native language provider wire
//! envelopes (`semantic-native-syntax-fact-index.v1` and
//! `provider-language-projection-batch-response`) into [`WorkspaceOpenEdbIngestBatch`].
//!
//! Enables existing heavy language providers (`asp-rust`, `asp-typescript`,
//! `asp-python`, `asp-julia`, `asp-gerbil-scheme`) to feed ASP's generation-scoped
//! Open EDB without provider binary rewrites while converging onto
//! `schemas/semantic-open-edb.v1.schema.json`.

use serde::Deserialize;
use serde_json::{Map, Value};

use super::edb::{
    SEMANTIC_OPEN_EDB_SCHEMA_ID, SEMANTIC_OPEN_EDB_SCHEMA_VERSION, WorkspaceEdbAttr,
    WorkspaceEdbEdgeFact, WorkspaceEdbEntityFact, WorkspaceOpenEdbIngestBatch,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeSyntaxFactIndexWire {
    language_id: String,
    provider_id: String,
    #[serde(default)]
    facts: Vec<NativeSyntaxFactWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeSyntaxFactWire {
    #[serde(default)]
    id: String,
    kind: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    language_kind: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    qualified_name: Option<String>,
    owner_path: String,
    #[serde(default)]
    location: Option<NativeLocationWire>,
    #[serde(default)]
    visibility: Option<String>,
    #[serde(default)]
    exported: Option<bool>,
    #[serde(default)]
    relations: Vec<NativeRelationWire>,
    #[serde(default)]
    fields: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeLocationWire {
    #[serde(default)]
    line: Option<usize>,
    #[serde(default)]
    start_line: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeRelationWire {
    kind: String,
    target: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderProjectionBatchWire {
    language_id: String,
    provider_id: String,
    #[serde(default)]
    owners: Vec<ProviderProjectedOwnerWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderProjectedOwnerWire {
    owner_path: String,
    #[serde(default)]
    items: Vec<ProviderProjectedItemWire>,
    #[serde(default)]
    relations: Vec<ProviderProjectedRelationWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderProjectedItemWire {
    #[serde(default)]
    item_id: String,
    kind: String,
    name: String,
    #[serde(default)]
    selector: String,
    #[serde(default)]
    source_byte_start: usize,
    #[serde(default)]
    line: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderProjectedRelationWire {
    #[serde(default, alias = "fromId", alias = "sourceId", alias = "src")]
    from: Option<String>,
    #[serde(default, alias = "toId", alias = "targetId", alias = "target")]
    to: Option<String>,
    #[serde(default, alias = "relationKind", alias = "kind")]
    relation: Option<String>,
    #[serde(default)]
    line: usize,
}

fn scalar_value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().filter_map(scalar_value_to_string).collect();
            (!parts.is_empty()).then(|| parts.join(","))
        }
        _ => None,
    }
}

/// Parse an ASP `semantic-native-syntax-fact-index.v1.schema.json` payload directly
/// into a zero-regex [`WorkspaceOpenEdbIngestBatch`].
pub fn parse_native_syntax_fact_index_json(
    bytes: &[u8],
) -> Result<WorkspaceOpenEdbIngestBatch, serde_json::Error> {
    let wire: NativeSyntaxFactIndexWire = serde_json::from_slice(bytes)?;
    let mut entities = Vec::with_capacity(wire.facts.len());
    let mut edges = Vec::new();

    for fact in wire.facts {
        let line = fact
            .location
            .as_ref()
            .and_then(|loc| loc.line.or(loc.start_line))
            .unwrap_or(1)
            .max(1);
        let name = fact
            .name
            .or_else(|| fact.qualified_name.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| fact.id.clone());
        let language_kind = fact
            .language_kind
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| fact.kind.clone());
        let source = if fact.source.is_empty() {
            "native-parser".to_owned()
        } else {
            fact.source
        };

        let mut attrs = Vec::new();
        if let Some(vis) = fact.visibility.filter(|s| !s.is_empty()) {
            attrs.push(WorkspaceEdbAttr {
                key: "visibility".to_owned(),
                value: vis,
            });
        }
        if let Some(qname) = fact.qualified_name.filter(|s| !s.is_empty()) {
            attrs.push(WorkspaceEdbAttr {
                key: "qualifiedName".to_owned(),
                value: qname,
            });
        }
        if let Some(exported) = fact.exported {
            attrs.push(WorkspaceEdbAttr {
                key: "exported".to_owned(),
                value: exported.to_string(),
            });
        }
        for (key, value) in &fact.fields {
            if let Some(val) = scalar_value_to_string(value) {
                attrs.push(WorkspaceEdbAttr {
                    key: key.clone(),
                    value: val,
                });
            }
        }

        for rel in fact.relations {
            if !rel.target.is_empty() && !rel.kind.is_empty() {
                edges.push(WorkspaceEdbEdgeFact {
                    src: name.clone(),
                    relation: rel.kind,
                    dst: rel.target,
                    path: fact.owner_path.clone(),
                    line,
                });
            }
        }

        entities.push(WorkspaceEdbEntityFact {
            entity_id: fact.id,
            file_id: 0,
            path: fact.owner_path,
            line,
            name,
            language: wire.language_id.clone(),
            kind: fact.kind,
            language_kind,
            source,
            attrs,
        });
    }

    Ok(WorkspaceOpenEdbIngestBatch {
        schema_id: Some(SEMANTIC_OPEN_EDB_SCHEMA_ID.to_owned()),
        schema_version: Some(SEMANTIC_OPEN_EDB_SCHEMA_VERSION.to_owned()),
        language_id: Some(wire.language_id),
        provider_id: Some(wire.provider_id),
        entities,
        edges,
    })
}

/// Parse an ASP `provider-language-projection-batch-response` payload directly
/// into a zero-regex [`WorkspaceOpenEdbIngestBatch`].
pub fn parse_provider_projection_batch_json(
    bytes: &[u8],
) -> Result<WorkspaceOpenEdbIngestBatch, serde_json::Error> {
    let wire: ProviderProjectionBatchWire = serde_json::from_slice(bytes)?;
    let mut entities = Vec::new();
    let mut edges = Vec::new();

    for owner in wire.owners {
        for item in owner.items {
            let mut attrs = Vec::new();
            if !item.selector.is_empty() {
                attrs.push(WorkspaceEdbAttr {
                    key: "selector".to_owned(),
                    value: item.selector,
                });
            }
            if item.source_byte_start > 0 {
                attrs.push(WorkspaceEdbAttr {
                    key: "byteStart".to_owned(),
                    value: item.source_byte_start.to_string(),
                });
            }
            entities.push(WorkspaceEdbEntityFact {
                entity_id: item.item_id,
                file_id: 0,
                path: owner.owner_path.clone(),
                line: item.line.unwrap_or(1).max(1),
                name: item.name,
                language: wire.language_id.clone(),
                kind: item.kind.clone(),
                language_kind: item.kind,
                source: "native-parser".to_owned(),
                attrs,
            });
        }
        for rel in owner.relations {
            if let (Some(src), Some(dst), Some(relation)) = (rel.from, rel.to, rel.relation)
                && !src.is_empty()
                && !dst.is_empty()
                && !relation.is_empty()
            {
                edges.push(WorkspaceEdbEdgeFact {
                    src,
                    relation,
                    dst,
                    path: owner.owner_path.clone(),
                    line: rel.line,
                });
            }
        }
    }

    Ok(WorkspaceOpenEdbIngestBatch {
        schema_id: Some(SEMANTIC_OPEN_EDB_SCHEMA_ID.to_owned()),
        schema_version: Some(SEMANTIC_OPEN_EDB_SCHEMA_VERSION.to_owned()),
        language_id: Some(wire.language_id),
        provider_id: Some(wire.provider_id),
        entities,
        edges,
    })
}
