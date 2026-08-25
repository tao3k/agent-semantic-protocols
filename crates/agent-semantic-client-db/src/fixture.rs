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
            request.import.source_blobs = source_blobs;
            let mut materialization_import = request.import.clone();
            if let Some(binding) = binding.as_ref()
                && let Some(active) = crate::latest_turso_source_index_generation_snapshot(
                    &binding.db_path,
                    &project_root,
                    &request.import.schema_id,
                    &request.import.schema_version,
                )
                .await?
                && let Some(active_blobs) = crate::active_turso_source_index_generation_blobs(
                    &binding.db_path,
                    &project_root,
                    &request.import.schema_id,
                    &request.import.schema_version,
                )
                .await?
            {
                let changed_owner_paths = request
                    .import
                    .owners
                    .iter()
                    .map(|owner| owner.owner_path.as_str().to_owned())
                    .collect::<std::collections::BTreeSet<_>>();
                let successor_paths = request
                    .import
                    .file_hashes
                    .iter()
                    .map(|file_hash| file_hash.path.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                let removed_owner_paths = active
                    .owners
                    .iter()
                    .map(|owner| owner.owner_path.clone())
                    .filter(|owner_path| !successor_paths.contains(owner_path.as_str()))
                    .collect::<std::collections::BTreeSet<_>>();
                materialization_import = crate::overlay_active_source_index_import(
                    &active,
                    &active_blobs,
                    &request.import,
                    &changed_owner_paths,
                    &removed_owner_paths,
                )?;
            }
            let materialization_source_blobs = materialization_import.source_blobs.clone();
            let canonical_source_snapshot =
                canonical_source_snapshot(&request, &materialization_source_blobs);
            let canonical_workspace_snapshot =
                agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes(
                    materialization_source_blobs.iter(),
                );
            request.source_snapshot = canonical_source_snapshot.clone();
            let materialization =
                crate::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
                    session.workspace_identity(),
                    &canonical_workspace_snapshot,
                    &canonical_source_snapshot,
                    &materialization_import,
                    &materialization_source_blobs,
                    Vec::new(),
                )?;
            session
                .commit_source_index_generation(request, materialization)
                .await
                .map(|(receipt, _materialization)| receipt)
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
                .map(|(receipt, _materialization)| receipt)
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

pub fn projection_capability_manifest_fixture()
-> crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest {
    crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::from_source_index(
            "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
        &[],
    )
    .expect("projection capability manifest fixture must satisfy the v1 contract")
}

pub fn ready_projection_capability_fixture(
    workspace_identity: impl Into<String>,
    generation_digest: impl Into<String>,
    root_digest: impl Into<String>,
    publication_epoch: u64,
) -> crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityReceipt {
    use crate::active_generation_projection_capability::{
        ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_ID,
        ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_VERSION, ActiveGenerationCapabilityState,
        ActiveGenerationProjectionCapabilityReceipt, ActiveGenerationProjectionMode,
        ActiveGenerationSelectorCapability,
    };

    let receipt = ActiveGenerationProjectionCapabilityReceipt {
        schema_id: ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_ID.to_owned(),
        schema_version: ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_VERSION.to_owned(),
        state: ActiveGenerationCapabilityState::Ready,
        workspace_identity: workspace_identity.into(),
        generation_digest: generation_digest.into(),
        root_digest: root_digest.into(),
        provider_catalog_digest:
            "blake3-256:3333333333333333333333333333333333333333333333333333333333333333".to_owned(),
        provider_catalog_readable: true,
        publication_epoch,
        selectors: vec![ActiveGenerationSelectorCapability {
            selector: "rust://src/lib.rs#item/function/example".to_owned(),
            owner_path: "src/lib.rs".to_owned(),
            projection_modes: std::collections::BTreeSet::from([
                ActiveGenerationProjectionMode::Source,
                ActiveGenerationProjectionMode::CallableSkeleton,
            ]),
        }],
        failure: None,
    };
    receipt
        .validate()
        .expect("ready projection capability fixture must satisfy the v1 contract");
    receipt
}

fn canonical_source_snapshot(
    request: &crate::ClientDbSourceIndexRefreshRequest,
    source_blobs: &crate::ClientDbSourceIndexSourceBlobs,
) -> agent_semantic_content_identity::SourceSnapshotEvidence {
    agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        source_blobs
            .iter()
            .map(|(path, bytes)| (path.to_owned(), blake3::hash(bytes).to_hex().to_string())),
    )
    .evidence(
        request.source_snapshot.source_kind.clone(),
        request.source_snapshot.provider_digest.clone(),
    )
}
