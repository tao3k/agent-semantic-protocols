use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use memmap2::{Mmap, MmapOptions};
use serde::{Deserialize, Serialize};
use tokio::fs;

use super::{
    SearchGenerationSection, SearchGenerationSectionKind, SearchGenerationSectionRepresentation,
    ValidatedSearchGenerationSegment, ValidatedSortedRecordTable, WorkspaceGenerationPointerReader,
    WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceSearchGenerationAuthority,
    encode_search_generation_segment, encode_sorted_record_table,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SearchOwnerRecord {
    owner_path: String,
    authority: Option<agent_semantic_search::ResidentSearchAuthority>,
    content_digest: String,
    byte_offset: u64,
    byte_length: u64,
    line_count: u32,
    query_keys: Vec<String>,
    selectors: Vec<super::WorkspaceSelectorSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SearchMerkleOwnerRecord {
    owner_path: String,
    source_blob_digest: String,
    owner_subtree_digest: String,
    inclusion_proof:
        Vec<agent_semantic_content_identity::exact_selector_merkle::MerkleInclusionStepV1>,
}

#[derive(Debug)]
pub struct WorkspaceSearchGenerationDataPlaneClient {
    mapping: Option<Mmap>,
    resident_generation: Option<Arc<WorkspaceMemoryGeneration>>,
    resident_owner_positions: BTreeMap<String, usize>,
    authority: WorkspaceSearchGenerationAuthority,
    project_root: String,
    owner_directory_records: BTreeMap<String, Arc<SearchOwnerRecord>>,
    source_documents: Vec<agent_semantic_search::ResidentSourceDocument>,
    resident_byte_coverage: agent_semantic_search::ResidentByteCoverageIndex,
    cold_rg_corpus: agent_semantic_search::ColdRgCorpusArtifact,
    callable_selector_by_owner: BTreeMap<String, String>,
    owner_bytes_range: Option<std::ops::Range<usize>>,
    merkle_owner_records: BTreeMap<String, Arc<SearchMerkleOwnerRecord>>,
    graph_relation_records: BTreeMap<
        (String, String),
        Vec<agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation>,
    >,
    graph_generation: Arc<
        tokio::sync::OnceCell<Result<agent_semantic_search::ResidentGraphGeneration, String>>,
    >,
    lexical_accelerator:
        Arc<tokio::sync::OnceCell<Result<agent_semantic_search::ResidentSourceIndex, String>>>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeDerivedAttachmentBuildTiming {
    pub build_micros: u64,
    pub finalize_micros: u64,
}

fn elapsed_micros(started: std::time::Instant) -> u64 {
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
                &record.owner_path,
                &source_blob_digest,
                &owner_subtree_digest,
                &record.inclusion_proof,
                root_digest,
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
    for owner in owners {
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
            byte_offset,
            byte_length,
            line_count: text.lines().count().max(1).min(u32::MAX as usize) as u32,
            query_keys,
            selectors: owner.selectors.clone(),
        };
        owner_records.push((
            owner.owner_path.as_bytes().to_vec(),
            serde_json::to_vec(&record)
                .map_err(|error| format!("encode workspace search owner record: {error}"))?,
        ));
    }
    let mut graph = BTreeMap::<Vec<u8>, Vec<_>>::new();
    for owned in &generation.relations {
        let relation = &owned.relation;
        graph
            .entry(graph_key(&relation.from.kind, &relation.from.id))
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

fn build_owner_search_indexes(
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
            return Err("workspace search owner record key drift".to_owned());
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

fn build_resident_byte_coverage_index(
    mapping: &[u8],
    owner_bytes_range: &std::ops::Range<usize>,
    owner_directory_records: &BTreeMap<String, Arc<SearchOwnerRecord>>,
) -> Result<agent_semantic_search::ResidentByteCoverageIndex, String> {
    let mut seeds = Vec::with_capacity(owner_directory_records.len());
    for (owner_path, record) in owner_directory_records {
        if record.owner_path != *owner_path {
            return Err("workspace byte-coverage owner key drift".to_owned());
        }
        let start = owner_bytes_range
            .start
            .checked_add(record.byte_offset as usize)
            .ok_or_else(|| "workspace byte-coverage offset overflows".to_owned())?;
        let end = start
            .checked_add(record.byte_length as usize)
            .ok_or_else(|| "workspace byte-coverage range overflows".to_owned())?;
        if end > owner_bytes_range.end {
            return Err("workspace byte-coverage bytes exceed section bounds".to_owned());
        }
        let bytes = mapping
            .get(start..end)
            .ok_or_else(|| "workspace byte-coverage bytes exceed mapping".to_owned())?;
        seeds.push(agent_semantic_search::ResidentByteCoverageInput {
            owner_path: record.owner_path.clone(),
            authority: record.authority.clone(),
            bytes,
        });
    }
    Ok(agent_semantic_search::ResidentByteCoverageIndex::new(seeds))
}

fn build_cold_rg_corpus(
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

fn build_resident_graph_generation(
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

impl WorkspaceSearchGenerationDataPlaneClient {
    pub async fn open(pointer_path: &Path, project_root: &Path) -> Result<Self, String> {
        Self::open_inner(pointer_path, project_root).await
    }

    async fn open_inner(pointer_path: &Path, project_root: &Path) -> Result<Self, String> {
        let pointer = WorkspaceGenerationPointerReader::open(pointer_path).await?;
        let snapshot = pointer.read()?;
        snapshot.validate()?;
        let path = workspace_search_generation_segment_path(Path::new(&snapshot.mmap_segment_path));
        let file = fs::File::open(&path)
            .await
            .map_err(|error| format!("open workspace search generation segment: {error}"))?;
        let file = file.into_std().await;
        let mapping = tokio::task::spawn_blocking(move || unsafe {
            MmapOptions::new()
                .map(&file)
                .map_err(|error| format!("map workspace search generation segment: {error}"))
        })
        .await
        .map_err(|error| format!("map workspace search generation task failed: {error}"))??;
        Self::from_mapping(
            mapping,
            project_root,
            snapshot.active_epoch,
            &snapshot.workspace_identity,
            &snapshot.generation_digest,
        )
    }

    pub(crate) fn from_generation(
        generation: Arc<WorkspaceMemoryGeneration>,
    ) -> Result<Self, String> {
        generation.validate()?;
        let project_root = generation.project_root.clone();
        let mut owners = generation.owners.iter().enumerate().collect::<Vec<_>>();
        owners.sort_by(|(_, left), (_, right)| left.owner_path.cmp(&right.owner_path));
        let merkle_tree = agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1::from_file_digests(
            owners.iter().map(|(_, owner)| {
                (
                    owner.owner_path.clone(),
                    agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                        &owner.bytes,
                    ),
                )
            }),
        )
        .map_err(|error| format!("build resident workspace Merkle owner index: {error}"))?;
        let search_projection_manifest = build_merkle_search_generation(&generation)?;
        let authority =
            WorkspaceSearchGenerationAuthority::from_generation_with_projection_digests(
                &generation,
                format!("blake3-256:{}", merkle_tree.root_digest().as_str()),
                search_projection_manifest.manifest_digest().to_owned(),
            )?;
        authority.validate_binding(&authority.project_id, &generation.workspace_identity)?;

        let mut owner_directory_records = BTreeMap::new();
        let mut resident_owner_positions = BTreeMap::new();
        let mut merkle_owner_records = BTreeMap::new();
        for (position, owner) in &owners {
            let text = std::str::from_utf8(&owner.bytes).unwrap_or_default();
            let query_keys = search_projection_manifest
                .owner(&owner.owner_path)
                .ok_or_else(|| {
                    format!(
                        "resident search projection omitted owner: {}",
                        owner.owner_path
                    )
                })?
                .lexical_query_keys
                .clone();
            owner_directory_records.insert(
                owner.owner_path.clone(),
                Arc::new(SearchOwnerRecord {
                    owner_path: owner.owner_path.clone(),
                    authority: owner.authority.clone(),
                    content_digest: owner.content_digest.clone(),
                    byte_offset: 0,
                    byte_length: owner.bytes.len() as u64,
                    line_count: text.lines().count().max(1).min(u32::MAX as usize) as u32,
                    query_keys,
                    selectors: owner.selectors.clone(),
                }),
            );
            resident_owner_positions.insert(owner.owner_path.clone(), *position);
            let source_blob_digest = merkle_tree
                .source_blob_digest(&owner.owner_path)
                .ok_or_else(|| "resident Merkle source digest is missing".to_owned())?;
            let owner_subtree_digest = merkle_tree
                .owner_subtree_digest(&owner.owner_path)
                .ok_or_else(|| "resident Merkle owner subtree is missing".to_owned())?;
            let inclusion_proof = merkle_tree
                .inclusion_proof(&owner.owner_path)
                .ok_or_else(|| "resident Merkle inclusion proof is missing".to_owned())?;
            merkle_owner_records.insert(
                owner.owner_path.clone(),
                Arc::new(SearchMerkleOwnerRecord {
                    owner_path: owner.owner_path.clone(),
                    source_blob_digest: source_blob_digest.as_str().to_owned(),
                    owner_subtree_digest: owner_subtree_digest.as_str().to_owned(),
                    inclusion_proof,
                }),
            );
        }
        let mut graph_relation_records = BTreeMap::<(String, String), Vec<_>>::new();
        for owned in &generation.relations {
            let relation = &owned.relation;
            relation.validate()?;
            graph_relation_records
                .entry((relation.from.kind.clone(), relation.from.id.clone()))
                .or_default()
                .push(relation.clone());
        }
        let (source_documents, callable_selector_by_owner) = build_owner_search_indexes(
            &owner_directory_records,
            &generation.relations,
            &authority,
        )?;
        let resident_byte_coverage =
            agent_semantic_search::ResidentByteCoverageIndex::new(owners.iter().map(
                |(_, owner)| agent_semantic_search::ResidentByteCoverageInput {
                    owner_path: owner.owner_path.clone(),
                    authority: owner.authority.clone(),
                    bytes: owner.bytes.as_slice(),
                },
            ));
        let cold_rg_corpus = agent_semantic_search::build_cold_rg_corpus(
            &authority
                .content_search_generation
                .content_generation_digest,
            owners
                .iter()
                .map(|(_, owner)| agent_semantic_search::ColdRgCorpusOwner {
                    owner_path: &owner.owner_path,
                    content_digest: &owner.content_digest,
                    bytes: &owner.bytes,
                }),
        )?;
        drop(owners);
        Ok(Self {
            mapping: None,
            resident_generation: Some(generation),
            resident_owner_positions,
            authority,
            project_root,
            owner_directory_records,
            source_documents,
            resident_byte_coverage,
            cold_rg_corpus,
            callable_selector_by_owner,
            owner_bytes_range: None,
            merkle_owner_records,
            graph_relation_records,
            graph_generation: Arc::new(tokio::sync::OnceCell::new()),
            lexical_accelerator: Arc::new(tokio::sync::OnceCell::new()),
        })
    }

    fn from_mapping(
        mapping: Mmap,
        project_root: &Path,
        expected_epoch: u64,
        expected_workspace_identity: &str,
        expected_generation_digest: &str,
    ) -> Result<Self, String> {
        let segment = ValidatedSearchGenerationSegment::parse(&mapping)?;
        let (evidence, _, _) = segment.section(SearchGenerationSectionKind::GenerationEvidence);
        let authority: WorkspaceSearchGenerationAuthority = serde_json::from_slice(evidence)
            .map_err(|error| format!("decode workspace search generation evidence: {error}"))?;
        authority.validate_binding(&authority.project_id, expected_workspace_identity)?;
        if segment.epoch() != expected_epoch
            || authority.generation_digest != expected_generation_digest
        {
            return Err("workspace search generation pointer binding mismatch".to_owned());
        }
        let (owner_directory_bytes, _, _) =
            segment.section(SearchGenerationSectionKind::OwnerDirectory);
        let owner_directory_records = ValidatedSortedRecordTable::parse(owner_directory_bytes)?
            .owned_records()?
            .into_iter()
            .map(|(key, value)| {
                let key = String::from_utf8(key).map_err(|error| {
                    format!("workspace search owner-directory key is not UTF-8: {error}")
                })?;
                let record: SearchOwnerRecord = serde_json::from_slice(&value)
                    .map_err(|error| format!("decode workspace search owner record: {error}"))?;
                if record.owner_path != key {
                    return Err("workspace search owner record key drift".to_owned());
                }
                Ok((key, Arc::new(record)))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let owner_bytes_range = segment.section_range(SearchGenerationSectionKind::OwnerBytes);
        let resident_byte_coverage = build_resident_byte_coverage_index(
            &mapping,
            &owner_bytes_range,
            &owner_directory_records,
        )?;
        let cold_rg_corpus = build_cold_rg_corpus(
            &mapping,
            &owner_bytes_range,
            &owner_directory_records,
            &authority
                .content_search_generation
                .content_generation_digest,
        )?;
        let (merkle_owner_bytes, _, _) =
            segment.section(SearchGenerationSectionKind::MerkleOwnerIndex);
        let merkle_owner_records = ValidatedSortedRecordTable::parse(merkle_owner_bytes)?
            .owned_records()?
            .into_iter()
            .map(|(key, value)| {
                let key = String::from_utf8(key).map_err(|error| {
                    format!("workspace search Merkle-owner key is not UTF-8: {error}")
                })?;
                let record: SearchMerkleOwnerRecord = serde_json::from_slice(&value)
                    .map_err(|error| format!("decode workspace search Merkle owner: {error}"))?;
                if record.owner_path != key {
                    return Err("workspace search Merkle owner key drift".to_owned());
                }
                Ok((key, Arc::new(record)))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let (graph_relation_bytes, _, _) =
            segment.section(SearchGenerationSectionKind::GraphRelations);
        let graph_records =
            ValidatedSortedRecordTable::parse(graph_relation_bytes)?.owned_records()?;
        let mut graph_relations = Vec::<crate::ClientDbSourceIndexOwnedRelation>::new();
        let mut graph_relation_records = BTreeMap::<
            (String, String),
            Vec<agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation>,
        >::new();
        for (key, value) in graph_records {
            let owned_relations =
                serde_json::from_slice::<Vec<crate::ClientDbSourceIndexOwnedRelation>>(&value)
                    .map_err(|error| format!("decode workspace search graph relations: {error}"))?;
            for owned in &owned_relations {
                let relation = &owned.relation;
                if graph_key(&relation.from.kind, &relation.from.id) != key {
                    return Err("workspace search graph relation key drift".to_owned());
                }
                graph_relation_records
                    .entry((relation.from.kind.clone(), relation.from.id.clone()))
                    .or_default()
                    .push(relation.clone());
            }
            graph_relations.extend(owned_relations);
        }
        let (source_documents, callable_selector_by_owner) =
            build_owner_search_indexes(&owner_directory_records, &graph_relations, &authority)?;
        let graph_generation = Arc::new(tokio::sync::OnceCell::new());
        let lexical_accelerator = Arc::new(tokio::sync::OnceCell::new());
        Ok(Self {
            mapping: Some(mapping),
            resident_generation: None,
            resident_owner_positions: BTreeMap::new(),
            authority,
            project_root: project_root
                .to_str()
                .ok_or_else(|| "workspace search project root is not UTF-8".to_owned())?
                .to_owned(),
            owner_directory_records,
            source_documents,
            resident_byte_coverage,
            cold_rg_corpus,
            callable_selector_by_owner,
            owner_bytes_range: Some(owner_bytes_range),
            merkle_owner_records,
            graph_relation_records,
            graph_generation,
            lexical_accelerator,
        })
    }

    pub fn authority(&self) -> &WorkspaceSearchGenerationAuthority {
        &self.authority
    }

    pub fn graph_generation(
        &self,
    ) -> Result<Option<&agent_semantic_search::ResidentGraphGeneration>, String> {
        match self.graph_generation.get() {
            None => Ok(None),
            Some(Ok(generation)) => Ok(Some(generation)),
            Some(Err(error)) => Err(error.clone()),
        }
    }

    #[must_use]
    pub fn graph_generation_is_ready(&self) -> bool {
        matches!(self.graph_generation.get(), Some(Ok(_)))
    }

    #[must_use]
    pub fn lexical_accelerator_is_ready(&self) -> bool {
        matches!(self.lexical_accelerator.get(), Some(Ok(_)))
    }

    pub fn build_graph_attachment(
        &self,
        expected_content_generation_digest: &str,
    ) -> Result<RuntimeDerivedAttachmentBuildTiming, String> {
        if let Some(result) = self.graph_generation.get() {
            return result
                .as_ref()
                .map(|_| RuntimeDerivedAttachmentBuildTiming::default())
                .map_err(Clone::clone);
        }
        if self
            .authority
            .content_search_generation
            .content_generation_digest
            != expected_content_generation_digest
        {
            let error = "graph attachment content-generation CAS mismatch".to_owned();
            let _ = self.graph_generation.set(Err(error.clone()));
            return Err(error);
        }
        let build_started = std::time::Instant::now();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            build_resident_graph_generation(
                &self.authority,
                self.owner_directory_records.keys().cloned().collect(),
                self.graph_relation_records
                    .values()
                    .flatten()
                    .cloned()
                    .collect(),
            )
        }))
        .unwrap_or_else(|_| Err("background generation graph builder panicked".to_owned()));
        let build_micros = elapsed_micros(build_started);
        let finalize_started = std::time::Instant::now();
        let timing = RuntimeDerivedAttachmentBuildTiming {
            build_micros,
            finalize_micros: elapsed_micros(finalize_started),
        };
        match result {
            Ok(generation) => {
                let _ = self.graph_generation.set(Ok(generation));
                Ok(timing)
            }
            Err(error) => {
                let _ = self.graph_generation.set(Err(error.clone()));
                Err(error)
            }
        }
    }

    pub fn build_lexical_attachment(
        &self,
        expected_content_generation_digest: &str,
        resources: agent_semantic_search::ResidentIndexBuildResources,
    ) -> Result<RuntimeDerivedAttachmentBuildTiming, String> {
        if let Some(result) = self.lexical_accelerator.get() {
            return result
                .as_ref()
                .map(|_| RuntimeDerivedAttachmentBuildTiming::default())
                .map_err(Clone::clone);
        }
        if self
            .authority
            .content_search_generation
            .content_generation_digest
            != expected_content_generation_digest
        {
            let error = "Tantivy attachment content-generation CAS mismatch".to_owned();
            let _ = self.lexical_accelerator.set(Err(error.clone()));
            return Err(error);
        }
        let documents = self
            .source_documents
            .iter()
            .cloned()
            .map(|document| (document.owner_path.clone(), document))
            .collect::<BTreeMap<_, _>>();
        let build_started = std::time::Instant::now();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            agent_semantic_search::ResidentSourceIndex::new(
                documents,
                self.authority.source_snapshot.clone(),
                self.authority.generation_digest.clone(),
                resources,
            )
        }))
        .unwrap_or_else(|_| Err("background Tantivy accelerator builder panicked".to_owned()));
        let build_micros = elapsed_micros(build_started);
        let finalize_started = std::time::Instant::now();
        let timing = RuntimeDerivedAttachmentBuildTiming {
            build_micros,
            finalize_micros: elapsed_micros(finalize_started),
        };
        match result {
            Ok(index) => {
                let _ = self.lexical_accelerator.set(Ok(index));
                Ok(timing)
            }
            Err(error) => {
                let _ = self.lexical_accelerator.set(Err(error.clone()));
                Err(error)
            }
        }
    }

    pub fn fail_derived_attachments(&self, error: &str) {
        let _ = self.graph_generation.set(Err(error.to_owned()));
        let _ = self.lexical_accelerator.set(Err(error.to_owned()));
    }

    pub fn fail_graph_attachment(&self, error: &str) {
        let _ = self.graph_generation.set(Err(error.to_owned()));
    }

    pub fn fail_lexical_attachment(&self, error: &str) {
        let _ = self.lexical_accelerator.set(Err(error.to_owned()));
    }

    #[must_use]
    pub fn derived_build_workload(&self, previous: Option<&Self>) -> (usize, usize, usize) {
        let owner_count = self.source_documents.len();
        let lexical_bytes = self
            .source_documents
            .iter()
            .map(|document| {
                document.owner_path.len()
                    + document.query_keys.iter().map(String::len).sum::<usize>()
            })
            .sum();
        let previous_digests = previous.map(|previous| {
            previous
                .source_documents
                .iter()
                .map(|document| {
                    (
                        document.owner_path.as_str(),
                        document.owner_content_digest.as_str(),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        });
        let changed_owner_count = match previous_digests {
            Some(previous) => {
                let current_owner_paths = self
                    .source_documents
                    .iter()
                    .map(|document| document.owner_path.as_str())
                    .collect::<BTreeSet<_>>();
                let changed_or_added = self
                    .source_documents
                    .iter()
                    .filter(|document| {
                        previous.get(document.owner_path.as_str()).copied()
                            != Some(document.owner_content_digest.as_str())
                    })
                    .count();
                let removed = previous
                    .keys()
                    .filter(|owner_path| !current_owner_paths.contains(**owner_path))
                    .count();
                changed_or_added.saturating_add(removed)
            }
            None => owner_count,
        };
        (owner_count, lexical_bytes, changed_owner_count)
    }

    pub fn read_source_index(
        &self,
        query: &str,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        self.cold_lexical_result(query, None, authority, limit)
    }

    pub fn read_source_index_for_owner_scope(
        &self,
        query: &str,
        owner_path: &str,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        self.cold_lexical_result(query, Some(&[owner_path.to_owned()]), authority, limit)
    }

    pub fn read_cold_rg_candidates(
        &self,
        query: &str,
        owner_paths: &[String],
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        self.cold_lexical_result(query, Some(owner_paths), authority, limit)
    }

    #[must_use]
    pub fn cold_rg_corpus(&self) -> &agent_semantic_search::ColdRgCorpusArtifact {
        &self.cold_rg_corpus
    }

    pub fn read_byte_evidence(
        &self,
        query: &str,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        let candidates = self.resident_byte_coverage.candidate_owner_paths(
            query.as_bytes(),
            authority,
            usize::try_from(limit).map_err(|_| "byte-evidence limit overflows".to_owned())?,
        )?;
        let mut exact_matches = Vec::new();
        for owner_path in candidates {
            let record = self
                .resident_owner_record(&owner_path)?
                .ok_or_else(|| "byte-evidence candidate owner is missing".to_owned())?;
            let bytes = self.resident_owner_bytes(&record)?;
            if bytes
                .windows(query.len())
                .any(|window| window == query.as_bytes())
            {
                exact_matches.push(owner_path);
            }
        }
        self.cold_lexical_result(query, Some(&exact_matches), authority, limit)
    }

    pub fn read_byte_evidence_for_owner_scope(
        &self,
        query: &str,
        owner_path: &str,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        let result = self.read_byte_evidence(query, authority, limit)?;
        let owner_paths = result
            .hits
            .iter()
            .filter(|hit| hit.owner_path == owner_path)
            .map(|hit| hit.owner_path.clone())
            .collect::<Vec<_>>();
        self.cold_lexical_result(query, Some(&owner_paths), authority, limit)
    }

    pub fn read_source_index_for_language(
        &self,
        query: &str,
        language_id: &agent_semantic_client_core::LanguageId,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        let mut authorities = self
            .source_documents
            .iter()
            .filter_map(|document| document.authority.as_ref())
            .filter(|authority| &authority.language_id == language_id);
        let authority = authorities.next().cloned().ok_or_else(|| {
            format!(
                "resident source-index language authority is missing: languageId={}",
                language_id.as_str()
            )
        })?;
        if authorities.any(|candidate| candidate.provider_id != authority.provider_id) {
            return Err(format!(
                "resident source-index language authority is ambiguous: languageId={}",
                language_id.as_str()
            ));
        }
        self.cold_lexical_result(query, None, Some(&authority), limit)
    }

    fn cold_lexical_result(
        &self,
        query: &str,
        admitted_owner_paths: Option<&[String]>,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        if query.trim().is_empty() || !(1..=100).contains(&limit) {
            return Err(
                "cold resident search requires a non-empty query and limit in 1..=100".to_owned(),
            );
        }
        if admitted_owner_paths.is_none()
            && let Some(Ok(accelerator)) = self.lexical_accelerator.get()
        {
            return accelerator.query(query, authority, limit);
        }
        let admitted = admitted_owner_paths.map(|paths| paths.iter().collect::<BTreeSet<_>>());
        let query_terms = agent_semantic_search::source_index_lookup_terms(query)
            .into_iter()
            .filter(|term| !term.chars().any(char::is_whitespace))
            .collect::<BTreeSet<_>>();
        let mut ranked = self
            .source_documents
            .iter()
            .filter(|document| {
                admitted
                    .as_ref()
                    .is_none_or(|paths| paths.contains(&document.owner_path))
                    && authority
                        .is_none_or(|required| document.authority.as_ref() == Some(required))
            })
            .filter_map(|document| {
                let matched_terms = query_terms
                    .iter()
                    .filter(|term| document.query_keys.binary_search(term).is_ok())
                    .cloned()
                    .collect::<Vec<_>>();
                (admitted.is_some() || !matched_terms.is_empty())
                    .then_some((document, matched_terms))
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|(left, left_terms), (right, right_terms)| {
            right_terms
                .len()
                .cmp(&left_terms.len())
                .then_with(|| left.owner_path.cmp(&right.owner_path))
        });
        let hits = ranked
            .into_iter()
            .take(limit as usize)
            .map(|(document, matched_terms)| {
                agent_semantic_search_projection::ResidentSearchHit {
                    owner_path: document.owner_path.clone(),
                    owner_content_digest: document.owner_content_digest.clone(),
                    language_id: document
                        .authority
                        .as_ref()
                        .map(|value| value.language_id.as_str().to_owned()),
                    projection_tier: agent_semantic_search_projection::ResidentSearchProjectionTier::ShallowNavigation,
                    line_count: document.line_count,
                    query_keys: matched_terms,
                    selector: None,
                    score: None,
                }
            })
            .collect();
        Ok(Arc::new(
            agent_semantic_search_projection::ResidentSearchReadyResult::new(
                self.authority.generation_digest.clone(),
                &self.authority.source_snapshot,
                self.authority.search_projection_manifest_digest.clone(),
                hits,
            )?,
        ))
    }

    pub fn parser_owned_callable_selector_pairs(
        &self,
        owner_paths: &[String],
    ) -> Result<Vec<(String, String)>, String> {
        owner_paths
            .iter()
            .map(|owner_path| {
                if !self.owner_directory_records.contains_key(owner_path) {
                    return Err("workspace search result references a missing owner".to_owned());
                }
                Ok(self
                    .callable_selector_by_owner
                    .get(owner_path)
                    .map(|selector| (selector.clone(), owner_path.clone())))
            })
            .collect::<Result<Vec<_>, String>>()
            .map(|pairs| pairs.into_iter().flatten().collect())
    }

    /// Project bounded provider-native syntax facts from the immutable resident generation.
    ///
    /// Owner paths are identities only. A projected owner must carry the parser-owned
    /// selectors, byte ranges, query keys, and derived projection digests that make it
    /// actionable to the single public Search playbook.
    pub fn native_syntax_playbook_projection(
        &self,
        owner_paths: &[String],
    ) -> Result<
        (
            Vec<agent_semantic_search::NativeSyntaxProjection>,
            Vec<agent_semantic_search::NativeSyntaxRelation>,
        ),
        String,
    > {
        let admitted = owner_paths
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut projections = Vec::with_capacity(admitted.len());
        for owner_path in &admitted {
            let record = self
                .resident_owner_record(owner_path)?
                .ok_or_else(|| "native syntax playbook owner is absent".to_owned())?;
            let selectors = record
                .selectors
                .iter()
                .map(|selector| {
                    let derived_projection_bytes =
                        serde_json::to_vec(&selector.derived_projections).map_err(|error| {
                            format!("encode resident native syntax projections: {error}")
                        })?;
                    Ok(agent_semantic_search::NativeSyntaxSelector {
                        selector: selector.selector.clone(),
                        byte_start: selector.byte_start,
                        byte_end: selector.byte_end,
                        query_keys: selector.query_keys.clone(),
                        derived_projection_digest: format!(
                            "blake3-256:{}",
                            blake3::hash(&derived_projection_bytes).to_hex()
                        ),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            if selectors.is_empty() {
                return Err(format!(
                    "native syntax playbook owner has no parser selectors: {owner_path}"
                ));
            }
            projections.push(agent_semantic_search::NativeSyntaxProjection {
                owner_path: record.owner_path.clone(),
                content_digest: record.content_digest.clone(),
                selectors,
            });
        }
        let mut relations = Vec::new();
        for ((owner_path, _), projected_relations) in &self.graph_relation_records {
            if !admitted.contains(owner_path.as_str()) {
                continue;
            }
            for relation in projected_relations {
                let relation_bytes = serde_json::to_vec(relation)
                    .map_err(|error| format!("encode resident native syntax relation: {error}"))?;
                relations.push(agent_semantic_search::NativeSyntaxRelation {
                    owner_path: owner_path.clone(),
                    relation_digest: format!(
                        "blake3-256:{}",
                        blake3::hash(&relation_bytes).to_hex()
                    ),
                });
            }
        }
        projections.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
        relations.sort_by(|left, right| {
            left.owner_path
                .cmp(&right.owner_path)
                .then_with(|| left.relation_digest.cmp(&right.relation_digest))
        });
        Ok((projections, relations))
    }

    #[must_use]
    pub fn indexed_owner_count(&self) -> usize {
        self.owner_directory_records.len()
    }

    #[must_use]
    pub fn indexed_owner_paths(&self) -> Vec<String> {
        self.owner_directory_records.keys().cloned().collect()
    }

    fn resident_owner_record(
        &self,
        owner_path: &str,
    ) -> Result<Option<Arc<SearchOwnerRecord>>, String> {
        Ok(self.owner_directory_records.get(owner_path).map(Arc::clone))
    }

    fn resident_owner_bytes<'a>(&'a self, record: &SearchOwnerRecord) -> Result<&'a [u8], String> {
        if let Some(generation) = &self.resident_generation {
            let position = self
                .resident_owner_positions
                .get(&record.owner_path)
                .ok_or_else(|| "resident owner position is missing".to_owned())?;
            return generation
                .owners
                .get(*position)
                .map(|owner| owner.bytes.as_slice())
                .ok_or_else(|| "resident owner position is out of range".to_owned());
        }
        let owner_bytes_range = self
            .owner_bytes_range
            .as_ref()
            .ok_or_else(|| "mapped owner byte range is missing".to_owned())?;
        let start = owner_bytes_range
            .start
            .checked_add(record.byte_offset as usize)
            .ok_or_else(|| "workspace search owner byte offset overflows".to_owned())?;
        let end = start
            .checked_add(record.byte_length as usize)
            .ok_or_else(|| "workspace search owner byte range overflows".to_owned())?;
        if end > owner_bytes_range.end {
            return Err("workspace search owner bytes exceed section bounds".to_owned());
        }
        self.mapping
            .as_ref()
            .ok_or_else(|| "mapped workspace search generation is missing".to_owned())?
            .get(start..end)
            .ok_or_else(|| "workspace search owner bytes exceed section bounds".to_owned())
    }

    pub fn read_merkle_owner(
        &self,
        owner_path: &str,
    ) -> Result<super::WorkspaceRuntimeMerkleOwnerRead, String> {
        let root_digest = if self
            .authority
            .owner_merkle_root_digest
            .starts_with("blake3-256:")
        {
            self.authority.owner_merkle_root_digest.clone()
        } else {
            format!("blake3-256:{}", self.authority.owner_merkle_root_digest)
        };
        let Some(value) = self.merkle_owner_records.get(owner_path) else {
            return Ok(super::WorkspaceRuntimeMerkleOwnerRead::OwnerMissing {
                schema_id: super::RUNTIME_MERKLE_OWNER_READ_RECEIPT_SCHEMA_ID.to_owned(),
                schema_version: "1".to_owned(),
                workspace_identity: self.authority.workspace_id.clone(),
                project_root: self.project_root.clone(),
                active_epoch: self.authority.active_epoch,
                generation_digest: self.authority.generation_digest.clone(),
                root_digest,
                owner_path: owner_path.to_owned(),
            });
        };
        let record = Arc::clone(value);
        if record.owner_path != owner_path {
            return Err("workspace search Merkle owner key drift".to_owned());
        }
        let source_blob_digest =
            agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1(
                &record.source_blob_digest,
            )
            .map_err(|error| format!("decode workspace search Merkle source digest: {error}"))?;
        let owner_subtree_digest =
            agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1(
                &record.owner_subtree_digest,
            )
            .map_err(|error| format!("decode workspace search Merkle subtree digest: {error}"))?;
        let parsed_root_digest =
            agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1(
                root_digest
                    .strip_prefix("blake3-256:")
                    .unwrap_or(&root_digest),
            )
            .map_err(|error| format!("decode workspace search Merkle root digest: {error}"))?;
        if !agent_semantic_content_identity::workspace_merkle_v1::verify_owner_inclusion_v1(
            &record.owner_path,
            &source_blob_digest,
            &owner_subtree_digest,
            &record.inclusion_proof,
            &parsed_root_digest,
        ) {
            return Err(format!(
                "workspace search Merkle owner proof drift: ownerPath={owner_path}"
            ));
        }
        Ok(super::WorkspaceRuntimeMerkleOwnerRead::Owner {
            schema_id: super::RUNTIME_MERKLE_OWNER_READ_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: self.authority.workspace_id.clone(),
            project_root: self.project_root.clone(),
            active_epoch: self.authority.active_epoch,
            generation_digest: self.authority.generation_digest.clone(),
            root_digest,
            owner_path: record.owner_path.clone(),
            source_blob_digest: record.source_blob_digest.clone(),
            owner_subtree_digest: record.owner_subtree_digest.clone(),
            inclusion_proof: record.inclusion_proof.clone(),
        })
    }

    pub fn read_owner(&self, owner_path: &str) -> Result<super::WorkspaceRuntimeOwnerRead, String> {
        let Some(record) = self.resident_owner_record(owner_path)? else {
            return Ok(super::WorkspaceRuntimeOwnerRead::OwnerMissing {
                generation_digest: self.authority.generation_digest.clone(),
                root_digest: self.authority.source_snapshot.root_digest.clone(),
            });
        };
        let owner_bytes = self.resident_owner_bytes(&record)?;
        Ok(super::WorkspaceRuntimeOwnerRead::Owner {
            generation_digest: self.authority.generation_digest.clone(),
            root_digest: self.authority.source_snapshot.root_digest.clone(),
            owner: WorkspaceOwnerSnapshot {
                authority: None,
                owner_path: record.owner_path.clone(),
                content_digest: record.content_digest.clone(),
                bytes: owner_bytes.to_vec(),
                selectors: record.selectors.clone(),
            },
        })
    }

    pub fn read_graph_facts(
        &self,
        sources: &[crate::workspace_db_ipc::RuntimeGraphFactSource],
    ) -> Result<crate::workspace_db_ipc::RuntimeGraphFactsRead, String> {
        let mut relations = Vec::new();
        for source in sources {
            if let Some(values) = self.graph_relation_records.get(&(
                source.kind.as_str().to_owned(),
                source.id.as_str().to_owned(),
            )) {
                relations.extend(values.iter().cloned());
            }
        }
        crate::workspace_db_ipc::RuntimeGraphFactsRead::new(
            self.authority.generation_digest.clone(),
            relations,
        )
    }
}

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

fn graph_key(kind: &str, id: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(kind.len() + id.len() + 1);
    key.extend_from_slice(kind.as_bytes());
    key.push(0);
    key.extend_from_slice(id.as_bytes());
    key
}
