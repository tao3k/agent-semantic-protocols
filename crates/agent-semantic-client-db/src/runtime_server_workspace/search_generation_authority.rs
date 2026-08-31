use serde::{Deserialize, Serialize};

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::WorkspaceGenerationLease;

pub const WORKSPACE_SEARCH_GENERATION_AUTHORITY_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-search-generation-authority.v2";
const MAX_WORKSPACE_SEARCH_GENERATION_AUTHORITY_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchGenerationAuthority {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub project_root: String,
    pub active_epoch: u64,
    pub generation_digest: String,
    pub owner_merkle_root_digest: String,
    pub search_projection_manifest_digest: String,
    pub search_projection_analyzer_digest: String,
    pub provider_schema_digest: String,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub project_resolutions: Vec<agent_semantic_runtime::AdmittedProjectResolution>,
    pub workspace_generation: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
}

impl WorkspaceSearchGenerationAuthority {
    /// Projects compact search authority from a validated resident lease.
    ///
    /// Complete generation validation belongs to publication. Repeating it on
    /// this read path would turn every search into an O(workspace) operation.
    pub fn from_lease(lease: &WorkspaceGenerationLease) -> Result<Self, String> {
        Self::from_generation(lease.generation())
    }

    pub fn from_generation(generation: &super::WorkspaceMemoryGeneration) -> Result<Self, String> {
        let mut owners = generation.owners.iter().collect::<Vec<_>>();
        owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
        let owner_merkle_root_digest = agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1::from_file_digests(
            owners.iter().map(|owner| (
                owner.owner_path.clone(),
                agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(&owner.bytes),
            )),
        )
        .map(|tree| format!("blake3-256:{}", tree.root_digest().as_str()))
        .map_err(|error| format!("build compact search authority owner Merkle tree: {error}"))?;
        let search_projection_manifest =
            super::search_index_projection::build_merkle_search_generation(generation)?;
        Ok(Self::from_generation_with_projection_digests(
            generation,
            owner_merkle_root_digest,
            search_projection_manifest.manifest_digest().to_owned(),
        ))
    }

    pub(super) fn from_generation_with_projection_digests(
        generation: &super::WorkspaceMemoryGeneration,
        owner_merkle_root_digest: String,
        search_projection_manifest_digest: String,
    ) -> Self {
        Self {
            schema_id: WORKSPACE_SEARCH_GENERATION_AUTHORITY_SCHEMA_ID.to_owned(),
            schema_version: "2".to_owned(),
            workspace_identity: generation.workspace_identity.clone(),
            project_root: generation.project_root.clone(),
            active_epoch: generation.active_epoch,
            generation_digest: generation.generation_digest.clone(),
            owner_merkle_root_digest,
            search_projection_manifest_digest,
            search_projection_analyzer_digest:
                agent_semantic_search::search_projection_analyzer_digest(),
            provider_schema_digest: generation.provider_schema_digest.clone(),
            source_snapshot: generation.source_snapshot.clone(),
            project_resolutions: generation.project_resolutions.clone(),
            workspace_generation: generation.workspace_generation.clone(),
        }
    }

    pub fn validate_binding(
        &self,
        workspace_identity: &str,
        project_root: &str,
    ) -> Result<(), String> {
        if self.schema_id != WORKSPACE_SEARCH_GENERATION_AUTHORITY_SCHEMA_ID
            || self.schema_version != "2"
            || self.workspace_identity != workspace_identity
            || self.project_root != project_root
            || self.active_epoch == 0
            || !self.generation_digest.starts_with("blake3-256:")
            || !self.owner_merkle_root_digest.starts_with("blake3-256:")
            || !self
                .search_projection_manifest_digest
                .starts_with("blake3-256:")
            || self.search_projection_analyzer_digest
                != agent_semantic_search::search_projection_analyzer_digest()
            || self.provider_schema_digest.trim().is_empty()
        {
            return Err(format!(
                "Runtime Server search generation authority binding mismatch: expectedWorkspace={} actualWorkspace={} expectedProjectRoot={} actualProjectRoot={}",
                workspace_identity, self.workspace_identity, project_root, self.project_root,
            ));
        }
        if self.source_snapshot.root_digest != self.workspace_generation.root_digest
            || self.workspace_generation.leaf_count
                != u64::try_from(self.source_snapshot.leaf_count)
                    .map_err(|_| "search generation authority leaf count overflow".to_owned())?
        {
            return Err("Runtime Server search generation authority evidence drift".to_owned());
        }
        Ok(())
    }
}

fn search_generation_authority_segment_path(
    generation_pointer_path: &Path,
    active_epoch: u64,
) -> Result<PathBuf, String> {
    let parent = generation_pointer_path
        .parent()
        .ok_or_else(|| "search generation authority pointer has no parent directory".to_owned())?;
    Ok(parent.join(format!("search-authority-{active_epoch}.json")))
}

pub(crate) async fn read_search_generation_authority_segment(
    generation_pointer_path: &Path,
    active_epoch: u64,
    workspace_identity: &str,
    project_root: &str,
) -> Result<WorkspaceSearchGenerationAuthority, String> {
    let path = search_generation_authority_segment_path(generation_pointer_path, active_epoch)?;
    let bytes = tokio::fs::read(&path).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "active-generation-required: search generation authority missing at {}",
                path.display()
            )
        } else {
            format!(
                "read compact search generation authority {}: {error}",
                path.display()
            )
        }
    })?;
    let authority =
        serde_json::from_slice::<WorkspaceSearchGenerationAuthority>(&bytes).map_err(|error| {
            format!("authority-corrupt: decode compact search generation authority: {error}")
        })?;
    authority.validate_binding(workspace_identity, project_root)?;
    if authority.active_epoch != active_epoch {
        return Err(format!(
            "compact search generation authority epoch mismatch: expected={active_epoch} actual={}",
            authority.active_epoch
        ));
    }
    Ok(authority)
}

/// Narrow fixture seam for external lifecycle/source-authority tests.
#[doc(hidden)]
pub async fn read_search_generation_authority_fixture(
    generation_pointer_path: &Path,
    active_epoch: u64,
    workspace_identity: &str,
    project_root: &str,
) -> Result<WorkspaceSearchGenerationAuthority, String> {
    read_search_generation_authority_segment(
        generation_pointer_path,
        active_epoch,
        workspace_identity,
        project_root,
    )
    .await
}

#[derive(Debug)]
pub(crate) struct WorkspaceResidentSearchGeneration {
    authority: Arc<WorkspaceSearchGenerationAuthority>,
    data_plane: Arc<super::WorkspaceSearchGenerationDataPlaneClient>,
    exact_projection: Arc<super::WorkspaceExactProjectionDataPlaneClient>,
}

impl WorkspaceResidentSearchGeneration {
    pub(crate) fn new(
        authority: Arc<WorkspaceSearchGenerationAuthority>,
        data_plane: Arc<super::WorkspaceSearchGenerationDataPlaneClient>,
        exact_projection: Arc<super::WorkspaceExactProjectionDataPlaneClient>,
    ) -> Result<Self, String> {
        if data_plane.authority() != authority.as_ref() {
            return Err("workspace resident search generation authority drift".to_owned());
        }
        Ok(Self {
            authority,
            data_plane,
            exact_projection,
        })
    }

    pub(crate) fn authority(&self) -> &Arc<WorkspaceSearchGenerationAuthority> {
        &self.authority
    }

    pub(crate) fn data_plane(&self) -> &Arc<super::WorkspaceSearchGenerationDataPlaneClient> {
        &self.data_plane
    }

    pub(crate) fn exact_projection(&self) -> &Arc<super::WorkspaceExactProjectionDataPlaneClient> {
        &self.exact_projection
    }
}

enum WorkspaceSearchGenerationAuthorityCommand {
    Publish {
        generation: Arc<WorkspaceResidentSearchGeneration>,
        completed:
            tokio::sync::oneshot::Sender<Result<Arc<WorkspaceResidentSearchGeneration>, String>>,
    },
    Shutdown(tokio::sync::oneshot::Sender<()>),
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceSearchGenerationAuthorityPublisher {
    commands: tokio::sync::mpsc::Sender<WorkspaceSearchGenerationAuthorityCommand>,
    task: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceSearchGenerationAuthorityReader {
    current: tokio::sync::watch::Receiver<Option<Arc<WorkspaceResidentSearchGeneration>>>,
}

pub(crate) fn workspace_search_generation_authority_channel(
    command_capacity: usize,
) -> (
    WorkspaceSearchGenerationAuthorityPublisher,
    WorkspaceSearchGenerationAuthorityReader,
) {
    let (current_sender, current) = tokio::sync::watch::channel(None);
    let (commands, mut receiver) = tokio::sync::mpsc::channel(command_capacity.max(1));
    let task = tokio::spawn(async move {
        let mut committed: Option<Arc<WorkspaceResidentSearchGeneration>> = None;
        while let Some(command) = receiver.recv().await {
            match command {
                WorkspaceSearchGenerationAuthorityCommand::Publish {
                    generation,
                    completed,
                } => {
                    let result = publish_resident_generation(&mut committed, generation);
                    if let Ok(generation) = &result {
                        current_sender.send_replace(Some(Arc::clone(generation)));
                    }
                    let _ = completed.send(result);
                }
                WorkspaceSearchGenerationAuthorityCommand::Shutdown(completed) => {
                    receiver.close();
                    current_sender.send_replace(None);
                    let _ = completed.send(());
                    break;
                }
            }
        }
    });
    (
        WorkspaceSearchGenerationAuthorityPublisher {
            commands,
            task: Arc::new(tokio::sync::Mutex::new(Some(task))),
        },
        WorkspaceSearchGenerationAuthorityReader { current },
    )
}

impl WorkspaceSearchGenerationAuthorityPublisher {
    pub(crate) async fn publish(
        &self,
        generation: Arc<WorkspaceResidentSearchGeneration>,
    ) -> Result<Arc<WorkspaceResidentSearchGeneration>, String> {
        let (completed, response) = tokio::sync::oneshot::channel();
        self.commands
            .send(WorkspaceSearchGenerationAuthorityCommand::Publish {
                generation,
                completed,
            })
            .await
            .map_err(|_| "workspace search generation authority is closed".to_owned())?;
        response.await.map_err(|_| {
            "workspace search generation authority closed without a receipt".to_owned()
        })?
    }

    pub(crate) async fn shutdown(&self) -> Result<(), String> {
        let (completed, response) = tokio::sync::oneshot::channel();
        if self
            .commands
            .send(WorkspaceSearchGenerationAuthorityCommand::Shutdown(
                completed,
            ))
            .await
            .is_ok()
        {
            let _ = response.await;
        }
        if let Some(task) = self.task.lock().await.take() {
            task.await
                .map_err(|error| format!("join workspace search generation authority: {error}"))?;
        }
        Ok(())
    }
}

impl WorkspaceSearchGenerationAuthorityReader {
    pub(crate) fn observed(&self) -> Option<Arc<WorkspaceResidentSearchGeneration>> {
        self.current.borrow().as_ref().map(Arc::clone)
    }
}

fn publish_resident_generation(
    committed: &mut Option<Arc<WorkspaceResidentSearchGeneration>>,
    generation: Arc<WorkspaceResidentSearchGeneration>,
) -> Result<Arc<WorkspaceResidentSearchGeneration>, String> {
    let authority = generation.authority();
    authority.validate_binding(&authority.workspace_identity, &authority.project_root)?;
    if let Some(current) = committed.as_ref() {
        let current_authority = current.authority();
        if authority.workspace_identity != current_authority.workspace_identity
            || authority.project_root != current_authority.project_root
        {
            return Err("workspace search generation authority binding changed".to_owned());
        }
        if authority.active_epoch < current_authority.active_epoch {
            return Err("workspace search generation authority epoch regressed".to_owned());
        }
        if authority.active_epoch == current_authority.active_epoch {
            if authority.as_ref() == current_authority.as_ref() {
                return Ok(Arc::clone(current));
            }
            return Err("workspace search generation authority epoch was reused".to_owned());
        }
    }
    *committed = Some(Arc::clone(&generation));
    Ok(generation)
}

pub(crate) async fn publish_search_generation_authority_segment(
    generation_pointer_path: &Path,
    generation: &super::WorkspaceMemoryGeneration,
) -> Result<Arc<WorkspaceSearchGenerationAuthority>, String> {
    let authority = WorkspaceSearchGenerationAuthority::from_generation(generation)?;
    authority.validate_binding(&generation.workspace_identity, &generation.project_root)?;
    let bytes = serde_json::to_vec(&authority)
        .map_err(|error| format!("encode compact search generation authority: {error}"))?;
    if bytes.len() > MAX_WORKSPACE_SEARCH_GENERATION_AUTHORITY_BYTES {
        return Err(format!(
            "search generation authority exceeds the bounded publication limit: bytes={} limit={MAX_WORKSPACE_SEARCH_GENERATION_AUTHORITY_BYTES}",
            bytes.len(),
        ));
    }
    let path =
        search_generation_authority_segment_path(generation_pointer_path, generation.active_epoch)?;
    let temporary = path.with_extension("json.pending");
    let mut file = tokio::fs::File::create(&temporary).await.map_err(|error| {
        format!(
            "create compact search generation authority {}: {error}",
            temporary.display()
        )
    })?;
    use tokio::io::AsyncWriteExt;
    file.write_all(&bytes).await.map_err(|error| {
        format!(
            "write compact search generation authority {}: {error}",
            temporary.display()
        )
    })?;
    file.sync_all().await.map_err(|error| {
        format!(
            "sync compact search generation authority {}: {error}",
            temporary.display()
        )
    })?;
    drop(file);
    tokio::fs::rename(&temporary, &path)
        .await
        .map_err(|error| {
            format!(
                "publish compact search generation authority {}: {error}",
                path.display()
            )
        })?;
    let parent = path
        .parent()
        .ok_or_else(|| "search generation authority path has no parent".to_owned())?;
    let directory = tokio::fs::File::open(parent)
        .await
        .map_err(|error| format!("open authority directory for durability: {error}"))?;
    directory
        .sync_all()
        .await
        .map_err(|error| format!("sync authority directory after atomic rename: {error}"))?;
    Ok(Arc::new(authority))
}
