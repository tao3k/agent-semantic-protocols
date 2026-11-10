// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! In-memory indexes and leases for an admitted workspace generation.

use std::{collections::HashMap, sync::Arc};

use super::WorkspaceMemoryGeneration;

#[derive(Debug)]
pub(crate) struct WorkspaceMemoryBackend {
    generation: Arc<WorkspaceMemoryGeneration>,
    index: Arc<WorkspaceMemoryIndex>,
    search_data_plane: Arc<super::WorkspaceSearchGenerationDataPlaneClient>,
    base_overlay: super::ResidentOverlaySnapshot,
}

#[derive(Debug)]
pub(crate) struct WorkspaceMemoryIndex {
    owner_index: HashMap<String, usize>,
    selector_index: HashMap<String, (usize, usize)>,
    term_index: std::sync::OnceLock<HashMap<String, Vec<usize>>>,
    relation_index: std::sync::OnceLock<HashMap<(String, String), Vec<usize>>>,
}

impl WorkspaceMemoryBackend {
    pub(crate) fn from_validated_generation(
        generation: WorkspaceMemoryGeneration,
    ) -> Result<Self, String> {
        // The caller authenticates the complete generation before constructing the indexes.
        // Both resident publication and durable segment restore use this same validated input.
        let index = Self::prepare_index(&generation.owners, &generation.relations);
        Self::from_validated_generation_with_index(Arc::new(generation), index)
    }

    pub(crate) fn prepare_index(
        owners: &[super::WorkspaceOwnerSnapshot],
        _relations: &[crate::ClientDbSourceIndexOwnedRelation],
    ) -> Arc<WorkspaceMemoryIndex> {
        let mut owner_index = HashMap::new();
        let mut selector_index = HashMap::new();
        for (owner_position, owner) in owners.iter().enumerate() {
            owner_index.insert(owner.owner_path.clone(), owner_position);
            for (selector_position, selector) in owner.selectors.iter().enumerate() {
                selector_index.insert(
                    selector.selector.clone(),
                    (owner_position, selector_position),
                );
            }
        }
        Arc::new(WorkspaceMemoryIndex {
            owner_index,
            selector_index,
            term_index: std::sync::OnceLock::new(),
            relation_index: std::sync::OnceLock::new(),
        })
    }

    pub(crate) fn from_validated_generation_with_index(
        generation: Arc<WorkspaceMemoryGeneration>,
        index: Arc<WorkspaceMemoryIndex>,
    ) -> Result<Self, String> {
        // Admission owns the only O(workspace) construction of the immutable
        // Search data plane. Read handles clone this Arc; they must never
        // revalidate or rebuild a generation on the request path.
        let search_data_plane = Arc::new(
            super::WorkspaceSearchGenerationDataPlaneClient::from_admitted_generation(Arc::clone(
                &generation,
            ))?,
        );
        // A lease is a reference to admitted state, not another workspace
        // construction. Build the empty read-through overlay once beside the
        // generation so every later lease clones only Arc-backed state.
        let base_overlay =
            super::ResidentOverlayStore::new(Some(&generation)).snapshot(&generation);
        Ok(Self {
            generation,
            index,
            search_data_plane,
            base_overlay,
        })
    }

    pub(crate) fn generation(&self) -> &Arc<WorkspaceMemoryGeneration> {
        &self.generation
    }

    pub(crate) fn search_data_plane(&self) -> Arc<super::WorkspaceSearchGenerationDataPlaneClient> {
        Arc::clone(&self.search_data_plane)
    }

    pub(crate) fn base_overlay(&self) -> super::ResidentOverlaySnapshot {
        self.base_overlay.clone()
    }

    pub(crate) fn projection(self: &Arc<Self>, selector: &str) -> Option<WorkspaceProjectionLease> {
        let (owner_position, selector_position) = *self.index.selector_index.get(selector)?;
        Some(WorkspaceProjectionLease {
            backend: Arc::clone(self),
            owner_position,
            selector_position,
        })
    }

    pub(crate) fn owner_snapshot(
        &self,
        owner_path: &str,
    ) -> Option<&super::WorkspaceOwnerSnapshot> {
        self.index
            .owner_index
            .get(owner_path)
            .and_then(|position| self.generation.owners.get(*position))
    }

    pub(crate) fn selector_snapshot(
        &self,
        selector: &str,
    ) -> Option<(
        &super::WorkspaceOwnerSnapshot,
        &super::WorkspaceSelectorSnapshot,
    )> {
        let (owner_position, selector_position) = *self.index.selector_index.get(selector)?;
        let owner = self.generation.owners.get(owner_position)?;
        let selector = owner.selectors.get(selector_position)?;
        Some((owner, selector))
    }

    pub(crate) fn source_index_owner_positions(&self, query: &str, limit: usize) -> Vec<usize> {
        let term_index = self.index.term_index.get_or_init(|| {
            let mut index = HashMap::<String, Vec<usize>>::new();
            for (owner_position, owner) in self.generation.owners.iter().enumerate() {
                let text = std::str::from_utf8(&owner.bytes).unwrap_or_default();
                for term in crate::source_index::source_query_keys(&owner.owner_path, text) {
                    index.entry(term).or_default().push(owner_position);
                }
            }
            index
        });
        let query_terms = crate::source_index::source_query_keys("", query);
        let query = query.trim();
        if !query.is_empty()
            && query.chars().all(|character| {
                character.is_alphanumeric() || character == '-' || character == '_'
            })
            && query.contains(['-', '_'])
        {
            return term_index
                .get(&query.to_ascii_lowercase())
                .into_iter()
                .flatten()
                .copied()
                .take(limit)
                .collect();
        }
        let mut scores = HashMap::<usize, usize>::new();
        for term in query_terms {
            if let Some(positions) = term_index.get(&term) {
                for &position in positions {
                    *scores.entry(position).or_default() += 1;
                }
            }
        }
        let mut ranked = scores.into_iter().collect::<Vec<_>>();
        ranked.sort_unstable_by(
            |(left_position, left_score), (right_position, right_score)| {
                right_score
                    .cmp(left_score)
                    .then_with(|| left_position.cmp(right_position))
            },
        );
        ranked.truncate(limit);
        ranked
            .into_iter()
            .map(|(position, _score)| position)
            .collect()
    }

    pub(crate) fn relations_from(
        &self,
        endpoint_kind: &str,
        endpoint_id: &str,
    ) -> Vec<
        &agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
    > {
        let relation_index = self.index.relation_index.get_or_init(|| {
            let mut index = HashMap::<(String, String), Vec<usize>>::new();
            for (relation_position, owned) in self.generation.relations.iter().enumerate() {
                let relation = &owned.relation;
                index
                    .entry((relation.from.kind.to_string(), relation.from.id.clone()))
                    .or_default()
                    .push(relation_position);
            }
            index
        });
        relation_index
            .get(&(endpoint_kind.to_owned(), endpoint_id.to_owned()))
            .into_iter()
            .flatten()
            .filter_map(|position| {
                self.generation
                    .relations
                    .get(*position)
                    .map(|owned| &owned.relation)
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceProjectionLease {
    pub(crate) backend: Arc<WorkspaceMemoryBackend>,
    pub(crate) owner_position: usize,
    pub(crate) selector_position: usize,
}

impl WorkspaceProjectionLease {
    pub fn workspace_identity(&self) -> &str {
        &self.backend.generation.workspace_identity
    }

    pub fn epoch(&self) -> u64 {
        self.backend.generation.active_epoch
    }

    pub fn selector(&self) -> &str {
        &self.backend.generation.owners[self.owner_position].selectors[self.selector_position]
            .selector
    }

    pub fn bytes(&self) -> &[u8] {
        let owner = &self.backend.generation.owners[self.owner_position];
        let selector = &owner.selectors[self.selector_position];
        &owner.bytes[selector.byte_start..selector.byte_end]
    }
}
