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
    RuntimeDerivedAttachmentBuildTiming, SearchMerkleOwnerRecord, SearchOwnerRecord,
    WorkspaceSearchGenerationDataPlaneClient, build_admitted_owner_search_indexes,
    build_merkle_search_generation, build_owner_search_indexes, build_resident_byte_coverage_index,
    build_resident_graph_generation, build_resident_grep_corpus, elapsed_micros, graph_key,
    workspace_search_generation_segment_path,
};
use super::{
    SearchGenerationSectionKind, ValidatedSearchGenerationSegment, ValidatedSortedRecordTable,
    WorkspaceGenerationPointerReader, WorkspaceMemoryGeneration,
    WorkspaceSearchGenerationAuthority,
};

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
        let owned_relations: Arc<[crate::ClientDbSourceIndexOwnedRelation]> =
            Arc::from(generation.relations.clone());
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
        let owner_search_micros = elapsed_micros(owner_search_started);
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
            source_documents,
            resident_byte_coverage,
            resident_grep_corpus,
            callable_selector_by_owner,
            owner_bytes_range: None,
            merkle_owner_records,
            owned_relations,
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
        let mapping = Arc::new(mapping);
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
        let (owner_directory_bytes, _, _) =
            segment.section(SearchGenerationSectionKind::OwnerDirectory);
        let owner_directory_records = ValidatedSortedRecordTable::parse(owner_directory_bytes)?
            .owned_records()?
            .into_iter()
            .map(|(key, value)| {
                let key = String::from_utf8(key).map_err(|error| {
                    format!("workspace owner-directory key is not UTF-8: {error}")
                })?;
                let record: SearchOwnerRecord = serde_json::from_slice(&value)
                    .map_err(|error| format!("decode workspace owner record: {error}"))?;
                if record.owner_path != key {
                    return Err("workspace owner record key drift".to_owned());
                }
                Ok((key, Arc::new(record)))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let owner_bytes_range = segment.section_range(SearchGenerationSectionKind::OwnerBytes);
        let resident_byte_coverage = build_resident_byte_coverage_index(
            Arc::clone(&mapping),
            &owner_bytes_range,
            &owner_directory_records,
        )?;
        let resident_grep_corpus = build_resident_grep_corpus(
            Arc::clone(&mapping),
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
                if graph_key(relation.from.kind.as_str(), &relation.from.id) != key {
                    return Err("workspace search graph relation key drift".to_owned());
                }
                graph_relation_records
                    .entry((relation.from.kind.to_string(), relation.from.id.clone()))
                    .or_default()
                    .push(relation.clone());
            }
            graph_relations.extend(owned_relations);
        }
        let owned_relations: Arc<[crate::ClientDbSourceIndexOwnedRelation]> =
            Arc::from(graph_relations.clone());
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
            resident_grep_corpus,
            callable_selector_by_owner,
            owner_bytes_range: Some(owner_bytes_range),
            merkle_owner_records,
            owned_relations,
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
                &self.owner_directory_records,
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
