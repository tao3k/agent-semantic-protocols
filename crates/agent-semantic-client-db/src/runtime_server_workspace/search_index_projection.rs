use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

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

#[derive(Debug)]
pub struct WorkspaceSearchGenerationDataPlaneClient {
    mapping: Mmap,
    authority: WorkspaceSearchGenerationAuthority,
}

pub fn workspace_search_generation_segment_path(generation_path: &Path) -> PathBuf {
    generation_path.with_extension("search.mmap")
}

pub fn encode_workspace_search_generation_segment(
    generation: &WorkspaceMemoryGeneration,
) -> Result<Vec<u8>, String> {
    generation.validate()?;
    let authority = WorkspaceSearchGenerationAuthority::from_generation(generation);
    authority.validate_binding(&generation.workspace_identity, &generation.project_root)?;
    let evidence = serde_json::to_vec(&authority)
        .map_err(|error| format!("encode workspace search generation evidence: {error}"))?;
    let project_resolutions = serde_json::to_vec(&generation.project_resolutions)
        .map_err(|error| format!("encode workspace search project resolutions: {error}"))?;

    let mut owners = generation.owners.iter().collect::<Vec<_>>();
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
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
        ],
    )
}

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
        Ok(Self { mapping, authority })
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
        let segment = ValidatedSearchGenerationSegment::parse(&self.mapping)?;
        let (lexical_bytes, _, _) = segment.section(SearchGenerationSectionKind::LexicalIndex);
        let lexical = ValidatedSortedRecordTable::parse(lexical_bytes)?;
        let query_terms = crate::source_index::source_query_keys("", query);
        let normalized = query.trim();
        let paths: Vec<String> = if !normalized.is_empty()
            && normalized.chars().all(|character| {
                character.is_alphanumeric() || character == '-' || character == '_'
            })
            && normalized.contains(['-', '_'])
        {
            lexical
                .get_checked(normalized.to_ascii_lowercase().as_bytes())?
                .map(decode_string_list)
                .transpose()?
                .unwrap_or_default()
                .into_iter()
                .take(limit as usize)
                .collect()
        } else {
            let mut scores = HashMap::<String, usize>::new();
            for term in query_terms {
                if let Some(value) = lexical.get_checked(term.as_bytes())? {
                    for path in decode_string_list(value)? {
                        *scores.entry(path).or_default() += 1;
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
            .map(|path| self.source_index_candidate(&segment, &path, language_id))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(crate::ClientDbSourceIndexLookupResult {
            db_path: PathBuf::new(),
            state: if candidates.is_empty() {
                crate::ClientDbSourceIndexLookupState::Miss
            } else {
                crate::ClientDbSourceIndexLookupState::Hit
            },
            candidates,
            source_snapshot: Some(self.authority.source_snapshot.clone()),
            index_artifact_digest: Some(crate::client_db_source_index_artifact_digest(
                &self.authority.source_snapshot,
            )),
        })
    }

    pub fn read_owner(&self, owner_path: &str) -> Result<super::WorkspaceRuntimeOwnerRead, String> {
        let segment = ValidatedSearchGenerationSegment::parse(&self.mapping)?;
        let Some(record) = self.owner_record(&segment, owner_path)? else {
            return Ok(super::WorkspaceRuntimeOwnerRead::OwnerMissing {
                generation_digest: self.authority.generation_digest.clone(),
                root_digest: self.authority.source_snapshot.root_digest.clone(),
            });
        };
        let (bytes, _, _) = segment.section(SearchGenerationSectionKind::OwnerBytes);
        let start = record.byte_offset as usize;
        let end = start
            .checked_add(record.byte_length as usize)
            .ok_or_else(|| "workspace search owner byte range overflows".to_owned())?;
        let owner_bytes = bytes
            .get(start..end)
            .ok_or_else(|| "workspace search owner bytes exceed section bounds".to_owned())?;
        Ok(super::WorkspaceRuntimeOwnerRead::Owner {
            generation_digest: self.authority.generation_digest.clone(),
            root_digest: self.authority.source_snapshot.root_digest.clone(),
            owner: WorkspaceOwnerSnapshot {
                owner_path: record.owner_path,
                content_digest: record.content_digest,
                bytes: owner_bytes.to_vec(),
                selectors: record.selectors,
            },
        })
    }

    pub fn read_graph_facts(
        &self,
        sources: &[crate::workspace_db_ipc::RuntimeGraphFactSource],
    ) -> Result<crate::workspace_db_ipc::RuntimeGraphFactsRead, String> {
        let segment = ValidatedSearchGenerationSegment::parse(&self.mapping)?;
        let (bytes, _, _) = segment.section(SearchGenerationSectionKind::GraphRelations);
        let table = ValidatedSortedRecordTable::parse(bytes)?;
        let mut relations = Vec::new();
        for source in sources {
            if let Some(value) = table.get_checked(&graph_key(&source.kind, &source.id))? {
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

    fn owner_record(
        &self,
        segment: &ValidatedSearchGenerationSegment<'_>,
        owner_path: &str,
    ) -> Result<Option<SearchOwnerRecord>, String> {
        let (bytes, _, _) = segment.section(SearchGenerationSectionKind::OwnerDirectory);
        let table = ValidatedSortedRecordTable::parse(bytes)?;
        table
            .get_checked(owner_path.as_bytes())?
            .map(|value| {
                serde_json::from_slice(value)
                    .map_err(|error| format!("decode workspace search owner record: {error}"))
            })
            .transpose()
    }

    fn source_index_candidate(
        &self,
        segment: &ValidatedSearchGenerationSegment<'_>,
        owner_path: &str,
        language_id: Option<&agent_semantic_client_core::LanguageId>,
    ) -> Result<crate::ClientDbSourceIndexCandidate, String> {
        let record = self.owner_record(segment, owner_path)?.ok_or_else(|| {
            "workspace search lexical index references a missing owner".to_owned()
        })?;
        Ok(crate::ClientDbSourceIndexCandidate {
            path: record.owner_path.into(),
            language_id: language_id.cloned(),
            provider_id: None,
            source_kind: crate::ClientDbSourceIndexSourceKind::File,
            line_count: Some(record.line_count),
            query_keys: record.query_keys.into_iter().map(Into::into).collect(),
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
