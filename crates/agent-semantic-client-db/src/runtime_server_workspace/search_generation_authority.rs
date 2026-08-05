use serde::{Deserialize, Serialize};

use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

use super::{
    WorkspaceGenerationLease, WorkspaceGenerationPointerReader, WorkspaceGenerationSnapshot,
};

pub const WORKSPACE_SEARCH_GENERATION_AUTHORITY_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-search-generation-authority";
pub const WORKSPACE_SEARCH_GENERATION_AUTHORITY_OPEN_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-search-generation-authority-open-receipt";

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
    pub workspace_generation: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
}

impl WorkspaceSearchGenerationAuthority {
    /// Projects compact search authority from a validated resident lease.
    ///
    /// Complete generation validation belongs to publication. Repeating it on
    /// this read path would turn every search into an O(workspace) operation.
    pub fn from_lease(lease: &WorkspaceGenerationLease) -> Self {
        let generation = lease.generation();
        Self {
            schema_id: WORKSPACE_SEARCH_GENERATION_AUTHORITY_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: generation.workspace_identity.clone(),
            project_root: generation.project_root.clone(),
            active_epoch: generation.active_epoch,
            generation_digest: generation.generation_digest.clone(),
            source_snapshot: generation.source_snapshot.clone(),
            workspace_generation: generation.workspace_generation.clone(),
        }
    }

    pub fn from_snapshot(
        project_root: &str,
        snapshot: &WorkspaceGenerationSnapshot,
    ) -> Result<Self, String> {
        snapshot.validate()?;
        let raw_digest = |field: &str, digest: &str| -> Result<String, String> {
            let raw = digest.strip_prefix("blake3-256:").ok_or_else(|| {
                format!("workspace generation pointer {field} is not a qualified BLAKE3 digest")
            })?;
            if raw.len() != 64 || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(format!(
                    "workspace generation pointer {field} is not a canonical BLAKE3 digest"
                ));
            }
            Ok(raw.to_owned())
        };
        let mut source_snapshot = agent_semantic_content_identity::SourceSnapshotEvidence::new(
            raw_digest("sourceRootDigest", &snapshot.source_root_digest)?,
            snapshot.source_kind,
            usize::try_from(snapshot.leaf_count)
                .map_err(|_| "workspace generation pointer leaf count overflow".to_owned())?,
            raw_digest("sourceProviderDigest", &snapshot.source_provider_digest)?,
        );
        source_snapshot.base_root_digest = snapshot
            .base_root_digest
            .as_deref()
            .map(|digest| raw_digest("baseRootDigest", digest))
            .transpose()?;
        source_snapshot.dirty_paths_digest = snapshot
            .dirty_paths_digest
            .as_deref()
            .map(|digest| raw_digest("dirtyPathsDigest", digest))
            .transpose()?;
        let authority = Self {
            schema_id: WORKSPACE_SEARCH_GENERATION_AUTHORITY_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: snapshot.workspace_identity.clone(),
            project_root: project_root.to_owned(),
            active_epoch: snapshot.active_epoch,
            generation_digest: snapshot.generation_digest.clone(),
            workspace_generation:
                agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1 {
                    root_digest: source_snapshot.root_digest.clone(),
                    root_depth: u32::from(snapshot.root_depth[0]),
                    leaf_count: snapshot.leaf_count,
                    owner_count: snapshot.owner_count,
                },
            source_snapshot,
        };
        authority.validate_binding(&snapshot.workspace_identity, project_root)?;
        Ok(authority)
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchGenerationAuthorityOpenReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub authority: WorkspaceSearchGenerationAuthority,
    pub generation_pointer_path: String,
}

impl WorkspaceSearchGenerationAuthorityOpenReceipt {
    pub fn new(
        authority: WorkspaceSearchGenerationAuthority,
        generation_pointer_path: &Path,
    ) -> Result<Self, String> {
        let receipt = Self {
            schema_id: WORKSPACE_SEARCH_GENERATION_AUTHORITY_OPEN_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            authority,
            generation_pointer_path: generation_pointer_path.to_string_lossy().into_owned(),
        };
        receipt.validate_binding(
            &receipt.authority.workspace_identity,
            &receipt.authority.project_root,
        )?;
        Ok(receipt)
    }

    pub fn validate_binding(
        &self,
        workspace_identity: &str,
        project_root: &str,
    ) -> Result<(), String> {
        if self.schema_id != WORKSPACE_SEARCH_GENERATION_AUTHORITY_OPEN_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("search generation authority open receipt schema mismatch".to_owned());
        }
        self.authority
            .validate_binding(workspace_identity, project_root)?;
        let pointer_path = Path::new(&self.generation_pointer_path);
        if !pointer_path.is_absolute()
            || pointer_path.file_name().and_then(|name| name.to_str())
                != Some("active-generation.pointer")
        {
            return Err("search generation authority pointer locator is invalid".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct WorkspaceSearchGenerationAuthorityPointerClient {
    reader: WorkspaceGenerationPointerReader,
    workspace_identity: String,
    project_root: String,
}

pub type SharedWorkspaceSearchGenerationAuthorityPointerClient =
    Arc<WorkspaceSearchGenerationAuthorityPointerClient>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct WorkspaceSearchGenerationAuthorityPointerKey {
    generation_pointer_path: PathBuf,
    workspace_identity: String,
    project_root: String,
}

type SharedAuthorityPointerCell =
    tokio::sync::OnceCell<Option<SharedWorkspaceSearchGenerationAuthorityPointerClient>>;

static SHARED_AUTHORITY_POINTERS: LazyLock<
    dashmap::DashMap<WorkspaceSearchGenerationAuthorityPointerKey, Arc<SharedAuthorityPointerCell>>,
> = LazyLock::new(dashmap::DashMap::new);

impl WorkspaceSearchGenerationAuthorityPointerClient {
    fn shared_key(
        generation_pointer_path: &Path,
        workspace_identity: &str,
        project_root: &str,
    ) -> WorkspaceSearchGenerationAuthorityPointerKey {
        WorkspaceSearchGenerationAuthorityPointerKey {
            generation_pointer_path: generation_pointer_path.to_path_buf(),
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.to_owned(),
        }
    }

    pub fn shared_get(
        generation_pointer_path: &Path,
        workspace_identity: &str,
        project_root: &str,
    ) -> Option<SharedWorkspaceSearchGenerationAuthorityPointerClient> {
        let key = Self::shared_key(generation_pointer_path, workspace_identity, project_root);
        SHARED_AUTHORITY_POINTERS
            .get(&key)
            .and_then(|cell| cell.get().and_then(Clone::clone))
    }

    pub(crate) fn invalidate_committed_pointer(
        generation_pointer_path: &Path,
        workspace_identity: &str,
        project_root: &str,
    ) {
        SHARED_AUTHORITY_POINTERS.remove(&Self::shared_key(
            generation_pointer_path,
            workspace_identity,
            project_root,
        ));
    }

    pub async fn shared_open_path_optional(
        generation_pointer_path: &Path,
        workspace_identity: &str,
        project_root: &str,
    ) -> Result<Option<Arc<Self>>, String> {
        let key = Self::shared_key(generation_pointer_path, workspace_identity, project_root);
        let cell = SHARED_AUTHORITY_POINTERS
            .entry(key.clone())
            .or_insert_with(|| Arc::new(tokio::sync::OnceCell::new()))
            .clone();
        let pointer = cell
            .get_or_try_init(|| async {
                Self::open_path_optional(generation_pointer_path, workspace_identity, project_root)
                    .await
                    .map(|pointer| pointer.map(Arc::new))
            })
            .await?
            .clone();
        if pointer.is_none() {
            SHARED_AUTHORITY_POINTERS.remove(&key);
        }
        Ok(pointer)
    }

    pub async fn open_path(
        generation_pointer_path: &Path,
        workspace_identity: &str,
        project_root: &str,
    ) -> Result<Self, String> {
        let reader = WorkspaceGenerationPointerReader::open(generation_pointer_path).await?;
        let client = Self {
            reader,
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.to_owned(),
        };
        client.read()?;
        Ok(client)
    }

    pub async fn open_path_optional(
        generation_pointer_path: &Path,
        workspace_identity: &str,
        project_root: &str,
    ) -> Result<Option<Self>, String> {
        let Some(reader) =
            WorkspaceGenerationPointerReader::open_optional(generation_pointer_path).await?
        else {
            return Ok(None);
        };
        let client = Self {
            reader,
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.to_owned(),
        };
        client.read()?;
        Ok(Some(client))
    }

    pub async fn open(
        receipt: &WorkspaceSearchGenerationAuthorityOpenReceipt,
        workspace_identity: &str,
        project_root: &str,
    ) -> Result<Self, String> {
        receipt.validate_binding(workspace_identity, project_root)?;
        let client = Self::open_path(
            Path::new(&receipt.generation_pointer_path),
            workspace_identity,
            project_root,
        )
        .await?;
        let mapped = client.read()?;
        if mapped.active_epoch < receipt.authority.active_epoch {
            return Err(
                "search generation authority pointer is older than its open receipt".to_owned(),
            );
        }
        Ok(client)
    }

    pub fn read(&self) -> Result<WorkspaceSearchGenerationAuthority, String> {
        let snapshot = self.reader.read()?;
        if snapshot.workspace_identity != self.workspace_identity {
            return Err("search generation authority pointer workspace drift".to_owned());
        }
        WorkspaceSearchGenerationAuthority::from_snapshot(&self.project_root, &snapshot)
    }
}
