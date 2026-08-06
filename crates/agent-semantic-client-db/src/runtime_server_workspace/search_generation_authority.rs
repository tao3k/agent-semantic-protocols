use serde::{Deserialize, Serialize};

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use super::WorkspaceGenerationLease;

pub const WORKSPACE_SEARCH_GENERATION_AUTHORITY_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-search-generation-authority";
const MAX_WORKSPACE_SEARCH_GENERATION_AUTHORITY_BYTES: usize = 4 * 1024 * 1024;

static RESIDENT_SEARCH_AUTHORITIES: LazyLock<
    dashmap::DashMap<PathBuf, Arc<WorkspaceSearchGenerationAuthority>>,
> = LazyLock::new(dashmap::DashMap::new);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchGenerationAuthority {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub project_root: String,
    pub active_epoch: u64,
    pub generation_digest: String,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub project_resolutions: Vec<agent_semantic_runtime::AdmittedProjectResolution>,
    pub workspace_generation: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
}

impl WorkspaceSearchGenerationAuthority {
    /// Projects compact search authority from a validated resident lease.
    ///
    /// Complete generation validation belongs to publication. Repeating it on
    /// this read path would turn every search into an O(workspace) operation.
    pub fn from_lease(lease: &WorkspaceGenerationLease) -> Self {
        Self::from_generation(lease.generation())
    }

    pub fn from_generation(generation: &super::WorkspaceMemoryGeneration) -> Self {
        Self {
            schema_id: WORKSPACE_SEARCH_GENERATION_AUTHORITY_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: generation.workspace_identity.clone(),
            project_root: generation.project_root.clone(),
            active_epoch: generation.active_epoch,
            generation_digest: generation.generation_digest.clone(),
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
            || self.schema_version != "1"
            || self.workspace_identity != workspace_identity
            || self.project_root != project_root
            || self.active_epoch == 0
            || !self.generation_digest.starts_with("blake3-256:")
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
    if let Some(authority) = resident_search_generation_authority(generation_pointer_path) {
        authority.validate_binding(workspace_identity, project_root)?;
        if authority.active_epoch != active_epoch {
            return Err(format!(
                "resident search generation authority epoch mismatch: expected={active_epoch} actual={}",
                authority.active_epoch
            ));
        }
        return Ok(authority.as_ref().clone());
    }
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

pub(crate) fn resident_search_generation_authority(
    generation_pointer_path: &Path,
) -> Option<Arc<WorkspaceSearchGenerationAuthority>> {
    RESIDENT_SEARCH_AUTHORITIES
        .get(generation_pointer_path)
        .map(|authority| Arc::clone(authority.value()))
}

pub(crate) async fn publish_search_generation_authority_segment(
    generation_pointer_path: &Path,
    generation: &super::WorkspaceMemoryGeneration,
) -> Result<(), String> {
    let authority = WorkspaceSearchGenerationAuthority::from_generation(generation);
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
    RESIDENT_SEARCH_AUTHORITIES.insert(generation_pointer_path.to_path_buf(), Arc::new(authority));
    Ok(())
}
