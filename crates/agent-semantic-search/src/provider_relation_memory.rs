use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use agent_semantic_content_identity::ProviderRelationEndpointKindV1;
use agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation;
use agent_semantic_content_identity::provider_projection_relation::ProviderRelationGeneration;
use memmap2::Mmap;
use memmap2::MmapOptions;

#[derive(Debug)]
enum ProviderRelationBytes {
    Mapped(Mmap),
    Owned(Arc<[u8]>),
}

impl AsRef<[u8]> for ProviderRelationBytes {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Mapped(mapping) => mapping,
            Self::Owned(bytes) => bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRelationMemoryError {
    ArtifactIo(String),
    ArtifactDigestMismatch,
    InvalidGeneration(String),
}

impl fmt::Display for ProviderRelationMemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactIo(error) => formatter.write_str(error),
            Self::ArtifactDigestMismatch => {
                formatter.write_str("provider relation artifact digest mismatch")
            }
            Self::InvalidGeneration(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for ProviderRelationMemoryError {}

#[derive(Debug, Clone)]
pub struct ProviderRelationMemorySearch {
    _bytes: Arc<ProviderRelationBytes>,
    generation: Arc<ProviderRelationGeneration>,
    source_index: Arc<BTreeMap<(ProviderRelationEndpointKindV1, String), Vec<usize>>>,
}

impl ProviderRelationMemorySearch {
    pub fn load_immutable_artifact(
        path: &Path,
        expected_generation_digest: [u8; 32],
        expected_artifact_digest: [u8; 32],
    ) -> Result<Self, ProviderRelationMemoryError> {
        let file = File::open(path).map_err(|error| {
            ProviderRelationMemoryError::ArtifactIo(format!(
                "failed to open provider relation artifact {}: {error}",
                path.display()
            ))
        })?;
        let mapping = unsafe {
            // SAFETY: generation artifacts are immutable after atomic publication,
            // and the mapping owns its file-backed virtual-memory lifetime.
            MmapOptions::new().map(&file)
        }
        .map_err(|error| {
            ProviderRelationMemoryError::ArtifactIo(format!(
                "failed to map provider relation artifact {}: {error}",
                path.display()
            ))
        })?;
        Self::attach(
            ProviderRelationBytes::Mapped(mapping),
            expected_generation_digest,
            expected_artifact_digest,
        )
    }

    pub fn attach_owned(
        bytes: Arc<[u8]>,
        expected_generation_digest: [u8; 32],
        expected_artifact_digest: [u8; 32],
    ) -> Result<Self, ProviderRelationMemoryError> {
        Self::attach(
            ProviderRelationBytes::Owned(bytes),
            expected_generation_digest,
            expected_artifact_digest,
        )
    }

    fn attach(
        bytes: ProviderRelationBytes,
        expected_generation_digest: [u8; 32],
        expected_artifact_digest: [u8; 32],
    ) -> Result<Self, ProviderRelationMemoryError> {
        if blake3::hash(bytes.as_ref()).as_bytes() != &expected_artifact_digest {
            return Err(ProviderRelationMemoryError::ArtifactDigestMismatch);
        }
        let generation: ProviderRelationGeneration = serde_json::from_slice(bytes.as_ref())
            .map_err(|error| {
                ProviderRelationMemoryError::InvalidGeneration(format!(
                    "invalid provider relation generation: {error}"
                ))
            })?;
        generation
            .validate()
            .map_err(ProviderRelationMemoryError::InvalidGeneration)?;
        let expected_generation_digest = expected_generation_digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if generation.generation_digest != format!("blake3-256:{expected_generation_digest}") {
            return Err(ProviderRelationMemoryError::InvalidGeneration(
                "provider relation generation identity mismatch".to_owned(),
            ));
        }
        let mut source_index =
            BTreeMap::<(ProviderRelationEndpointKindV1, String), Vec<usize>>::new();
        for (index, relation) in generation.relations.iter().enumerate() {
            source_index
                .entry((relation.from.kind, relation.from.id.clone()))
                .or_default()
                .push(index);
        }
        Ok(Self {
            _bytes: Arc::new(bytes),
            generation: Arc::new(generation),
            source_index: Arc::new(source_index),
        })
    }

    pub fn relations_from(
        &self,
        endpoint_kind: ProviderRelationEndpointKindV1,
        endpoint_id: &str,
    ) -> Vec<&ProviderProjectedRelation> {
        self.source_index
            .get(&(endpoint_kind, endpoint_id.to_owned()))
            .into_iter()
            .flatten()
            .filter_map(|index| self.generation.relations.get(*index))
            .collect()
    }

    pub fn relation_count(&self) -> usize {
        self.generation.relations.len()
    }
}
