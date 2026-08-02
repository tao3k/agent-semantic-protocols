//! In-memory indexes and leases for an admitted workspace generation.

use std::{collections::HashMap, sync::Arc};

use super::WorkspaceMemoryGeneration;

#[derive(Debug)]
pub(crate) struct WorkspaceMemoryBackend {
    generation: Arc<WorkspaceMemoryGeneration>,
    selector_index: HashMap<String, (usize, usize)>,
    term_index: HashMap<String, Vec<usize>>,
    relation_index: HashMap<(String, String), Vec<usize>>,
}

impl WorkspaceMemoryBackend {
    pub(crate) fn from_generation(generation: WorkspaceMemoryGeneration) -> Result<Self, String> {
        generation.validate()?;
        let mut selector_index = HashMap::new();
        let mut term_index = HashMap::<String, Vec<usize>>::new();
        for (owner_position, owner) in generation.owners.iter().enumerate() {
            let text = std::str::from_utf8(&owner.bytes).unwrap_or_default();
            for term in crate::source_index::source_query_keys(&owner.owner_path, text) {
                term_index.entry(term).or_default().push(owner_position);
            }
            for (selector_position, selector) in owner.selectors.iter().enumerate() {
                selector_index.insert(
                    selector.selector.clone(),
                    (owner_position, selector_position),
                );
            }
        }
        let mut relation_index = HashMap::<(String, String), Vec<usize>>::new();
        for (relation_position, relation) in generation.relations.iter().enumerate() {
            relation_index
                .entry((relation.from.kind.clone(), relation.from.id.clone()))
                .or_default()
                .push(relation_position);
        }
        Ok(Self {
            generation: Arc::new(generation),
            selector_index,
            term_index,
            relation_index,
        })
    }

    pub(crate) fn generation(&self) -> &Arc<WorkspaceMemoryGeneration> {
        &self.generation
    }

    pub(crate) fn projection(self: &Arc<Self>, selector: &str) -> Option<WorkspaceProjectionLease> {
        let (owner_position, selector_position) = *self.selector_index.get(selector)?;
        Some(WorkspaceProjectionLease {
            backend: Arc::clone(self),
            owner_position,
            selector_position,
        })
    }

    pub(crate) fn source_index_owner_positions(&self, query: &str, limit: usize) -> Vec<usize> {
        let query_terms = crate::source_index::source_query_keys("", query);
        let query = query.trim();
        if !query.is_empty()
            && query.chars().all(|character| {
                character.is_alphanumeric() || character == '-' || character == '_'
            })
            && query.contains(['-', '_'])
        {
            return self
                .term_index
                .get(&query.to_ascii_lowercase())
                .into_iter()
                .flatten()
                .copied()
                .take(limit)
                .collect();
        }
        let mut scores = HashMap::<usize, usize>::new();
        for term in query_terms {
            if let Some(positions) = self.term_index.get(&term) {
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
    ) -> Vec<&agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation>
    {
        self.relation_index
            .get(&(endpoint_kind.to_owned(), endpoint_id.to_owned()))
            .into_iter()
            .flatten()
            .filter_map(|position| self.generation.relations.get(*position))
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
