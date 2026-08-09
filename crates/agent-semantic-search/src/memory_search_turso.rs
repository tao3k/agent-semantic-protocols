use std::time::Instant;

use agent_semantic_client_db::{ProviderSearchWorkspaceSession, TursoResidentSelectorQuery};

use crate::memory_search::{
    MemorySearchGenerationReceipt, MemorySearchItem, MemorySearchPerformanceReceipt,
    MemorySearchRequest, MemorySearchResolution, MemorySearchResolutionState,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoMemorySearchBinding {
    pub language_id: String,
    pub provider_id: String,
    pub parser_identity_digest: String,
    pub query_pack_digest: String,
}

#[derive(Clone, Debug)]
pub struct TursoMemorySearchBackend {
    session: TursoMemorySearchSession,
    project_root: String,
    schema_id: String,
    schema_version: String,
    binding: TursoMemorySearchBinding,
}

#[derive(Clone, Debug)]
enum TursoMemorySearchSession {
    Resident(agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession),
    Fixture(ProviderSearchWorkspaceSession),
}

impl TursoMemorySearchBackend {
    pub fn new(
        session: agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
        project_root: impl Into<String>,
        schema_id: impl Into<String>,
        schema_version: impl Into<String>,
        binding: TursoMemorySearchBinding,
    ) -> Result<Self, String> {
        Self::with_session(
            TursoMemorySearchSession::Resident(session),
            project_root,
            schema_id,
            schema_version,
            binding,
        )
    }

    pub(crate) fn new_fixture(
        session: ProviderSearchWorkspaceSession,
        project_root: impl Into<String>,
        schema_id: impl Into<String>,
        schema_version: impl Into<String>,
        binding: TursoMemorySearchBinding,
    ) -> Result<Self, String> {
        Self::with_session(
            TursoMemorySearchSession::Fixture(session),
            project_root,
            schema_id,
            schema_version,
            binding,
        )
    }

    fn with_session(
        session: TursoMemorySearchSession,
        project_root: impl Into<String>,
        schema_id: impl Into<String>,
        schema_version: impl Into<String>,
        binding: TursoMemorySearchBinding,
    ) -> Result<Self, String> {
        let project_root = project_root.into();
        let schema_id = schema_id.into();
        let schema_version = schema_version.into();
        for (field, value) in [
            ("projectRoot", project_root.as_str()),
            ("schemaId", schema_id.as_str()),
            ("schemaVersion", schema_version.as_str()),
            ("languageId", binding.language_id.as_str()),
            ("providerId", binding.provider_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!(
                    "Turso Memory Search binding {field} must not be empty"
                ));
            }
        }
        if binding.parser_identity_digest.len() != 64 || binding.query_pack_digest.len() != 64 {
            return Err(
                "Turso Memory Search parser and query-pack digests must be 64 hex characters"
                    .to_string(),
            );
        }
        Ok(Self {
            session,
            project_root,
            schema_id,
            schema_version,
            binding,
        })
    }

    pub async fn resolve(
        &self,
        request: &MemorySearchRequest,
    ) -> Result<MemorySearchResolution, String> {
        let started = Instant::now();
        if request.canonical_item_selector.language_id.as_str() != self.binding.language_id {
            return Err(format!(
                "Turso Memory Search language binding mismatch: bound={} requested={}",
                self.binding.language_id,
                request.canonical_item_selector.language_id.as_str(),
            ));
        }
        let query = TursoResidentSelectorQuery {
            project_root: self.project_root.clone(),
            schema_id: self.schema_id.clone(),
            schema_version: self.schema_version.clone(),
            provider_id: self.binding.provider_id.clone(),
            parser_identity_digest: self.binding.parser_identity_digest.clone(),
            query_pack_digest: self.binding.query_pack_digest.clone(),
            owner_path: request.requested_owner_path.clone(),
            canonical_item_selector: request.canonical_item_selector.clone(),
        };
        let read = match &self.session {
            TursoMemorySearchSession::Resident(session) => {
                session.read_resident_selector(&query).await?
            }
            TursoMemorySearchSession::Fixture(session) => {
                session.read_resident_selector(&query).await?
            }
        }
            .ok_or_else(|| {
                format!(
                    "Turso Memory Search has no active generation: projectRoot={} schemaId={} schemaVersion={}",
                    self.project_root, self.schema_id, self.schema_version,
                )
            })?;
        if read.source_snapshot.leaf_count != read.owner_count as usize {
            return Err(format!(
                "Turso Memory Search generation is incomplete: leafCount={} ownerCount={}",
                read.source_snapshot.leaf_count, read.owner_count,
            ));
        }
        let candidates = read
            .candidates
            .into_iter()
            .map(|candidate| MemorySearchItem {
                owner_path: candidate.owner_path,
                owner_content_digest: candidate.owner_content_digest,
                canonical_item_selector: candidate.canonical_item_selector,
            })
            .collect::<Vec<_>>();
        let (state, resolved) = if request.expected_generation_id != read.generation_id {
            (MemorySearchResolutionState::GenerationMismatch, None)
        } else if let Some(hit) = candidates
            .iter()
            .find(|candidate| candidate.owner_path == request.requested_owner_path)
            .cloned()
        {
            (MemorySearchResolutionState::LiveHit, Some(hit))
        } else {
            match candidates.as_slice() {
                [candidate] => (
                    MemorySearchResolutionState::LiveRelocated,
                    Some(candidate.clone()),
                ),
                [] if !read.actual_kinds.is_empty() => {
                    (MemorySearchResolutionState::KindMismatch, None)
                }
                [] if read.requested_owner_exists => {
                    (MemorySearchResolutionState::ItemMissing, None)
                }
                [] => (MemorySearchResolutionState::OwnerMissing, None),
                _ => (MemorySearchResolutionState::Ambiguous, None),
            }
        };
        let candidate_count = candidates.len();
        Ok(MemorySearchResolution {
            state,
            resolved,
            candidates,
            actual_kinds: read.actual_kinds,
            generation: MemorySearchGenerationReceipt {
                generation_id: read.generation_id,
                root_digest: read.source_snapshot.root_digest,
                root_depth: 0,
                leaf_count: read.source_snapshot.leaf_count,
                owner_count: read.owner_count as usize,
                selector_count: read.selector_count as usize,
            },
            performance: MemorySearchPerformanceReceipt {
                generation_load_micros: 0,
                index_lookup_micros: started.elapsed().as_micros().try_into().unwrap_or(u64::MAX),
                candidate_count,
                source_bytes_materialized: 0,
                db_opens: 0,
                db_queries: read.database_query_count as usize,
                provider_subprocesses: 0,
                cache_writes: 0,
            },
        })
    }
}
