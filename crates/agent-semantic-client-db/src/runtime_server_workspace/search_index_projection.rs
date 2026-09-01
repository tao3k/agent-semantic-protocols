use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
    mapping: Mmap,
    authority: WorkspaceSearchGenerationAuthority,
    owner_directory_records: BTreeMap<String, Vec<u8>>,
    resident_source_index: agent_semantic_search::ResidentSourceIndex,
    callable_selector_by_owner: BTreeMap<String, String>,
    owner_record_cache: Mutex<HashMap<String, Arc<SearchOwnerRecord>>>,
    owner_bytes_range: std::ops::Range<usize>,
    merkle_owner_records: BTreeMap<String, Vec<u8>>,
    graph_relation_records: BTreeMap<
        (String, String),
        Vec<agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation>,
    >,
    graph_generation: agent_semantic_search::ResidentGraphGeneration,
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
    let changes = owners
        .into_iter()
        .map(|owner| {
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
                    agent_semantic_search::resident_lexical_coverage_keys(
                        &owner.owner_path,
                        &owner.bytes,
                        owner
                            .selectors
                            .iter()
                            .flat_map(|selector| selector.query_keys.iter().cloned()),
                    ),
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
    encode_workspace_search_generation_segment_inner(generation)
}

fn encode_workspace_search_generation_segment_inner(
    generation: &WorkspaceMemoryGeneration,
) -> Result<Vec<u8>, String> {
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
    );
    authority.validate_binding(&generation.workspace_identity, &generation.project_root)?;
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
        let query_keys = agent_semantic_search::resident_lexical_coverage_keys(
            &owner.owner_path,
            &owner.bytes,
            owner
                .selectors
                .iter()
                .flat_map(|selector| selector.query_keys.iter().cloned()),
        );
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
    encode_search_generation_segment(
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
    )
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_workspace/merkle_owner_index.rs"]
mod merkle_owner_index_tests;

type OwnerSearchIndexes = (
    agent_semantic_search::ResidentSourceIndex,
    BTreeMap<String, String>,
);

fn build_owner_search_indexes(
    owner_directory_records: &BTreeMap<String, Vec<u8>>,
    graph_relations: &[crate::ClientDbSourceIndexOwnedRelation],
    authority: &WorkspaceSearchGenerationAuthority,
) -> Result<OwnerSearchIndexes, String> {
    let records = owner_directory_records
        .iter()
        .map(|(key, value)| {
            let record: SearchOwnerRecord = serde_json::from_slice(value)
                .map_err(|error| format!("decode workspace search owner record: {error}"))?;
            if record.owner_path != *key {
                return Err("workspace search owner record key drift".to_owned());
            }
            Ok((key.clone(), record))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let mut graph_relations_by_owner = records
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
    let mut changes = Vec::with_capacity(records.len());
    for (key, record) in records {
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
            agent_semantic_search::ResidentSourceIndexSeed {
                owner_path: record.owner_path.clone(),
                authority: record.authority.clone(),
                owner_content_digest: record.content_digest.clone(),
                line_count: record.line_count,
                query_keys: record.query_keys.clone(),
            },
        );
        changes.push(agent_semantic_search::SearchOwnerChange::Added {
            fragment: agent_semantic_search::SearchOwnerFragment::new(
                record.owner_path,
                record.content_digest,
                record.line_count,
                record.query_keys,
                Some(agent_semantic_search::search_owner_graph_fragment_digest(
                    graph_relations_by_owner
                        .get(&key)
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
            workspace_identity: authority.workspace_identity.clone(),
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
    let resident_source_index = manifest.resident_source_index_from_seeds(
        authority.source_snapshot.clone(),
        authority.generation_digest.clone(),
        seeds,
    )?;
    Ok((resident_source_index, callable_selectors))
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
        let segment = ValidatedSearchGenerationSegment::parse(&mapping)?;
        let (evidence, _, _) = segment.section(SearchGenerationSectionKind::GenerationEvidence);
        let authority: WorkspaceSearchGenerationAuthority = serde_json::from_slice(evidence)
            .map_err(|error| format!("decode workspace search generation evidence: {error}"))?;
        authority.validate_binding(
            &snapshot.workspace_identity,
            project_root
                .to_str()
                .ok_or_else(|| "workspace search project root is not UTF-8".to_owned())?,
        )?;
        if segment.epoch() != snapshot.active_epoch
            || authority.generation_digest != snapshot.generation_digest
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
                Ok((key, value))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let owner_bytes_range = segment.section_range(SearchGenerationSectionKind::OwnerBytes);
        let (merkle_owner_bytes, _, _) =
            segment.section(SearchGenerationSectionKind::MerkleOwnerIndex);
        let merkle_owner_records = ValidatedSortedRecordTable::parse(merkle_owner_bytes)?
            .owned_records()?
            .into_iter()
            .map(|(key, value)| {
                let key = String::from_utf8(key).map_err(|error| {
                    format!("workspace search Merkle-owner key is not UTF-8: {error}")
                })?;
                Ok((key, value))
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
        let (resident_source_index, callable_selector_by_owner) =
            build_owner_search_indexes(&owner_directory_records, &graph_relations, &authority)?;
        let graph_generation = agent_semantic_search::build_resident_graph_generation(
            &authority.source_snapshot,
            &authority.workspace_generation,
            owner_directory_records.keys().cloned(),
            graph_relations.iter().map(|owned| owned.relation.clone()),
        )?;
        Ok(Self {
            mapping,
            authority,
            owner_directory_records,
            resident_source_index,
            callable_selector_by_owner,
            owner_record_cache: Mutex::new(HashMap::new()),
            owner_bytes_range,
            merkle_owner_records,
            graph_relation_records,
            graph_generation,
        })
    }

    pub fn authority(&self) -> &WorkspaceSearchGenerationAuthority {
        &self.authority
    }

    pub fn graph_generation(&self) -> &agent_semantic_search::ResidentGraphGeneration {
        &self.graph_generation
    }

    pub fn read_source_index(
        &self,
        query: &str,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        self.resident_source_index.query(query, authority, limit)
    }

    pub fn read_source_index_for_language(
        &self,
        query: &str,
        language_id: &agent_semantic_client_core::LanguageId,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        self.resident_source_index
            .query_language(query, language_id, limit)
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

    fn resident_owner_record(
        &self,
        owner_path: &str,
    ) -> Result<Option<Arc<SearchOwnerRecord>>, String> {
        let mut cache = self
            .owner_record_cache
            .lock()
            .map_err(|_| "workspace search owner-record cache is poisoned".to_owned())?;
        if let Some(record) = cache.get(owner_path) {
            return Ok(Some(Arc::clone(record)));
        }
        let Some(bytes) = self.owner_directory_records.get(owner_path) else {
            return Ok(None);
        };
        let record: Arc<SearchOwnerRecord> = Arc::new(
            serde_json::from_slice(bytes)
                .map_err(|error| format!("decode workspace search owner record: {error}"))?,
        );
        cache.insert(owner_path.to_owned(), Arc::clone(&record));
        Ok(Some(record))
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
                workspace_identity: self.authority.workspace_identity.clone(),
                project_root: self.authority.project_root.clone(),
                active_epoch: self.authority.active_epoch,
                generation_digest: self.authority.generation_digest.clone(),
                root_digest,
                owner_path: owner_path.to_owned(),
            });
        };
        let record: SearchMerkleOwnerRecord = serde_json::from_slice(value)
            .map_err(|error| format!("decode workspace search Merkle owner: {error}"))?;
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
            workspace_identity: self.authority.workspace_identity.clone(),
            project_root: self.authority.project_root.clone(),
            active_epoch: self.authority.active_epoch,
            generation_digest: self.authority.generation_digest.clone(),
            root_digest,
            owner_path: record.owner_path,
            source_blob_digest: record.source_blob_digest,
            owner_subtree_digest: record.owner_subtree_digest,
            inclusion_proof: record.inclusion_proof,
        })
    }

    pub fn read_owner(&self, owner_path: &str) -> Result<super::WorkspaceRuntimeOwnerRead, String> {
        let Some(record) = self.resident_owner_record(owner_path)? else {
            return Ok(super::WorkspaceRuntimeOwnerRead::OwnerMissing {
                generation_digest: self.authority.generation_digest.clone(),
                root_digest: self.authority.source_snapshot.root_digest.clone(),
            });
        };
        let start = self
            .owner_bytes_range
            .start
            .checked_add(record.byte_offset as usize)
            .ok_or_else(|| "workspace search owner byte offset overflows".to_owned())?;
        let end = start
            .checked_add(record.byte_length as usize)
            .ok_or_else(|| "workspace search owner byte range overflows".to_owned())?;
        if end > self.owner_bytes_range.end {
            return Err("workspace search owner bytes exceed section bounds".to_owned());
        }
        let owner_bytes = self
            .mapping
            .get(start..end)
            .ok_or_else(|| "workspace search owner bytes exceed section bounds".to_owned())?;
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
