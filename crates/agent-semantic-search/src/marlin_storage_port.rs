// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Storage-neutral persistence port for Marlin-native search engine convergence.
//!
//! Marlin (`marlin-agent-storage`) owns the unified Turso persistence layer
//! (`AgentStorage`, `TursoAgentStorage`, `TursoContextIndex`). ASP's search
//! engine consumes only this minimal typed port instead of embedding a second
//! Turso database engine or standalone workspace DB daemon.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

macro_rules! search_storage_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, SearchStoragePortError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(SearchStoragePortError::InvalidIdentifier {
                        kind: stringify!($name),
                    });
                }
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

search_storage_id!(
    SearchProjectId,
    "Typed project identity aligned with Marlin `marlin-agent-storage::ProjectId`."
);
search_storage_id!(
    SearchWorkspaceId,
    "Typed workspace identity bound to one Marlin project."
);
search_storage_id!(
    SearchGenerationDigest,
    "Canonical BLAKE3 digest identifying one immutable search/source generation."
);
search_storage_id!(
    SearchArtifactHash,
    "Content-addressed digest aligned with Marlin `marlin-agent-storage::ArtifactHash`."
);
search_storage_id!(
    SearchPointerKey,
    "Named CAS pointer key aligned with Marlin `marlin-agent-storage::ArtifactPointerKey`."
);
search_storage_id!(
    SearchTopologyEdgeId,
    "Stable topology edge identity aligned with Marlin `marlin-agent-storage::TopologyEdgeId`."
);

/// Content-addressed derived search/index artifact stored in Marlin's unified storage.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchArtifactBlob {
    pub project_id: SearchProjectId,
    pub artifact_hash: SearchArtifactHash,
    pub media_type: String,
    pub body: Vec<u8>,
}

impl SearchArtifactBlob {
    /// Construct a content-addressed artifact blob and compute its `blake3-256:` hash.
    pub fn from_bytes(
        project_id: SearchProjectId,
        media_type: impl Into<String>,
        body: Vec<u8>,
    ) -> Result<Self, SearchStoragePortError> {
        let media_type = media_type.into();
        if media_type.trim().is_empty() {
            return Err(SearchStoragePortError::InvalidIdentifier { kind: "mediaType" });
        }
        let digest = format!("blake3-256:{}", blake3::hash(&body).to_hex());
        let artifact_hash = SearchArtifactHash::new(digest)?;
        Ok(Self {
            project_id,
            artifact_hash,
            media_type,
            body,
        })
    }

    /// Verify that `body` hashes to `artifact_hash`.
    pub fn verify_digest(&self) -> Result<(), SearchStoragePortError> {
        let expected = format!("blake3-256:{}", blake3::hash(&self.body).to_hex());
        if self.artifact_hash.as_str() != expected {
            return Err(SearchStoragePortError::ArtifactHashMismatch {
                declared: self.artifact_hash.clone(),
                computed: expected,
            });
        }
        Ok(())
    }
}

/// Typed compare-and-swap request for advancing a workspace generation pointer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchGenerationPointerSwapRequest {
    pub project_id: SearchProjectId,
    pub workspace_id: SearchWorkspaceId,
    pub pointer_key: SearchPointerKey,
    pub expected_hash: Option<SearchArtifactHash>,
    pub new_generation_digest: SearchGenerationDigest,
    pub new_artifact_hash: SearchArtifactHash,
}

/// Compare-and-swap generation pointer record persisted by Marlin's `AgentStorage`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchGenerationPointerRecord {
    pub project_id: SearchProjectId,
    pub workspace_id: SearchWorkspaceId,
    pub pointer_key: SearchPointerKey,
    pub generation_digest: SearchGenerationDigest,
    pub artifact_hash: SearchArtifactHash,
    pub revision: u64,
}

/// Derived topology edge persisted via Marlin's `AgentStorage::append_topology_edge`.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchTopologyEdgeRecord {
    pub project_id: SearchProjectId,
    pub generation_digest: SearchGenerationDigest,
    pub edge_id: SearchTopologyEdgeId,
    pub from_node: String,
    pub to_node: String,
    pub relation: String,
    pub payload: Vec<u8>,
}

/// Deterministic error returned by a [`SearchEngineStoragePort`] implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchStoragePortError {
    InvalidIdentifier {
        kind: &'static str,
    },
    ArtifactHashMismatch {
        declared: SearchArtifactHash,
        computed: String,
    },
    MissingArtifact {
        project_id: SearchProjectId,
        artifact_hash: SearchArtifactHash,
    },
    PointerConflict {
        pointer_key: SearchPointerKey,
        expected: Option<SearchArtifactHash>,
        actual: Option<SearchArtifactHash>,
    },
    Backend(String),
}

impl fmt::Display for SearchStoragePortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { kind } => {
                write!(formatter, "invalid empty search storage identifier: {kind}")
            }
            Self::ArtifactHashMismatch { declared, computed } => {
                write!(
                    formatter,
                    "artifact digest mismatch: declared={declared}, computed={computed}"
                )
            }
            Self::MissingArtifact {
                project_id,
                artifact_hash,
            } => write!(
                formatter,
                "artifact {artifact_hash} is missing in project {project_id}"
            ),
            Self::PointerConflict {
                pointer_key,
                expected,
                actual,
            } => write!(
                formatter,
                "generation pointer conflict for {pointer_key}: expected={expected:?}, actual={actual:?}"
            ),
            Self::Backend(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for SearchStoragePortError {}

/// Minimal host-owned storage port implemented by Marlin (`marlin-agent-storage`).
///
/// Operations map 1:1 onto Marlin's `AgentStorage` artifact CAS, pointer CAS,
/// and topology edge tables so ASP never needs to embed a duplicate Turso DB.
pub trait SearchEngineStoragePort: Send + Sync {
    /// Persist one immutable content-addressed search artifact. Returns `true`
    /// when newly inserted and `false` when already present.
    fn put_artifact(&self, artifact: SearchArtifactBlob) -> Result<bool, SearchStoragePortError>;

    /// Fetch one immutable content-addressed search artifact by digest.
    fn get_artifact(
        &self,
        project_id: &SearchProjectId,
        artifact_hash: &SearchArtifactHash,
    ) -> Result<Option<SearchArtifactBlob>, SearchStoragePortError>;

    /// Atomically advance a workspace's active generation pointer.
    fn compare_and_swap_generation_pointer(
        &self,
        request: SearchGenerationPointerSwapRequest,
    ) -> Result<SearchGenerationPointerRecord, SearchStoragePortError>;

    /// Read the currently committed generation pointer for a workspace.
    fn get_generation_pointer(
        &self,
        project_id: &SearchProjectId,
        workspace_id: &SearchWorkspaceId,
        pointer_key: &SearchPointerKey,
    ) -> Result<Option<SearchGenerationPointerRecord>, SearchStoragePortError>;

    /// Persist derived topology edges for one generation (idempotent by edge id).
    fn append_topology_edges(
        &self,
        edges: &[SearchTopologyEdgeRecord],
    ) -> Result<usize, SearchStoragePortError>;

    /// Load all topology edges recorded for a project generation in deterministic order.
    fn list_topology_edges(
        &self,
        project_id: &SearchProjectId,
        generation_digest: &SearchGenerationDigest,
    ) -> Result<Vec<SearchTopologyEdgeRecord>, SearchStoragePortError>;
}

type ArtifactKey = (SearchProjectId, SearchArtifactHash);
type PointerMapKey = (SearchProjectId, SearchWorkspaceId, SearchPointerKey);
type EdgeMapKey = (SearchProjectId, SearchGenerationDigest);

#[derive(Default)]
struct InMemoryState {
    artifacts: BTreeMap<ArtifactKey, SearchArtifactBlob>,
    pointers: BTreeMap<PointerMapKey, SearchGenerationPointerRecord>,
    edges: BTreeMap<EdgeMapKey, BTreeMap<SearchTopologyEdgeId, SearchTopologyEdgeRecord>>,
}

/// Zero-dependency in-memory reference implementation of [`SearchEngineStoragePort`].
#[derive(Default)]
pub struct InMemorySearchStoragePort {
    state: RwLock<InMemoryState>,
}

impl InMemorySearchStoragePort {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SearchEngineStoragePort for InMemorySearchStoragePort {
    fn put_artifact(&self, artifact: SearchArtifactBlob) -> Result<bool, SearchStoragePortError> {
        artifact.verify_digest()?;
        let key = (artifact.project_id.clone(), artifact.artifact_hash.clone());
        let mut state = self
            .state
            .write()
            .map_err(|_| SearchStoragePortError::Backend("storage lock poisoned".to_owned()))?;
        if let Some(existing) = state.artifacts.get(&key) {
            if existing.body != artifact.body {
                return Err(SearchStoragePortError::ArtifactHashMismatch {
                    declared: artifact.artifact_hash,
                    computed: "existing-artifact-body-conflict".to_owned(),
                });
            }
            return Ok(false);
        }
        state.artifacts.insert(key, artifact);
        Ok(true)
    }

    fn get_artifact(
        &self,
        project_id: &SearchProjectId,
        artifact_hash: &SearchArtifactHash,
    ) -> Result<Option<SearchArtifactBlob>, SearchStoragePortError> {
        let state = self
            .state
            .read()
            .map_err(|_| SearchStoragePortError::Backend("storage lock poisoned".to_owned()))?;
        Ok(state
            .artifacts
            .get(&(project_id.clone(), artifact_hash.clone()))
            .cloned())
    }

    fn compare_and_swap_generation_pointer(
        &self,
        request: SearchGenerationPointerSwapRequest,
    ) -> Result<SearchGenerationPointerRecord, SearchStoragePortError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| SearchStoragePortError::Backend("storage lock poisoned".to_owned()))?;
        let artifact_key = (
            request.project_id.clone(),
            request.new_artifact_hash.clone(),
        );
        if !state.artifacts.contains_key(&artifact_key) {
            return Err(SearchStoragePortError::MissingArtifact {
                project_id: request.project_id,
                artifact_hash: request.new_artifact_hash,
            });
        }
        let key = (
            request.project_id.clone(),
            request.workspace_id.clone(),
            request.pointer_key.clone(),
        );
        let actual_hash = state
            .pointers
            .get(&key)
            .map(|record| record.artifact_hash.clone());
        if actual_hash != request.expected_hash {
            return Err(SearchStoragePortError::PointerConflict {
                pointer_key: request.pointer_key,
                expected: request.expected_hash,
                actual: actual_hash,
            });
        }
        let next_revision = state
            .pointers
            .get(&key)
            .map_or(1, |record| record.revision.saturating_add(1));
        let record = SearchGenerationPointerRecord {
            project_id: request.project_id,
            workspace_id: request.workspace_id,
            pointer_key: request.pointer_key,
            generation_digest: request.new_generation_digest,
            artifact_hash: request.new_artifact_hash,
            revision: next_revision,
        };
        state.pointers.insert(key, record.clone());
        Ok(record)
    }

    fn get_generation_pointer(
        &self,
        project_id: &SearchProjectId,
        workspace_id: &SearchWorkspaceId,
        pointer_key: &SearchPointerKey,
    ) -> Result<Option<SearchGenerationPointerRecord>, SearchStoragePortError> {
        let state = self
            .state
            .read()
            .map_err(|_| SearchStoragePortError::Backend("storage lock poisoned".to_owned()))?;
        Ok(state
            .pointers
            .get(&(
                project_id.clone(),
                workspace_id.clone(),
                pointer_key.clone(),
            ))
            .cloned())
    }

    fn append_topology_edges(
        &self,
        edges: &[SearchTopologyEdgeRecord],
    ) -> Result<usize, SearchStoragePortError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| SearchStoragePortError::Backend("storage lock poisoned".to_owned()))?;
        let mut inserted = 0usize;
        for edge in edges {
            let bucket = state
                .edges
                .entry((edge.project_id.clone(), edge.generation_digest.clone()))
                .or_default();
            if bucket.insert(edge.edge_id.clone(), edge.clone()).is_none() {
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    fn list_topology_edges(
        &self,
        project_id: &SearchProjectId,
        generation_digest: &SearchGenerationDigest,
    ) -> Result<Vec<SearchTopologyEdgeRecord>, SearchStoragePortError> {
        let state = self
            .state
            .read()
            .map_err(|_| SearchStoragePortError::Backend("storage lock poisoned".to_owned()))?;
        Ok(state
            .edges
            .get(&(project_id.clone(), generation_digest.clone()))
            .map(|bucket| bucket.values().cloned().collect())
            .unwrap_or_default())
    }
}
