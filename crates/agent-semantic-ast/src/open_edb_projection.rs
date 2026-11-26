// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Single-tree Open EDB projection converting verified [`AstNodeSpanRecord`]s
//! directly into ASP's zero-regex `semantic-open-edb.v1` [`WorkspaceOpenEdbIngestBatch`].

use agent_semantic_search::{
    SEMANTIC_OPEN_EDB_SCHEMA_ID, SEMANTIC_OPEN_EDB_SCHEMA_VERSION, WorkspaceEdbAttr,
    WorkspaceEdbEntityFact, WorkspaceOpenEdbIngestBatch,
};

use crate::types::{AstLanguageId, AstNodeSpanRecord, AstProviderId, AstWorkspacePath};

/// Project freshly verified native AST node spans directly into an Open EDB
/// ingestion batch (`semantic-open-edb.v1`, `source = "native-parser"`).
#[must_use]
pub fn project_nodes_to_open_edb_batch(
    path: &AstWorkspacePath,
    language_id: &AstLanguageId,
    provider_id: &AstProviderId,
    nodes: &[AstNodeSpanRecord],
) -> WorkspaceOpenEdbIngestBatch {
    let entities = nodes
        .iter()
        .map(|node| WorkspaceEdbEntityFact {
            entity_id: String::new(),
            file_id: 0,
            path: path.as_str().to_owned(),
            line: node.line.max(1),
            name: node.name.clone(),
            language: language_id.as_str().to_owned(),
            kind: node.kind.as_open_edb_kind().to_owned(),
            language_kind: node.language_kind.as_str().to_owned(),
            source: "native-parser".to_owned(),
            attrs: node
                .attrs
                .iter()
                .map(|a| WorkspaceEdbAttr {
                    key: a.key.clone(),
                    value: a.value.clone(),
                })
                .collect(),
        })
        .collect();

    WorkspaceOpenEdbIngestBatch {
        schema_id: Some(SEMANTIC_OPEN_EDB_SCHEMA_ID.to_owned()),
        schema_version: Some(SEMANTIC_OPEN_EDB_SCHEMA_VERSION.to_owned()),
        language_id: Some(language_id.as_str().to_owned()),
        provider_id: Some(provider_id.as_str().to_owned()),
        entities,
        edges: Vec::new(),
    }
}
