// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Derived search attachment builders for one immutable generation.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::{SearchOwnerRecord, WorkspaceSearchGenerationAuthority};

pub(super) fn build_cold_rg_corpus(
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

pub(super) fn build_resident_graph_generation(
    authority: &WorkspaceSearchGenerationAuthority,
    owner_paths: Vec<String>,
    graph_relations: Vec<
        agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
    >,
) -> Result<agent_semantic_search::ResidentGraphGeneration, String> {
    let graph_request = Arc::new(agent_semantic_search::SearchGenerationGraphRequest::new(
        &authority.content_search_generation,
        authority.source_snapshot.clone(),
        authority.workspace_generation.clone(),
        owner_paths,
        graph_relations,
    )?);
    agent_semantic_search::build_resident_graph_generation(graph_request)
}
