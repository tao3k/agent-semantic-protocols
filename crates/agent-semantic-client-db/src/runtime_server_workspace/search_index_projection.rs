// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use memmap2::Mmap;
use serde::{Deserialize, Serialize};

#[cfg(test)]
use super::ValidatedSearchGenerationSegment;
use super::{
    SearchGenerationSection, SearchGenerationSectionKind, SearchGenerationSectionRepresentation,
    ValidatedSortedRecordTable, WorkspaceMemoryGeneration, WorkspaceSearchGenerationAuthority,
    encode_search_generation_segment, encode_sorted_record_table,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SearchOwnerRecord {
    pub(super) owner_path: String,
    pub(super) authority: Option<agent_semantic_search::ResidentSearchAuthority>,
    pub(super) content_digest: String,
    pub(super) native_syntax_diagnostic: Option<agent_semantic_search::NativeSyntaxDiagnostic>,
    pub(super) byte_offset: u64,
    pub(super) byte_length: u64,
    pub(super) line_count: u32,
    pub(super) query_keys: Vec<String>,
    pub(super) selectors: Vec<super::WorkspaceSelectorSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SearchMerkleOwnerRecord {
    pub(super) owner_path: String,
    pub(super) source_blob_digest: String,
    pub(super) owner_subtree_digest: String,
    pub(super) inclusion_proof:
        Vec<agent_semantic_content_identity::exact_selector_merkle::MerkleInclusionStepV1>,
}

#[derive(Debug)]
pub struct WorkspaceSearchGenerationDataPlaneClient {
    pub(super) mapping: Option<Arc<Mmap>>,
    pub(super) resident_generation: Option<Arc<WorkspaceMemoryGeneration>>,
    pub(super) resident_owner_positions: BTreeMap<String, usize>,
    pub(super) authority: WorkspaceSearchGenerationAuthority,
    pub(super) project_root: String,
    pub(super) owner_directory_records: BTreeMap<String, Arc<SearchOwnerRecord>>,
    pub(super) source_documents: Vec<agent_semantic_search::ResidentSourceDocument>,
    pub(super) resident_byte_coverage: agent_semantic_search::ResidentByteCoverageIndex,
    pub(super) resident_grep_corpus: agent_semantic_search::ResidentGrepCorpusArtifact,
    pub(super) callable_selector_by_owner: BTreeMap<String, String>,
    pub(super) owner_bytes_range: Option<std::ops::Range<usize>>,
    pub(super) merkle_owner_records: BTreeMap<String, Arc<SearchMerkleOwnerRecord>>,
    pub(super) owned_relations: Arc<[crate::ClientDbSourceIndexOwnedRelation]>,
    pub(super) graph_relation_records: BTreeMap<
        (String, String),
        Vec<agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation>,
    >,
    pub(super) graph_generation: Arc<
        tokio::sync::OnceCell<Result<agent_semantic_search::ResidentGraphGeneration, String>>,
    >,
    pub(super) lexical_accelerator:
        Arc<tokio::sync::OnceCell<Result<agent_semantic_search::ResidentSourceIndex, String>>>,
}

impl WorkspaceSearchGenerationDataPlaneClient {
    pub(super) fn lexical_source_documents(
        &self,
    ) -> Result<BTreeMap<String, agent_semantic_search::ResidentSourceDocument>, String> {
        let mut documents = self
            .source_documents
            .iter()
            .cloned()
            .map(|document| (document.owner_path.clone(), document))
            .collect::<BTreeMap<_, _>>();
        for document in documents.values_mut() {
            let record = self
                .owner_directory_records
                .get(&document.owner_path)
                .ok_or_else(|| {
                    format!(
                        "Tantivy attachment owner record is missing: {}",
                        document.owner_path
                    )
                })?;
            document.lexical_body =
                Some(String::from_utf8_lossy(self.resident_owner_bytes(record)?).into_owned());
        }
        Ok(documents)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeDerivedAttachmentBuildTiming {
    pub build_micros: u64,
    pub finalize_micros: u64,
}

pub(super) fn elapsed_micros(started: std::time::Instant) -> u64 {
    started.elapsed().as_micros().try_into().unwrap_or(u64::MAX)
}

pub fn workspace_search_generation_segment_path(generation_path: &Path) -> PathBuf {
    generation_path.with_extension("search.mmap")
}

pub(super) fn build_merkle_search_generation(
    generation: &WorkspaceMemoryGeneration,
) -> Result<agent_semantic_search::MerkleSearchGeneration, String> {
    let mut graph_relations_by_owner = BTreeMap::<String, Vec<_>>::new();
    for owner in &generation.owners {
        graph_relations_by_owner.insert(owner.owner_path.clone(), Vec::new());
    }
    for owned in &generation.relations {
        let relation = &owned.relation;
        relation.validate()?;
        let owner_path = owned.owner_path.as_str();
        graph_relations_by_owner
            .get_mut(owner_path)
            .ok_or_else(|| {
                format!("workspace search graph relation owner is absent: {owner_path}")
            })?
            .push(relation.clone());
    }

    let mut owners = generation.owners.iter().collect::<Vec<_>>();
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    let coverage_inputs = owners
        .iter()
        .map(
            |owner| agent_semantic_search::ResidentLexicalCoverageInput {
                owner_path: &owner.owner_path,
                source: &owner.bytes,
                parser_query_keys: owner
                    .selectors
                    .iter()
                    .flat_map(|selector| selector.query_keys.iter().cloned())
                    .collect(),
            },
        )
        .collect::<Vec<_>>();
    let lexical_coverage = agent_semantic_search::resident_lexical_coverage_batch(&coverage_inputs);
    let changes = owners
        .into_iter()
        .zip(lexical_coverage)
        .map(|(owner, lexical_query_keys)| {
            let text = std::str::from_utf8(&owner.bytes).unwrap_or_default();
            let graph_fragment_digest = agent_semantic_search::search_owner_graph_fragment_digest(
                graph_relations_by_owner
                    .get(&owner.owner_path)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
            )?;
            Ok(agent_semantic_search::SearchOwnerChange::Added {
                fragment: agent_semantic_search::SearchOwnerFragment::new(
                    owner.owner_path.clone(),
                    owner.content_digest.clone(),
                    text.lines().count().max(1).min(u32::MAX as usize) as u32,
                    lexical_query_keys,
                    Some(graph_fragment_digest),
                )?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    agent_semantic_search::MerkleSearchGeneration::apply_change_set(
        None,
        agent_semantic_search::SearchProjectionIdentity {
            workspace_identity: generation.workspace_identity.clone(),
            source_root_digest: generation.source_snapshot.root_digest.clone(),
            provider_digest: generation.source_snapshot.provider_digest.clone(),
            schema_digest: generation.provider_schema_digest.clone(),
            analyzer_digest: agent_semantic_search::search_projection_analyzer_digest(),
        },
        changes,
    )
}

pub fn encode_workspace_search_generation_segment(
    generation: &WorkspaceMemoryGeneration,
) -> Result<Vec<u8>, String> {
    Ok(encode_workspace_search_generation_segment_with_authority(generation)?.bytes)
}

pub(super) struct EncodedWorkspaceSearchGeneration {
    pub bytes: Vec<u8>,
    pub authority: WorkspaceSearchGenerationAuthority,
}

pub(super) fn encode_workspace_search_generation_segment_with_authority(
    generation: &WorkspaceMemoryGeneration,
) -> Result<EncodedWorkspaceSearchGeneration, String> {
    generation.validate()?;
    let mut owners = generation.owners.iter().collect::<Vec<_>>();
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    let merkle_tree =
        agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1::from_file_digests(
            owners.iter().map(|owner| {
                (
                    owner.owner_path.clone(),
                    agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                        &owner.bytes,
                    ),
                )
            }),
        )
        .map_err(|error| format!("build workspace search Merkle owner index: {error}"))?;
    let search_projection_manifest = build_merkle_search_generation(generation)?;
    let authority = WorkspaceSearchGenerationAuthority::from_generation_with_projection_digests(
        generation,
        format!("blake3-256:{}", merkle_tree.root_digest().as_str()),
        search_projection_manifest.manifest_digest().to_owned(),
    )?;
    authority.validate_binding(&authority.project_id, &generation.workspace_identity)?;
    let evidence = serde_json::to_vec(&authority)
        .map_err(|error| format!("encode workspace search generation evidence: {error}"))?;
    let project_resolutions = serde_json::to_vec(&generation.project_resolutions)
        .map_err(|error| format!("encode workspace search project resolutions: {error}"))?;
    let merkle_records = owners
        .iter()
        .map(|owner| {
            let source_blob_digest = merkle_tree
                .source_blob_digest(&owner.owner_path)
                .ok_or_else(|| {
                    format!(
                        "workspace search Merkle tree omitted owner source digest: ownerPath={}",
                        owner.owner_path
                    )
                })?;
            let owner_subtree_digest = merkle_tree
                .owner_subtree_digest(&owner.owner_path)
                .ok_or_else(|| {
                    format!(
                        "workspace search Merkle tree omitted owner subtree: ownerPath={}",
                        owner.owner_path
                    )
                })?;
            let inclusion_proof =
                merkle_tree
                    .inclusion_proof(&owner.owner_path)
                    .ok_or_else(|| {
                        format!(
                            "workspace search Merkle tree omitted owner proof: ownerPath={}",
                            owner.owner_path
                        )
                    })?;
            let record = SearchMerkleOwnerRecord {
                owner_path: owner.owner_path.clone(),
                source_blob_digest: source_blob_digest.as_str().to_owned(),
                owner_subtree_digest: owner_subtree_digest.as_str().to_owned(),
                inclusion_proof,
            };
            let source_blob_digest =
                agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1(
                    &record.source_blob_digest,
                )
                .map_err(|error| {
                    format!(
                        "workspace search Merkle owner source digest is invalid: ownerPath={} error={error}",
                        owner.owner_path
                    )
                })?;
            let owner_subtree_digest =
                agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1(
                    &record.owner_subtree_digest,
                )
                .map_err(|error| {
                    format!(
                        "workspace search Merkle owner subtree digest is invalid: ownerPath={} error={error}",
                        owner.owner_path
                    )
                })?;
            let root_digest = merkle_tree.root_digest();
            if !agent_semantic_content_identity::workspace_merkle_v1::verify_owner_inclusion_v1(
                agent_semantic_content_identity::workspace_merkle_v1::WorkspaceOwnerInclusionV1 {
                    owner_path: &record.owner_path,
                    source_blob_digest: &source_blob_digest,
                    expected_owner_subtree_digest: &owner_subtree_digest,
                    inclusion_proof: &record.inclusion_proof,
                    expected_workspace_root_digest: root_digest,
                },
            ) {
                return Err(format!(
                    "workspace search Merkle owner proof self-check failed: ownerPath={}",
                    owner.owner_path
                ));
            }
            Ok((
                owner.owner_path.as_bytes().to_vec(),
                serde_json::to_vec(&record)
                    .map_err(|error| format!("encode workspace search Merkle owner: {error}"))?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut owner_bytes = Vec::new();
    let mut owner_records = Vec::with_capacity(owners.len());
    let mut selectors = Vec::<(Vec<u8>, Vec<u8>)>::new();
    for owner in &owners {
        let text = std::str::from_utf8(&owner.bytes).unwrap_or_default();
        let query_keys = search_projection_manifest
            .owner(&owner.owner_path)
            .ok_or_else(|| {
                format!(
                    "workspace search projection manifest omitted owner: {}",
                    owner.owner_path
                )
            })?
            .lexical_query_keys
            .clone();
        for (position, selector) in owner.selectors.iter().enumerate() {
            selectors.push((
                selector.selector.as_bytes().to_vec(),
                serde_json::to_vec(&(owner.owner_path.as_str(), position))
                    .map_err(|error| format!("encode workspace search selector owner: {error}"))?,
            ));
        }
        let byte_offset = owner_bytes.len() as u64;
        let byte_length = owner.bytes.len() as u64;
        owner_bytes.extend_from_slice(&owner.bytes);
        let record = SearchOwnerRecord {
            owner_path: owner.owner_path.clone(),
            authority: owner.authority.clone(),
            content_digest: owner.content_digest.clone(),
            native_syntax_diagnostic: owner.native_syntax_diagnostic.clone(),
            byte_offset,
            byte_length,
            line_count: text.lines().count().max(1).min(u32::MAX as usize) as u32,
            query_keys,
            selectors: owner.selectors.clone(),
        };
        owner_records.push((
            owner.owner_path.as_bytes().to_vec(),
            serde_json::to_vec(&record)
                .map_err(|error| format!("encode workspace owner record: {error}"))?,
        ));
    }
    let (_, byte_coverage_artifact) =
        agent_semantic_search::ResidentByteCoverageIndex::encode_artifact(owners.iter().map(
            |owner| agent_semantic_search::ResidentByteCoverageInput {
                owner_path: owner.owner_path.clone(),
                authority: owner.authority.clone(),
                bytes: owner.bytes.as_slice(),
            },
        ))?;
    owner_bytes.extend_from_slice(&byte_coverage_artifact);
    let mut graph = BTreeMap::<Vec<u8>, Vec<_>>::new();
    for owned in &generation.relations {
        let relation = &owned.relation;
        graph
            .entry(graph_key(relation.from.kind.as_str(), &relation.from.id))
            .or_default()
            .push(owned.clone());
    }
    let graph_records = graph
        .into_iter()
        .map(|(key, relations)| {
            serde_json::to_vec(&relations)
                .map(|value| (key, value))
                .map_err(|error| format!("encode workspace search graph relations: {error}"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let owner_directory = encode_sorted_record_table(owner_records)?;
    let selector_index = encode_sorted_record_table(selectors)?;
    let graph_relations = encode_sorted_record_table(graph_records)?;
    let merkle_owner_index = encode_sorted_record_table(merkle_records)?;
    let selector_count = ValidatedSortedRecordTable::parse(&selector_index)?.len();
    let graph_count = ValidatedSortedRecordTable::parse(&graph_relations)?.len();
    let bytes = encode_search_generation_segment(
        generation.active_epoch,
        vec![
            section(
                SearchGenerationSectionKind::GenerationEvidence,
                SearchGenerationSectionRepresentation::Utf8Json,
                1,
                evidence,
            ),
            section(
                SearchGenerationSectionKind::ProjectResolutions,
                SearchGenerationSectionRepresentation::Utf8Json,
                generation.project_resolutions.len(),
                project_resolutions,
            ),
            section(
                SearchGenerationSectionKind::OwnerDirectory,
                SearchGenerationSectionRepresentation::SortedOffsetTable,
                generation.owners.len(),
                owner_directory,
            ),
            section(
                SearchGenerationSectionKind::OwnerBytes,
                SearchGenerationSectionRepresentation::OpaqueBytes,
                generation.owners.len(),
                owner_bytes,
            ),
            section(
                SearchGenerationSectionKind::SelectorIndex,
                SearchGenerationSectionRepresentation::SortedOffsetTable,
                selector_count,
                selector_index,
            ),
            section(
                SearchGenerationSectionKind::GraphRelations,
                SearchGenerationSectionRepresentation::SortedOffsetTable,
                graph_count,
                graph_relations,
            ),
            section(
                SearchGenerationSectionKind::MerkleOwnerIndex,
                SearchGenerationSectionRepresentation::SortedOffsetTable,
                generation.owners.len(),
                merkle_owner_index,
            ),
        ],
    )?;
    Ok(EncodedWorkspaceSearchGeneration { bytes, authority })
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_workspace/merkle_owner_index.rs"]
mod merkle_owner_index_tests;

type OwnerSearchIndexes = (
    Vec<agent_semantic_search::ResidentSourceDocument>,
    BTreeMap<String, String>,
);

pub(super) fn build_admitted_owner_search_indexes(
    owner_directory_records: &BTreeMap<String, Arc<SearchOwnerRecord>>,
) -> Result<OwnerSearchIndexes, String> {
    let mut source_documents = Vec::with_capacity(owner_directory_records.len());
    let mut callable_selectors = BTreeMap::new();
    for (key, record) in owner_directory_records {
        if record.owner_path != *key {
            return Err("workspace owner record key drift".to_owned());
        }
        if let Some(selector) = record.selectors.iter().find_map(|selector| {
            selector
                .derived_projections
                .iter()
                .any(|projection| {
                    projection.projection_kind == super::ExactProjectionKind::CallableSkeleton
                })
                .then(|| selector.selector.clone())
        }) {
            callable_selectors.insert(key.clone(), selector);
        }
        source_documents.push(agent_semantic_search::ResidentSourceDocument {
            owner_path: record.owner_path.clone(),
            authority: record.authority.clone(),
            owner_content_digest: record.content_digest.clone(),
            line_count: record.line_count,
            query_keys: record.query_keys.clone(),
            lexical_body: None,
        });
    }
    Ok((source_documents, callable_selectors))
}

pub(super) fn build_owner_search_indexes(
    owner_directory_records: &BTreeMap<String, Arc<SearchOwnerRecord>>,
    graph_relations: &[crate::ClientDbSourceIndexOwnedRelation],
    authority: &WorkspaceSearchGenerationAuthority,
) -> Result<OwnerSearchIndexes, String> {
    let mut graph_relations_by_owner = owner_directory_records
        .keys()
        .map(|owner_path| (owner_path.clone(), Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for owned in graph_relations {
        let relation = &owned.relation;
        relation.validate()?;
        let owner_path = owned.owner_path.as_str();
        graph_relations_by_owner
            .get_mut(owner_path)
            .ok_or_else(|| {
                format!("workspace search graph relation owner is absent: {owner_path}")
            })?
            .push(relation.clone());
    }

    let mut seeds = BTreeMap::new();
    let mut callable_selectors = BTreeMap::new();
    let mut changes = Vec::with_capacity(owner_directory_records.len());
    for (key, record) in owner_directory_records {
        if record.owner_path != *key {
            return Err("workspace owner record key drift".to_owned());
        }
        let callable_selector = record.selectors.iter().find_map(|selector| {
            selector
                .derived_projections
                .iter()
                .any(|projection| {
                    projection.projection_kind == super::ExactProjectionKind::CallableSkeleton
                })
                .then(|| selector.selector.clone())
        });
        seeds.insert(
            key.clone(),
            agent_semantic_search::ResidentSourceDocument {
                owner_path: record.owner_path.clone(),
                authority: record.authority.clone(),
                owner_content_digest: record.content_digest.clone(),
                line_count: record.line_count,
                query_keys: record.query_keys.clone(),
                lexical_body: None,
            },
        );
        changes.push(agent_semantic_search::SearchOwnerChange::Added {
            fragment: agent_semantic_search::SearchOwnerFragment::new(
                record.owner_path.clone(),
                record.content_digest.clone(),
                record.line_count,
                record.query_keys.clone(),
                Some(agent_semantic_search::search_owner_graph_fragment_digest(
                    graph_relations_by_owner
                        .get(key)
                        .map(Vec::as_slice)
                        .unwrap_or_default(),
                )?),
            )?,
        });
        if let Some(selector) = callable_selector {
            callable_selectors.insert(key.clone(), selector);
        }
    }
    let manifest = agent_semantic_search::MerkleSearchGeneration::apply_change_set(
        None,
        agent_semantic_search::SearchProjectionIdentity {
            workspace_identity: authority.workspace_id.clone(),
            source_root_digest: authority.source_snapshot.root_digest.clone(),
            provider_digest: authority.source_snapshot.provider_digest.clone(),
            schema_digest: authority.provider_schema_digest.clone(),
            analyzer_digest: authority.search_projection_analyzer_digest.clone(),
        },
        changes,
    )?;
    if manifest.manifest_digest() != authority.search_projection_manifest_digest {
        return Err("workspace search projection manifest digest mismatch".to_owned());
    }
    Ok((seeds.into_values().collect(), callable_selectors))
}

pub(super) fn build_resident_byte_coverage_index(
    mapping: Arc<Mmap>,
    owner_bytes_range: &std::ops::Range<usize>,
    owner_directory_records: &BTreeMap<String, Arc<SearchOwnerRecord>>,
) -> Result<agent_semantic_search::ResidentByteCoverageIndex, String> {
    let mut owners = Vec::with_capacity(owner_directory_records.len());
    let mut owner_payload_len = 0usize;
    for (owner_path, record) in owner_directory_records {
        if record.owner_path != *owner_path {
            return Err("workspace byte-coverage owner key drift".to_owned());
        }
        let relative_start = usize::try_from(record.byte_offset)
            .map_err(|_| "workspace byte-coverage offset exceeds usize".to_owned())?;
        let relative_end = relative_start
            .checked_add(
                usize::try_from(record.byte_length)
                    .map_err(|_| "workspace byte-coverage length exceeds usize".to_owned())?,
            )
            .ok_or_else(|| "workspace byte-coverage range overflows".to_owned())?;
        owner_payload_len = owner_payload_len.max(relative_end);
        let start = owner_bytes_range
            .start
            .checked_add(relative_start)
            .ok_or_else(|| "workspace byte-coverage offset overflows".to_owned())?;
        let end = owner_bytes_range
            .start
            .checked_add(relative_end)
            .ok_or_else(|| "workspace byte-coverage range overflows".to_owned())?;
        if end > owner_bytes_range.end {
            return Err("workspace byte-coverage bytes exceed section bounds".to_owned());
        }
        mapping
            .get(start..end)
            .ok_or_else(|| "workspace byte-coverage bytes exceed mapping".to_owned())?;
        owners.push(agent_semantic_search::ResidentByteCoverageOwner {
            owner_path: record.owner_path.clone(),
            authority: record.authority.clone(),
        });
    }
    let artifact_start = owner_bytes_range
        .start
        .checked_add(owner_payload_len)
        .ok_or_else(|| "workspace byte-coverage artifact offset overflows".to_owned())?;
    if artifact_start == owner_bytes_range.end {
        return Err("workspace search generation has no resident trigram artifact".to_owned());
    }
    agent_semantic_search::ResidentByteCoverageIndex::from_mapped_artifact(
        owners,
        mapping,
        artifact_start..owner_bytes_range.end,
    )
}

#[path = "search_index_projection_builders.rs"]
mod builders;
pub(super) use builders::{build_resident_graph_generation, build_resident_grep_corpus};

#[path = "search_index_projection_reads.rs"]
mod reads;

fn section(
    kind: SearchGenerationSectionKind,
    representation: SearchGenerationSectionRepresentation,
    record_count: usize,
    bytes: Vec<u8>,
) -> SearchGenerationSection {
    SearchGenerationSection {
        kind,
        representation: representation as u8,
        record_count: record_count as u64,
        bytes,
    }
}

pub(super) fn graph_key(kind: &str, id: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(kind.len() + id.len() + 1);
    key.extend_from_slice(kind.as_bytes());
    key.push(0);
    key.extend_from_slice(id.as_bytes());
    key
}
