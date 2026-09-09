// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Derived search attachment builders for one immutable generation.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{SearchOwnerRecord, WorkspaceSearchGenerationAuthority};

pub(in crate::runtime_server_workspace) fn build_cold_rg_corpus(
    mapping: &[u8],
    owner_bytes_range: &std::ops::Range<usize>,
    owner_directory_records: &BTreeMap<String, Arc<SearchOwnerRecord>>,
    content_generation_digest: &str,
) -> Result<agent_semantic_search::ColdRgCorpusArtifact, String> {
    let owners = owner_directory_records
        .values()
        .map(|record| {
            let start = owner_bytes_range
                .start
                .checked_add(record.byte_offset as usize)
                .ok_or_else(|| "cold rg corpus owner offset overflows".to_owned())?;
            let end = start
                .checked_add(record.byte_length as usize)
                .ok_or_else(|| "cold rg corpus owner range overflows".to_owned())?;
            let bytes = mapping
                .get(start..end)
                .ok_or_else(|| "cold rg corpus owner exceeds mapped generation".to_owned())?;
            Ok(agent_semantic_search::ColdRgCorpusOwner {
                owner_path: &record.owner_path,
                content_digest: &record.content_digest,
                bytes,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    agent_semantic_search::build_cold_rg_corpus(content_generation_digest, owners)
}

pub(in crate::runtime_server_workspace) fn build_resident_graph_generation(
    authority: &WorkspaceSearchGenerationAuthority,
    owner_directory_records: &BTreeMap<String, Arc<SearchOwnerRecord>>,
    mut graph_relations: Vec<
        agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
    >,
) -> Result<agent_semantic_search::ResidentGraphGeneration, String> {
    add_parser_owner_contains_relations(
        &mut graph_relations,
        owner_directory_records
            .iter()
            .flat_map(|(owner_path, owner)| {
                owner
                    .selectors
                    .iter()
                    .map(|selector| (owner_path.clone(), selector.selector.clone()))
            }),
    );
    let graph_request = Arc::new(agent_semantic_search::SearchGenerationGraphRequest::new(
        &authority.content_search_generation,
        authority.source_snapshot.clone(),
        authority.workspace_generation.clone(),
        owner_directory_records.keys().cloned(),
        graph_relations,
    )?);
    agent_semantic_search::build_resident_graph_generation(graph_request)
}

fn add_parser_owner_contains_relations(
    graph_relations: &mut Vec<
        agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
    >,
    owner_selectors: impl IntoIterator<Item = (String, String)>,
) {
    let mut relation_keys = graph_relations
        .iter()
        .map(graph_relation_key)
        .collect::<BTreeSet<_>>();
    for (owner_path, selector) in owner_selectors {
        let relation = agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation {
            from: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
                id: owner_path,
            },
            kind: agent_semantic_content_identity::ProviderRelationKindV1::from("CONTAINS"),
            to: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
                id: selector,
            },
        };
        if relation_keys.insert(graph_relation_key(&relation)) {
            graph_relations.push(relation);
        }
    }
}

fn graph_relation_key(
    relation: &agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
) -> (String, String, String, String, String) {
    (
        relation.from.kind.as_str().to_owned(),
        relation.from.id.clone(),
        relation.kind.as_str().to_ascii_uppercase(),
        relation.to.kind.as_str().to_owned(),
        relation.to.id.clone(),
    )
}

#[cfg(test)]
#[path = "../../tests/unit/search_index_projection_builders.rs"]
mod tests;
