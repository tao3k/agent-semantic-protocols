// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Single-tree Open EDB projection converting verified [`AstNodeSpanRecord`]s
//! and semantic [`AstSemanticEdgeRecord`]s directly into ASP's zero-regex
//! `semantic-open-edb.v1` [`WorkspaceOpenEdbIngestBatch`].

use std::collections::BTreeSet;

use agent_semantic_provider_protocol::LanguageConformancePack;
use agent_semantic_search::{
    SEMANTIC_OPEN_EDB_SCHEMA_ID, SEMANTIC_OPEN_EDB_SCHEMA_VERSION, WorkspaceEdbAttr,
    WorkspaceEdbEdgeFact, WorkspaceEdbEntityFact, WorkspaceOpenEdbIngestBatch,
};
use serde_json::{Value, json};

use crate::types::{
    AstLanguageId, AstNodeSpanRecord, AstProviderId, AstSemanticEdgeRecord, AstUniversalKind,
    AstWorkspacePath,
};

/// Derive a Yuanxu-compatible targeted JSON Schema enforcing a language's exact
/// `languageId`, `kind`, and native `languageKind` surface from its DSL pack.
#[must_use]
pub fn derive_targeted_entity_schema(
    language_id: &AstLanguageId,
    dsl_pack: Option<&LanguageConformancePack>,
) -> Value {
    let mut native_kinds = Vec::new();
    let mut general_kinds = Vec::new();
    if let Some(pack) = dsl_pack {
        for mapping in &pack.kind_mappings {
            if !native_kinds.contains(&mapping.native_language_kind) {
                native_kinds.push(mapping.native_language_kind.clone());
            }
            if !general_kinds.contains(&mapping.general_kind) {
                general_kinds.push(mapping.general_kind.clone());
            }
        }
    }
    let kind_prop = if general_kinds.is_empty() {
        json!({ "type": "string", "minLength": 1 })
    } else {
        json!({ "type": "string", "enum": general_kinds })
    };
    let language_kind_prop = if native_kinds.is_empty() {
        json!({ "type": "string", "minLength": 1 })
    } else {
        json!({ "type": "string", "enum": native_kinds })
    };
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["languageId", "kind", "languageKind", "name", "line"],
        "properties": {
            "languageId": { "type": "string", "const": language_id.as_str() },
            "kind": kind_prop,
            "languageKind": language_kind_prop,
            "name": { "type": "string", "minLength": 1 },
            "line": { "type": "integer", "minimum": 1 }
        }
    })
}

fn build_entity_attrs(node: &AstNodeSpanRecord) -> Vec<WorkspaceEdbAttr> {
    let mut attrs: Vec<WorkspaceEdbAttr> = node
        .attrs
        .iter()
        .map(|a| WorkspaceEdbAttr {
            key: a.key.clone(),
            value: a.value.clone(),
        })
        .collect();
    if !attrs.iter().any(|a| a.key == "locator") {
        attrs.push(WorkspaceEdbAttr {
            key: "locator".to_owned(),
            value: node.locator.as_str().to_owned(),
        });
    }
    attrs
}

fn push_unique_edge(
    seen: &mut BTreeSet<(String, String, String, usize)>,
    out: &mut Vec<WorkspaceEdbEdgeFact>,
    src: String,
    dst: String,
    relation: String,
    path: &str,
    line: usize,
) {
    let key = (src.clone(), dst.clone(), relation.clone(), line);
    if seen.insert(key) {
        out.push(WorkspaceEdbEdgeFact {
            src,
            relation,
            dst,
            path: path.to_owned(),
            line,
        });
    }
}

/// Project freshly verified native AST node spans and semantic edges directly
/// into an Open EDB ingestion batch (`semantic-open-edb.v1`, `source = "native-parser"`).
///
/// Automatically preserves each node's authoritative `locator` in both
/// `entityId` and `attr(id, "locator", loc)`, derives structural `contains`,
/// `imports`, and `implements` edges across all languages, and merges any
/// explicit provider-emitted semantic edges (`calls`, `imports`, `implements`,
/// `references`).
#[must_use]
pub fn project_inspection_to_open_edb_batch(
    path: &AstWorkspacePath,
    language_id: &AstLanguageId,
    provider_id: &AstProviderId,
    nodes: &[AstNodeSpanRecord],
    explicit_edges: &[AstSemanticEdgeRecord],
) -> WorkspaceOpenEdbIngestBatch {
    let path_str = path.as_str();
    let entities = nodes
        .iter()
        .map(|node| WorkspaceEdbEntityFact {
            entity_id: node.locator.as_str().to_owned(),
            file_id: 0,
            path: path_str.to_owned(),
            line: node.line.max(1),
            name: node.name.clone(),
            language: language_id.as_str().to_owned(),
            kind: node.kind.as_open_edb_kind().to_owned(),
            language_kind: node.language_kind.as_str().to_owned(),
            source: "native-parser".to_owned(),
            attrs: build_entity_attrs(node),
        })
        .collect();

    let mut seen_edges = BTreeSet::new();
    let mut edges = Vec::new();
    for node in nodes {
        let line = node.line.max(1);
        let loc = node.locator.as_str();
        if let Some(parent) = &node.parent {
            push_unique_edge(
                &mut seen_edges,
                &mut edges,
                parent.as_str().to_owned(),
                loc.to_owned(),
                "contains".to_owned(),
                path_str,
                line,
            );
        }
        if node.kind == AstUniversalKind::Import {
            let target = node
                .attrs
                .iter()
                .find(|a| a.key == "import_target")
                .map(|a| a.value.as_str())
                .unwrap_or(node.name.as_str());
            push_unique_edge(
                &mut seen_edges,
                &mut edges,
                path_str.to_owned(),
                target.to_owned(),
                "imports".to_owned(),
                path_str,
                line,
            );
        }
        for attr in &node.attrs {
            if matches!(attr.key.as_str(), "implements" | "extends" | "calls") {
                push_unique_edge(
                    &mut seen_edges,
                    &mut edges,
                    loc.to_owned(),
                    attr.value.clone(),
                    attr.key.clone(),
                    path_str,
                    line,
                );
            }
        }
    }
    for edge in explicit_edges {
        push_unique_edge(
            &mut seen_edges,
            &mut edges,
            edge.src.clone(),
            edge.dst.clone(),
            edge.relation.clone(),
            path_str,
            edge.line.max(1),
        );
    }

    WorkspaceOpenEdbIngestBatch {
        schema_id: Some(SEMANTIC_OPEN_EDB_SCHEMA_ID.to_owned()),
        schema_version: Some(SEMANTIC_OPEN_EDB_SCHEMA_VERSION.to_owned()),
        language_id: Some(language_id.as_str().to_owned()),
        provider_id: Some(provider_id.as_str().to_owned()),
        entities,
        edges,
    }
}

/// Project freshly verified native AST node spans into an Open EDB batch.
#[must_use]
pub fn project_nodes_to_open_edb_batch(
    path: &AstWorkspacePath,
    language_id: &AstLanguageId,
    provider_id: &AstProviderId,
    nodes: &[AstNodeSpanRecord],
) -> WorkspaceOpenEdbIngestBatch {
    project_inspection_to_open_edb_batch(path, language_id, provider_id, nodes, &[])
}
