// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resident Search/Query read paths over one immutable generation.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::{SearchOwnerRecord, WorkspaceSearchGenerationDataPlaneClient};

const MAX_COLD_LEXICAL_CANDIDATES: u32 = 4096;

impl WorkspaceSearchGenerationDataPlaneClient {
    /// Returns the immutable parser-owned topology inputs retained by this
    /// exact resident or mmap generation.
    pub fn topology_source_segments(
        &self,
    ) -> Result<Vec<crate::runtime_server_workspace::WorkspaceTopologySourceSegment>, String> {
        let mut relations_by_owner = self
            .owner_directory_records
            .keys()
            .map(|owner_path| (owner_path.clone(), Vec::new()))
            .collect::<std::collections::BTreeMap<_, _>>();
        for relation in self.owned_relations.iter() {
            relation.relation.validate()?;
            relations_by_owner
                .get_mut(relation.owner_path.as_str())
                .ok_or_else(|| {
                    format!(
                        "topology relation owner is absent from generation: {}",
                        relation.owner_path.as_str()
                    )
                })?
                .push(relation.clone());
        }
        self.owner_directory_records
            .iter()
            .map(|(owner_path, owner)| {
                let mut selectors = owner
                    .selectors
                    .iter()
                    .map(|selector| selector.selector.clone())
                    .collect::<Vec<_>>();
                selectors.sort();
                selectors.dedup();
                let mut relations = relations_by_owner
                    .remove(owner_path)
                    .expect("owner relation bucket was initialized");
                relations.sort();
                Ok(
                    crate::runtime_server_workspace::WorkspaceTopologySourceSegment {
                        owner_path: owner_path.clone(),
                        content_digest: owner.content_digest.clone(),
                        authority: owner.authority.clone(),
                        selectors,
                        relations,
                    },
                )
            })
            .collect()
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
        validate_cold_lexical_request(query, limit)?;
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
            Vec<agent_semantic_search::NativeSyntaxDiagnostic>,
        ),
        String,
    > {
        let admitted = owner_paths
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut projections = Vec::with_capacity(admitted.len());
        let mut diagnostics = Vec::new();
        for owner_path in &admitted {
            let record = self
                .resident_owner_record(owner_path)?
                .ok_or_else(|| "native syntax playbook owner is absent".to_owned())?;
            if let Some(diagnostic) = &record.native_syntax_diagnostic {
                if diagnostic.owner_path != *owner_path
                    || diagnostic.content_digest != record.content_digest
                {
                    return Err("native syntax diagnostic identity drift".to_owned());
                }
                diagnostics.push(diagnostic.clone());
                continue;
            }
            if record.selectors.is_empty() {
                diagnostics.push(agent_semantic_search::NativeSyntaxDiagnostic {
                    owner_path: record.owner_path.clone(),
                    content_digest: record.content_digest.clone(),
                    reason_kind: "source-syntax-unavailable".to_owned(),
                    message: "the admitted owner has no parser-owned selectors".to_owned(),
                });
                continue;
            }
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
        diagnostics.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
        relations.sort_by(|left, right| {
            left.owner_path
                .cmp(&right.owner_path)
                .then_with(|| left.relation_digest.cmp(&right.relation_digest))
        });
        Ok((projections, relations, diagnostics))
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
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead, String> {
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
            return Ok(
                crate::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead::OwnerMissing {
                    schema_id:
                        crate::runtime_server_workspace::RUNTIME_MERKLE_OWNER_READ_RECEIPT_SCHEMA_ID
                            .to_owned(),
                    schema_version: "1".to_owned(),
                    workspace_identity: self.authority.workspace_id.clone(),
                    project_root: self.project_root.clone(),
                    active_epoch: self.authority.active_epoch,
                    generation_digest: self.authority.generation_digest.clone(),
                    root_digest,
                    owner_path: owner_path.to_owned(),
                },
            );
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
            agent_semantic_content_identity::workspace_merkle_v1::WorkspaceOwnerInclusionV1 {
                owner_path: &record.owner_path,
                source_blob_digest: &source_blob_digest,
                expected_owner_subtree_digest: &owner_subtree_digest,
                inclusion_proof: &record.inclusion_proof,
                expected_workspace_root_digest: &parsed_root_digest,
            },
        ) {
            return Err(format!(
                "workspace search Merkle owner proof drift: ownerPath={owner_path}"
            ));
        }
        Ok(
            crate::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead::Owner {
                schema_id:
                    crate::runtime_server_workspace::RUNTIME_MERKLE_OWNER_READ_RECEIPT_SCHEMA_ID
                        .to_owned(),
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
            },
        )
    }

    pub fn read_owner(
        &self,
        owner_path: &str,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead, String> {
        let Some(record) = self.resident_owner_record(owner_path)? else {
            return Ok(
                crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::OwnerMissing {
                    generation_digest: self.authority.generation_digest.clone(),
                    root_digest: self.authority.source_snapshot.root_digest.clone(),
                },
            );
        };
        let owner_bytes = self.resident_owner_bytes(&record)?;
        Ok(
            crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::Owner {
                generation_digest: self.authority.generation_digest.clone(),
                root_digest: self.authority.source_snapshot.root_digest.clone(),
                owner: crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
                    authority: None,
                    owner_path: record.owner_path.clone(),
                    content_digest: record.content_digest.clone(),
                    native_syntax_diagnostic: record.native_syntax_diagnostic.clone(),
                    bytes: owner_bytes.to_vec(),
                    selectors: record.selectors.clone(),
                },
            },
        )
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

fn validate_cold_lexical_request(query: &str, limit: u32) -> Result<(), String> {
    if query.trim().is_empty() || !(1..=MAX_COLD_LEXICAL_CANDIDATES).contains(&limit) {
        return Err(
            "cold resident search requires a non-empty query and limit in 1..=4096".to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_cold_lexical_request;

    #[test]
    fn progressive_acquisition_accepts_4096_candidates_but_rejects_larger_requests() {
        assert!(validate_cold_lexical_request("runtime|query", 4096).is_ok());
        assert!(validate_cold_lexical_request("runtime|query", 4097).is_err());
        assert!(validate_cold_lexical_request(" ", 30).is_err());
    }
}
