// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Open, hydrate, and build operations for the workspace Search data plane.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use memmap2::{Mmap, MmapOptions};
use tokio::fs;

use super::search_index_projection::{
    RuntimeDerivedAttachmentBuildTiming, SearchMerkleOwnerRecord, SearchOwnerLocatorRecord,
    SearchOwnerRecord, WorkspaceSearchGenerationDataPlaneClient,
    build_admitted_owner_search_indexes, build_merkle_search_generation,
    build_resident_byte_coverage_index, build_resident_graph_generation,
    build_resident_grep_corpus, elapsed_micros, workspace_search_generation_segment_path,
};
use super::{
    SearchGenerationSectionKind, ValidatedSearchGenerationSegment, ValidatedSortedRecordTable,
    WorkspaceGenerationPointerReader, WorkspaceMemoryGeneration,
    WorkspaceSearchGenerationAuthority,
};

#[derive(Debug, Default)]
struct WorkspaceSearchGenerationDataPlaneCell {
    client: tokio::sync::OnceCell<Arc<WorkspaceSearchGenerationDataPlaneClient>>,
}

static SEARCH_GENERATION_DATA_PLANE_CELLS: std::sync::OnceLock<
    parking_lot::RwLock<BTreeMap<std::path::PathBuf, Arc<WorkspaceSearchGenerationDataPlaneCell>>>,
> = std::sync::OnceLock::new();

fn search_generation_data_plane_cell(
    pointer_path: &Path,
) -> Arc<WorkspaceSearchGenerationDataPlaneCell> {
    let cells = SEARCH_GENERATION_DATA_PLANE_CELLS.get_or_init(Default::default);
    if let Some(cell) = cells.read().get(pointer_path) {
        return Arc::clone(cell);
    }
    let mut cells = cells.write();
    Arc::clone(
        cells
            .entry(pointer_path.to_path_buf())
            .or_insert_with(|| Arc::new(WorkspaceSearchGenerationDataPlaneCell::default())),
    )
}

fn graph_entry_owner_index(
    owners: &BTreeMap<String, Arc<SearchOwnerRecord>>,
) -> Result<BTreeMap<String, String>, String> {
    let mut index = BTreeMap::new();
    for (owner_path, owner) in owners {
        let entries = std::iter::once(agent_semantic_search::stable_graph_node_id(
            "owner", owner_path,
        ))
        .chain(owner.selectors.iter().map(|selector| {
            agent_semantic_search::stable_graph_node_id("item", &selector.selector)
        }));
        for node_id in entries {
            if let Some(previous) = index.insert(node_id.clone(), owner_path.clone())
                && previous != *owner_path
            {
                return Err(format!(
                    "graph entry node id collision between owners {previous} and {owner_path}: {node_id}"
                ));
            }
        }
    }
    Ok(index)
}

fn owned_relation_index(
    owners: &BTreeMap<String, Arc<SearchOwnerRecord>>,
    relations: &[crate::ClientDbSourceIndexOwnedRelation],
) -> Result<BTreeMap<String, Arc<[crate::ClientDbSourceIndexOwnedRelation]>>, String> {
    let mut index = BTreeMap::<String, Vec<_>>::new();
    for relation in relations {
        relation.relation.validate()?;
        if !owners.contains_key(relation.owner_path.as_str()) {
            return Err(format!(
                "topology relation owner is absent from generation: {}",
                relation.owner_path
            ));
        }
        index
            .entry(relation.owner_path.to_string())
            .or_default()
            .push(relation.clone());
    }
    Ok(index
        .into_iter()
        .map(|(owner, mut relations)| {
            relations.sort();
            (owner, Arc::from(relations))
        })
        .collect())
}

impl WorkspaceSearchGenerationDataPlaneClient {
    /// Open one immutable Search generation through the process-wide pointer
    /// cell. Admission validation, Runtime observation, and the first query
    /// therefore share the exact same mmap and decoded locator plane.
    pub async fn open(pointer_path: &Path, project_root: &Path) -> Result<Arc<Self>, String> {
        let cell = search_generation_data_plane_cell(pointer_path);
        let client = cell
            .client
            .get_or_try_init(|| async {
                Self::open_inner(pointer_path, project_root)
                    .await
                    .map(Arc::new)
            })
            .await?;
        if client.project_root != project_root.to_string_lossy() {
            return Err("workspace Search generation project root drift".to_owned());
        }
        Ok(Arc::clone(client))
    }

    pub(crate) fn invalidate_committed_pointer(pointer_path: &Path) {
        if let Some(cells) = SEARCH_GENERATION_DATA_PLANE_CELLS.get() {
            cells.write().remove(pointer_path);
        }
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

    #[cfg(test)]
    pub(crate) fn from_generation(
        generation: Arc<WorkspaceMemoryGeneration>,
    ) -> Result<Self, String> {
        generation.validate()?;
        Self::from_admitted_generation(generation)
    }

    /// Build the immutable Search plane from a generation already admitted by
    /// `WorkspaceMemoryBackend`. This deliberately avoids a second full digest
    /// validation; publication or durable restore owns that boundary.
    pub(crate) fn from_admitted_generation(
        generation: Arc<WorkspaceMemoryGeneration>,
    ) -> Result<Self, String> {
        let total_started = std::time::Instant::now();
        let project_root = generation.project_root.clone();
        let sort_started = std::time::Instant::now();
        let mut owners = generation.owners.iter().enumerate().collect::<Vec<_>>();
        owners.sort_by(|(_, left), (_, right)| left.owner_path.cmp(&right.owner_path));
        let sort_micros = elapsed_micros(sort_started);
        let merkle_started = std::time::Instant::now();
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
        let merkle_micros = elapsed_micros(merkle_started);
        let manifest_started = std::time::Instant::now();
        let search_projection_manifest = build_merkle_search_generation(&generation)?;
        let manifest_micros = elapsed_micros(manifest_started);
        let authority_started = std::time::Instant::now();
        let authority =
            WorkspaceSearchGenerationAuthority::from_generation_with_projection_digests(
                &generation,
                format!("blake3-256:{}", merkle_tree.root_digest().as_str()),
                search_projection_manifest.manifest_digest().to_owned(),
            )?;
        authority.validate_binding(&authority.project_id, &generation.workspace_identity)?;
        let authority_micros = elapsed_micros(authority_started);

        let owner_records_started = std::time::Instant::now();
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
                    native_syntax_diagnostic: owner.native_syntax_diagnostic.clone(),
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
        let owner_records_micros = elapsed_micros(owner_records_started);
        let relation_records_started = std::time::Instant::now();
        let owned_relations_by_owner =
            owned_relation_index(&owner_directory_records, &generation.relations)?;
        let mut graph_relation_records = BTreeMap::<(String, String), Vec<_>>::new();
        for owned in &generation.relations {
            let relation = &owned.relation;
            relation.validate()?;
            graph_relation_records
                .entry((relation.from.kind.to_string(), relation.from.id.clone()))
                .or_default()
                .push(relation.clone());
        }
        let relation_records_micros = elapsed_micros(relation_records_started);
        let owner_search_started = std::time::Instant::now();
        let (source_documents, callable_selector_by_owner) =
            build_admitted_owner_search_indexes(&owner_directory_records)?;
        let topology_index = std::sync::OnceLock::new();
        topology_index
            .set(super::search_index_projection::build_topology_index(
                &owner_directory_records,
            ))
            .map_err(|_| "resident topology index initialized twice".to_owned())?;
        let provider_authorities = provider_authorities(&source_documents);
        let graph_entry_owner_by_node_id = graph_entry_owner_index(&owner_directory_records)?;
        let owner_search_micros = elapsed_micros(owner_search_started);
        let corpus_digest =
            resident_grep_corpus_digest(owners.iter().map(|(_, owner)| owner.bytes.as_slice()));
        let owner_locator_records = owner_directory_records
            .iter()
            .map(|(owner_path, record)| {
                (
                    owner_path.clone(),
                    Arc::new(SearchOwnerLocatorRecord {
                        owner_path: owner_path.clone(),
                        authority: record.authority.clone(),
                        content_digest: record.content_digest.clone(),
                        byte_offset: record.byte_offset,
                        byte_length: record.byte_length,
                        line_count: record.line_count,
                        corpus_digest: corpus_digest.clone(),
                    }),
                )
            })
            .collect();
        let byte_coverage_started = std::time::Instant::now();
        let resident_byte_coverage =
            agent_semantic_search::ResidentByteCoverageIndex::new(owners.iter().map(
                |(_, owner)| agent_semantic_search::ResidentByteCoverageInput {
                    owner_path: owner.owner_path.clone(),
                    authority: owner.authority.clone(),
                    bytes: owner.bytes.as_slice(),
                },
            ))?;
        let byte_coverage_micros = elapsed_micros(byte_coverage_started);
        let grep_corpus_started = std::time::Instant::now();
        let resident_grep_corpus = agent_semantic_search::build_resident_grep_corpus(
            &authority
                .content_search_generation
                .content_generation_digest,
            owners.iter().map(
                |(_, owner)| agent_semantic_search::ResidentGrepCorpusOwner {
                    owner_path: &owner.owner_path,
                    content_digest: &owner.content_digest,
                    bytes: &owner.bytes,
                },
            ),
        )?;
        let grep_corpus_micros = elapsed_micros(grep_corpus_started);
        eprintln!(
            "[resident-search-plane-admission-timing] ownerCount={} sortMicros={} merkleMicros={} manifestMicros={} authorityMicros={} ownerRecordsMicros={} relationRecordsMicros={} ownerSearchMicros={} byteCoverageMicros={} grepCorpusMicros={} totalMicros={}",
            generation.owners.len(),
            sort_micros,
            merkle_micros,
            manifest_micros,
            authority_micros,
            owner_records_micros,
            relation_records_micros,
            owner_search_micros,
            byte_coverage_micros,
            grep_corpus_micros,
            elapsed_micros(total_started),
        );
        drop(owners);
        Ok(Self {
            mapping: None,
            resident_generation: Some(generation),
            resident_owner_positions,
            authority,
            project_root,
            owner_directory_records,
            owner_locator_records,
            mapped_owner_record_table_range: None,
            mapped_merkle_owner_table_range: None,
            mapped_graph_relation_table_range: None,
            source_documents,
            provider_authorities,
            resident_byte_coverage,
            resident_grep_corpus,
            callable_selector_by_owner,
            topology_index,
            graph_entry_owner_by_node_id,
            owner_bytes_range: None,
            merkle_owner_records,
            owned_relations_by_owner,
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
        let total_started = std::time::Instant::now();
        let mapping = Arc::new(mapping);
        let segment_started = std::time::Instant::now();
        let segment = ValidatedSearchGenerationSegment::parse(mapping.as_ref())?;
        let (evidence, _, _) = segment.section(SearchGenerationSectionKind::GenerationEvidence);
        let authority: WorkspaceSearchGenerationAuthority = serde_json::from_slice(evidence)
            .map_err(|error| format!("decode workspace search generation evidence: {error}"))?;
        authority.validate_binding(&authority.project_id, expected_workspace_identity)?;
        if segment.epoch() != expected_epoch
            || authority.generation_digest != expected_generation_digest
        {
            return Err("workspace search generation pointer binding mismatch".to_owned());
        }
        let segment_micros = elapsed_micros(segment_started);
        let owners_started = std::time::Instant::now();
        let (owner_directory_bytes, _, _) =
            segment.section(SearchGenerationSectionKind::OwnerDirectory);
        let owner_locator_records = ValidatedSortedRecordTable::parse(owner_directory_bytes)?
            .owned_records()?
            .into_iter()
            .map(|(key, value)| {
                let key = String::from_utf8(key).map_err(|error| {
                    format!("workspace owner-directory key is not UTF-8: {error}")
                })?;
                let record: SearchOwnerLocatorRecord = serde_json::from_slice(&value)
                    .map_err(|error| format!("decode workspace owner locator: {error}"))?;
                if record.owner_path != key {
                    return Err("workspace owner locator key drift".to_owned());
                }
                Ok((key, Arc::new(record)))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let owners_micros = elapsed_micros(owners_started);
        let owner_bytes_range = segment.section_range(SearchGenerationSectionKind::OwnerBytes);
        let byte_coverage_started = std::time::Instant::now();
        let resident_byte_coverage = build_resident_byte_coverage_index(
            Arc::clone(&mapping),
            &owner_bytes_range,
            &owner_locator_records,
        )?;
        let byte_coverage_micros = elapsed_micros(byte_coverage_started);
        let grep_started = std::time::Instant::now();
        let resident_grep_corpus = build_resident_grep_corpus(
            Arc::clone(&mapping),
            &owner_bytes_range,
            &owner_locator_records,
            &authority
                .content_search_generation
                .content_generation_digest,
        )?;
        let grep_micros = elapsed_micros(grep_started);
        let merkle_micros = 0;
        let graph_micros = 0;
        let topology_micros = 0;
        let owner_directory_records = BTreeMap::new();
        let source_documents = owner_locator_records
            .values()
            .map(|record| agent_semantic_search::ResidentSourceDocument {
                owner_path: record.owner_path.clone(),
                authority: record.authority.clone(),
                owner_content_digest: record.content_digest.clone(),
                line_count: record.line_count,
                query_keys: Vec::new(),
            })
            .collect::<Vec<_>>();
        let provider_authorities = provider_authorities(&source_documents);
        let callable_selector_by_owner = BTreeMap::new();
        let topology_index = std::sync::OnceLock::new();
        let graph_entry_owner_by_node_id = BTreeMap::new();
        let merkle_owner_records = BTreeMap::new();
        let owned_relations_by_owner = BTreeMap::new();
        let graph_relation_records = BTreeMap::new();
        let mapped_owner_record_table_range =
            segment.section_range(SearchGenerationSectionKind::SelectorIndex);
        let mapped_merkle_owner_table_range =
            segment.section_range(SearchGenerationSectionKind::MerkleOwnerIndex);
        let mapped_graph_relation_table_range =
            segment.section_range(SearchGenerationSectionKind::GraphRelations);
        let graph_generation = Arc::new(tokio::sync::OnceCell::new());
        let lexical_accelerator = Arc::new(tokio::sync::OnceCell::new());
        eprintln!(
            "[mapped-search-plane-open-timing] ownerCount={} segmentMicros={} ownersMicros={} byteCoverageMicros={} grepCorpusMicros={} merkleMicros={} graphMicros={} topologyMicros={} totalMicros={}",
            owner_locator_records.len(),
            segment_micros,
            owners_micros,
            byte_coverage_micros,
            grep_micros,
            merkle_micros,
            graph_micros,
            topology_micros,
            elapsed_micros(total_started),
        );
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
            owner_locator_records,
            mapped_owner_record_table_range: Some(mapped_owner_record_table_range),
            mapped_merkle_owner_table_range: Some(mapped_merkle_owner_table_range),
            mapped_graph_relation_table_range: Some(mapped_graph_relation_table_range),
            source_documents,
            provider_authorities,
            resident_byte_coverage,
            resident_grep_corpus,
            callable_selector_by_owner,
            topology_index,
            graph_entry_owner_by_node_id,
            owner_bytes_range: Some(owner_bytes_range),
            merkle_owner_records,
            owned_relations_by_owner,
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
            let mapped_owners;
            let mapped_relations;
            let (owners, relations) = if self.mapping.is_some() {
                mapped_owners = self.mapped_all_owner_records()?;
                mapped_relations = self.mapped_all_owned_relations()?;
                (
                    &mapped_owners,
                    mapped_relations
                        .iter()
                        .map(|owned| owned.relation.clone())
                        .collect(),
                )
            } else {
                (
                    &self.owner_directory_records,
                    self.graph_relation_records
                        .values()
                        .flatten()
                        .cloned()
                        .collect(),
                )
            };
            build_resident_graph_generation(&self.authority, owners, relations)
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
        let documents = self.lexical_source_documents()?;
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
}

fn provider_authorities(
    source_documents: &[agent_semantic_search::ResidentSourceDocument],
) -> BTreeMap<String, BTreeSet<String>> {
    let mut authorities = BTreeMap::<String, BTreeSet<String>>::new();
    for authority in source_documents
        .iter()
        .filter_map(|document| document.authority.as_ref())
    {
        authorities
            .entry(authority.language_id.as_str().to_owned())
            .or_default()
            .insert(authority.provider_id.as_str().to_owned());
    }
    authorities
}

fn resident_grep_corpus_digest<'a>(owners: impl IntoIterator<Item = &'a [u8]>) -> String {
    let mut hasher = blake3::Hasher::new();
    for bytes in owners {
        hasher.update(bytes);
        if !bytes.ends_with(b"\n") {
            hasher.update(b"\n");
        }
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}
