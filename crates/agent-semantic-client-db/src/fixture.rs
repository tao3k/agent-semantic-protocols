//! Reusable in-process fixtures for exercising the resident workspace DB model.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

#[derive(Clone, Debug)]
struct FixtureWorkspaceBinding {
    workspace_identity: String,
    db_path: PathBuf,
}

/// Load-once fixture backed by the same registry, read leases, and single writer
/// queue used by the resident transport.
#[derive(Clone, Debug, Default)]
pub struct SourceIndexFixture {
    registry: Arc<crate::WorkspaceDbRegistry>,
    sessions: Arc<parking_lot::Mutex<HashMap<PathBuf, crate::ProviderSearchWorkspaceSession>>>,
    binding: Option<FixtureWorkspaceBinding>,
}

impl SourceIndexFixture {
    pub fn for_client_dir(fixture_client_dir: impl AsRef<std::path::Path>) -> Self {
        let workspace_identity = workspace_identity_from_fixture_dir(&fixture_client_dir);
        Self::for_client_dir_with_workspace_identity(fixture_client_dir, workspace_identity)
    }

    pub fn for_client_dir_with_workspace_identity(
        fixture_client_dir: impl AsRef<std::path::Path>,
        workspace_identity: impl Into<String>,
    ) -> Self {
        Self {
            registry: Arc::new(crate::WorkspaceDbRegistry::default()),
            sessions: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            binding: Some(FixtureWorkspaceBinding {
                workspace_identity: workspace_identity.into(),
                db_path: crate::ClientDbEngine::db_path_for_client_dir(fixture_client_dir),
            }),
        }
    }

    pub fn commit_source_index_generation(
        &self,
        mut request: crate::ClientDbSourceIndexRefreshRequest,
        source_blobs: &crate::ClientDbSourceIndexSourceBlobs,
    ) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
        let registry = Arc::clone(&self.registry);
        let sessions = Arc::clone(&self.sessions);
        let binding = self.binding.clone();
        let source_blobs = source_blobs.clone();
        crate::engine::facade::block_on_db_engine_async(async move {
            let project_root = request.import.project_root.clone();
            let session =
                fixture_session(&registry, &sessions, binding.as_ref(), &project_root).await?;
            let canonical_source_snapshot = canonical_source_snapshot(&request);
            request.source_snapshot = canonical_source_snapshot.clone();
            let materialization =
                crate::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
                    session.workspace_identity(),
                    &canonical_source_snapshot,
                    &request.import,
                    &source_blobs,
                    Vec::new(),
                )?;
            session
                .commit_source_index_generation(request, materialization)
                .await
        })
    }

    pub fn commit_source_index_generation_with_materialization(
        &self,
        request: crate::ClientDbSourceIndexRefreshRequest,
        materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    ) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
        let registry = Arc::clone(&self.registry);
        let sessions = Arc::clone(&self.sessions);
        let binding = self.binding.clone();
        crate::engine::facade::block_on_db_engine_async(async move {
            let project_root = request.import.project_root.clone();
            let session =
                fixture_session(&registry, &sessions, binding.as_ref(), &project_root).await?;
            session
                .commit_source_index_generation(request, materialization)
                .await
        })
    }

    pub fn load_active_workspace_generation_materialization(
        &self,
        project_root: impl AsRef<std::path::Path>,
    ) -> Result<Option<crate::runtime_server_workspace::WorkspaceCanonicalMaterialization>, String>
    {
        let registry = Arc::clone(&self.registry);
        let sessions = Arc::clone(&self.sessions);
        let binding = self.binding.clone();
        let project_root = project_root.as_ref().to_path_buf();
        crate::engine::facade::block_on_db_engine_async(async move {
            let session =
                fixture_session(&registry, &sessions, binding.as_ref(), &project_root).await?;
            session
                .load_active_workspace_generation_materialization(&project_root)
                .await
        })
    }

    pub fn counters(&self) -> crate::WorkspaceDbRegistryCounters {
        self.registry.counters()
    }

    pub fn read_source_index(
        &self,
        project_root: std::path::PathBuf,
        indexed_project_root: std::path::PathBuf,
        source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
        query: String,
        language_id: Option<agent_semantic_client_core::LanguageId>,
        limit: u32,
    ) -> Result<crate::ClientDbSourceIndexLookupResult, String> {
        let registry = Arc::clone(&self.registry);
        let sessions = Arc::clone(&self.sessions);
        let binding = self.binding.clone();
        crate::engine::facade::block_on_db_engine_async(async move {
            let session =
                fixture_session(&registry, &sessions, binding.as_ref(), &project_root).await?;
            session
                .read_source_index(
                    &indexed_project_root,
                    &source_snapshot,
                    &query,
                    language_id.as_ref(),
                    limit,
                )
                .await
        })
    }
}

async fn fixture_session(
    registry: &crate::WorkspaceDbRegistry,
    sessions: &parking_lot::Mutex<HashMap<PathBuf, crate::ProviderSearchWorkspaceSession>>,
    binding: Option<&FixtureWorkspaceBinding>,
    project_root: &std::path::Path,
) -> Result<crate::ProviderSearchWorkspaceSession, String> {
    if let Some(session) = sessions.lock().get(project_root).cloned() {
        return Ok(session);
    }
    let session = match binding {
        Some(binding) => {
            registry
                .bootstrap_fixture_workspace(&binding.workspace_identity, binding.db_path.clone())
                .await?
        }
        None => registry.bootstrap_workspace(project_root).await?,
    };
    sessions
        .lock()
        .insert(project_root.to_path_buf(), session.clone());
    Ok(session)
}

pub fn commit_source_index_generation(
    request: crate::ClientDbSourceIndexRefreshRequest,
    source_blobs: &crate::ClientDbSourceIndexSourceBlobs,
) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
    SourceIndexFixture::default().commit_source_index_generation(request, source_blobs)
}

pub fn workspace_identity_from_fixture_dir(
    fixture_client_dir: impl AsRef<std::path::Path>,
) -> String {
    let db_path = crate::ClientDbEngine::db_path_for_client_dir(fixture_client_dir);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.fixture-workspace-identity.v1");
    hasher.update(&[0]);
    hasher.update(db_path.to_string_lossy().as_bytes());
    format!("workspace-fixture-{}", &hasher.finalize().to_hex()[..16])
}

fn canonical_source_snapshot(
    request: &crate::ClientDbSourceIndexRefreshRequest,
) -> agent_semantic_content_identity::SourceSnapshotEvidence {
    agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        request
            .import
            .file_hashes
            .iter()
            .filter(|file| {
                request
                    .import
                    .owners
                    .iter()
                    .any(|owner| owner.owner_path.as_str() == file.path.as_str())
            })
            .map(|file| {
                (
                    file.path.as_str().to_owned(),
                    file.sha256.as_str().to_owned(),
                )
            }),
    )
    .evidence(
        request.source_snapshot.source_kind.clone(),
        request.source_snapshot.provider_digest.clone(),
    )
}
