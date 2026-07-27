use crate::memory_search::{
    MemorySearchBackendV1, MemorySearchGenerationV1, MemorySearchIndexV1, MemorySearchItemV1,
    MemorySearchSourceLeafV1,
};
use agent_semantic_client_core::{SemanticSchemaId, SemanticSchemaVersion};
use agent_semantic_client_db::engine::ClientDbSourceIndexGenerationSnapshotV1;
use agent_semantic_content_identity::CanonicalItemSelectorV1;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
/// Provider identity required to interpret one active Turso generation.
pub struct TursoMemorySearchBindingV1 {
    /// Registered language identifier.
    pub language_id: String,
    /// Activated provider identifier.
    pub provider_id: String,
    /// Digest of the parser identity used to produce selectors.
    pub parser_identity_digest: String,
    /// Digest of the activated query pack.
    pub query_pack_digest: String,
}

#[derive(Clone, Debug)]
/// Immutable Search-owned backend materialized from an active Turso generation.
pub struct TursoMemorySearchBackendV1 {
    index: MemorySearchIndexV1,
}

impl TursoMemorySearchBackendV1 {
    /// Load and verify the active database generation once before serving memory-only reads.
    pub async fn load_from_db(
        db_path: &Path,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
        binding: TursoMemorySearchBindingV1,
    ) -> Result<Option<Self>, String> {
        let Some(snapshot) =
            agent_semantic_client_db::engine::latest_turso_source_index_generation_snapshot(
                db_path,
                project_root,
                schema_id,
                schema_version,
            )
            .await?
        else {
            return Ok(None);
        };
        Self::from_snapshot(snapshot, binding).map(Some)
    }

    /// Verify and materialize a database snapshot into the shared in-memory index.
    pub fn from_snapshot(
        snapshot: ClientDbSourceIndexGenerationSnapshotV1,
        binding: TursoMemorySearchBindingV1,
    ) -> Result<Self, String> {
        if binding.language_id.is_empty()
            || binding.provider_id.is_empty()
            || binding.parser_identity_digest.len() != 64
            || binding.query_pack_digest.len() != 64
        {
            return Err("invalid Turso memory-search provider binding".to_string());
        }
        let source_leaves = snapshot
            .file_hashes
            .iter()
            .map(
                |(owner_path, owner_content_digest)| MemorySearchSourceLeafV1 {
                    owner_path: owner_path.clone(),
                    owner_content_digest: owner_content_digest.clone(),
                },
            )
            .collect::<Vec<_>>();
        let mut items = Vec::with_capacity(snapshot.selector_count as usize);
        for owner in &snapshot.owners {
            if owner.language_id.as_deref() != Some(binding.language_id.as_str())
                || owner.provider_id.as_deref() != Some(binding.provider_id.as_str())
            {
                return Err(format!(
                    "Turso memory-search generation contains a foreign provider owner: ownerPath={}",
                    owner.owner_path
                ));
            }
            for selector in &owner.selectors {
                let canonical_item_selector =
                    CanonicalItemSelectorV1::parse(
                        &selector.materialization_proof.structural_selector,
                    )
                    .map_err(
                        |error| {
                            format!(
                                "invalid Turso memory-search canonical selector: ownerPath={} error={error}",
                                owner.owner_path
                            )
                        },
                    )?;
                items.push(MemorySearchItemV1 {
                    owner_path: owner.owner_path.clone(),
                    owner_content_digest: owner.owner_content_digest.clone(),
                    canonical_item_selector,
                });
            }
        }
        let leaf_count = source_leaves.len();
        let generation = MemorySearchGenerationV1 {
            generation_id: snapshot.generation_id,
            root_digest: snapshot.source_snapshot.root_digest,
            root_depth: MemorySearchGenerationV1::expected_root_depth(leaf_count),
            leaf_count,
            owner_count: snapshot.owner_count as usize,
            selector_count: snapshot.selector_count as usize,
            language_id: binding.language_id,
            provider_id: binding.provider_id,
            parser_identity_digest: binding.parser_identity_digest,
            query_pack_digest: binding.query_pack_digest,
            source_leaves,
        };
        Ok(Self {
            index: MemorySearchIndexV1::build(generation, items)?,
        })
    }
}

impl MemorySearchBackendV1 for TursoMemorySearchBackendV1 {
    fn load_generation(&self) -> Result<MemorySearchIndexV1, String> {
        Ok(self.index.clone())
    }
}
