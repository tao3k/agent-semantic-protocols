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
    lexical_index: BTreeMap<String, Vec<String>>,
    owner_directory_records: BTreeMap<String, Vec<u8>>,
    owner_record_cache: Mutex<HashMap<String, Arc<SearchOwnerRecord>>>,
    owner_bytes_range: std::ops::Range<usize>,
    merkle_owner_records: BTreeMap<String, Vec<u8>>,
    graph_relation_records: BTreeMap<Vec<u8>, Vec<u8>>,
    source_index_query_cache: Vec<
        Mutex<
            Option<(
                SourceIndexQueryCacheKey,
                crate::ClientDbSourceIndexLookupResult,
            )>,
        >,
    >,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceIndexQueryCacheKey {
    query: String,
    language_id: Option<String>,
    limit: u32,
}

pub fn workspace_search_generation_segment_path(generation_path: &Path) -> PathBuf {
    generation_path.with_extension("search.mmap")
}

pub fn encode_workspace_search_generation_segment(
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
    let authority =
        WorkspaceSearchGenerationAuthority::from_generation_with_owner_merkle_root_digest(
            generation,
            format!("blake3-256:{}", merkle_tree.root_digest().as_str()),
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
    let mut lexical = BTreeMap::<String, Vec<String>>::new();
    let mut selectors = Vec::<(Vec<u8>, Vec<u8>)>::new();
    for owner in owners {
        let text = std::str::from_utf8(&owner.bytes).unwrap_or_default();
        let query_keys = crate::source_index::source_query_keys(&owner.owner_path, text);
        for key in &query_keys {
            lexical
                .entry(key.clone())
                .or_default()
                .push(owner.owner_path.clone());
        }
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
    let lexical_records = lexical
        .into_iter()
        .map(|(term, paths)| Ok((term.into_bytes(), encode_string_list(&paths)?)))
        .collect::<Result<Vec<_>, String>>()?;
    let mut graph = BTreeMap::<Vec<u8>, Vec<_>>::new();
    for relation in &generation.relations {
        graph
            .entry(graph_key(&relation.from.kind, &relation.from.id))
            .or_default()
            .push(relation.clone());
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
    let lexical_index = encode_sorted_record_table(lexical_records)?;
    let selector_index = encode_sorted_record_table(selectors)?;
    let graph_relations = encode_sorted_record_table(graph_records)?;
    let merkle_owner_index = encode_sorted_record_table(merkle_records)?;
    let lexical_count = ValidatedSortedRecordTable::parse(&lexical_index)?.len();
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
                SearchGenerationSectionKind::LexicalIndex,
                SearchGenerationSectionRepresentation::SortedOffsetTable,
                lexical_count,
                lexical_index,
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

impl WorkspaceSearchGenerationDataPlaneClient {
    pub async fn open(pointer_path: &Path, project_root: &Path) -> Result<Self, String> {
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
        let (lexical_bytes, _, _) = segment.section(SearchGenerationSectionKind::LexicalIndex);
        let lexical_index = ValidatedSortedRecordTable::parse(lexical_bytes)?
            .owned_records()?
            .into_iter()
            .map(|(key, value)| {
                let key = String::from_utf8(key).map_err(|error| {
                    format!("workspace search lexical key is not UTF-8: {error}")
                })?;
                Ok((key, decode_string_list(&value)?))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
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
        let graph_relation_records = ValidatedSortedRecordTable::parse(graph_relation_bytes)?
            .owned_records()?
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        Ok(Self {
            mapping,
            authority,
            lexical_index,
            owner_directory_records,
            owner_record_cache: Mutex::new(HashMap::new()),
            owner_bytes_range,
            merkle_owner_records,
            graph_relation_records,
            source_index_query_cache: (0..64).map(|_| Mutex::new(None)).collect(),
        })
    }

    pub fn authority(&self) -> &WorkspaceSearchGenerationAuthority {
        &self.authority
    }

    pub fn read_source_index(
        &self,
        query: &str,
        language_id: Option<&agent_semantic_client_core::LanguageId>,
        limit: u32,
    ) -> Result<crate::ClientDbSourceIndexLookupResult, String> {
        let cache_key = SourceIndexQueryCacheKey {
            query: query.trim().to_ascii_lowercase(),
            language_id: language_id.map(ToString::to_string),
            limit,
        };
        let cache_identity = format!(
            "{}\0{}\0{}",
            cache_key.query,
            cache_key.language_id.as_deref().unwrap_or_default(),
            cache_key.limit
        );
        let cache_digest = blake3::hash(cache_identity.as_bytes());
        let cache_slot = usize::from(u16::from_le_bytes([
            cache_digest.as_bytes()[0],
            cache_digest.as_bytes()[1],
        ])) % self.source_index_query_cache.len();
        if let Some((stored_key, stored_result)) = self.source_index_query_cache[cache_slot]
            .lock()
            .map_err(|_| "workspace search query cache is poisoned".to_owned())?
            .as_ref()
        {
            if stored_key == &cache_key {
                return Ok(stored_result.clone());
            }
        }
        let query_terms = crate::source_index::source_query_keys("", query);
        let normalized = query.trim();
        let paths: Vec<String> = if !normalized.is_empty()
            && normalized.chars().all(|character| {
                character.is_alphanumeric() || character == '-' || character == '_'
            })
            && normalized.contains(['-', '_'])
        {
            self.lexical_index
                .get(&normalized.to_ascii_lowercase())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .take(limit as usize)
                .collect()
        } else {
            let mut scores = HashMap::<String, usize>::new();
            for term in query_terms {
                if let Some(paths) = self.lexical_index.get(&term) {
                    for path in paths {
                        *scores.entry(path.clone()).or_default() += 1;
                    }
                }
            }
            let mut ranked = scores.into_iter().collect::<Vec<_>>();
            ranked.sort_unstable_by(|left, right| {
                right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0))
            });
            ranked.truncate(limit as usize);
            ranked.into_iter().map(|(path, _)| path).collect()
        };
        let candidates = paths
            .into_iter()
            .map(|path| self.source_index_candidate(&path, language_id))
            .collect::<Result<Vec<_>, String>>()?;
        let result = crate::ClientDbSourceIndexLookupResult {
            db_path: PathBuf::new(),
            state: if candidates.is_empty()
                && self.authority.workspace_generation.owner_count == 0
                && self.authority.workspace_generation.leaf_count == 0
            {
                crate::ClientDbSourceIndexLookupState::ColdRequired
            } else if candidates.is_empty() {
                crate::ClientDbSourceIndexLookupState::Miss
            } else {
                crate::ClientDbSourceIndexLookupState::Hit
            },
            candidates,
            source_snapshot: Some(self.authority.source_snapshot.clone()),
            index_artifact_digest: Some(crate::client_db_source_index_artifact_digest(
                &self.authority.source_snapshot,
            )),
        };
        self.source_index_query_cache[cache_slot]
            .lock()
            .map_err(|_| "workspace search query cache is poisoned".to_owned())?
            .replace((cache_key, result.clone()));
        Ok(result)
    }

    pub fn parser_owned_callable_selector_pairs(
        &self,
        owner_paths: &[String],
    ) -> Result<Vec<(String, String)>, String> {
        let mut pairs = Vec::new();
        for owner_path in owner_paths {
            let record = self
                .resident_owner_record(owner_path)?
                .ok_or_else(|| "workspace search result references a missing owner".to_owned())?;
            if let Some(selector) = record.selectors.iter().find(|selector| {
                selector.derived_projections.iter().any(|projection| {
                    projection.projection_kind == super::ExactProjectionKind::CallableSkeleton
                })
            }) {
                pairs.push((selector.selector.clone(), owner_path.clone()));
            }
        }
        Ok(pairs)
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
            if let Some(value) = self
                .graph_relation_records
                .get(&graph_key(&source.kind, &source.id))
            {
                let mut decoded = serde_json::from_slice(value)
                    .map_err(|error| format!("decode workspace search graph relations: {error}"))?;
                relations.append(&mut decoded);
            }
        }
        crate::workspace_db_ipc::RuntimeGraphFactsRead::new(
            self.authority.generation_digest.clone(),
            relations,
        )
    }

    fn source_index_candidate(
        &self,
        owner_path: &str,
        language_id: Option<&agent_semantic_client_core::LanguageId>,
    ) -> Result<crate::ClientDbSourceIndexCandidate, String> {
        let record = self.resident_owner_record(owner_path)?.ok_or_else(|| {
            "workspace search lexical index references a missing owner".to_owned()
        })?;
        Ok(crate::ClientDbSourceIndexCandidate {
            path: record.owner_path.clone().into(),
            language_id: language_id.cloned(),
            provider_id: None,
            source_kind: crate::ClientDbSourceIndexSourceKind::File,
            line_count: Some(record.line_count),
            query_keys: record.query_keys.iter().cloned().map(Into::into).collect(),
            selector_symbol: None,
            selector_kind: None,
            selector_projection: None,
        })
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

fn encode_string_list(values: &[String]) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    output.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for value in values {
        output.extend_from_slice(&(value.len() as u32).to_le_bytes());
        output.extend_from_slice(value.as_bytes());
    }
    Ok(output)
}

fn decode_string_list(bytes: &[u8]) -> Result<Vec<String>, String> {
    let count = read_u32(bytes, 0)? as usize;
    let mut cursor = 4_usize;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let length = read_u32(bytes, cursor)? as usize;
        cursor += 4;
        let end = cursor
            .checked_add(length)
            .ok_or_else(|| "workspace search string range overflows".to_owned())?;
        let value = std::str::from_utf8(
            bytes
                .get(cursor..end)
                .ok_or_else(|| "workspace search string is truncated".to_owned())?,
        )
        .map_err(|error| format!("workspace search string is not UTF-8: {error}"))?;
        values.push(value.to_owned());
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err("workspace search string-list contains trailing bytes".to_owned());
    }
    Ok(values)
}

fn read_u32(bytes: &[u8], start: usize) -> Result<u32, String> {
    let encoded = bytes
        .get(
            start
                ..start
                    .checked_add(4)
                    .ok_or_else(|| "workspace search integer range overflows".to_owned())?,
        )
        .ok_or_else(|| "workspace search integer is truncated".to_owned())?;
    Ok(u32::from_le_bytes(encoded.try_into().map_err(|_| {
        "workspace search integer width is invalid".to_owned()
    })?))
}

fn graph_key(kind: &str, id: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(kind.len() + id.len() + 1);
    key.extend_from_slice(kind.as_bytes());
    key.push(0);
    key.extend_from_slice(id.as_bytes());
    key
}
