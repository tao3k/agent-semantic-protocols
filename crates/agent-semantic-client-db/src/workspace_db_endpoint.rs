//! Canonical identity and private socket binding for one workspace DB owner epoch.

use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::net::UnixListener;

const MAX_UNIX_SOCKET_PATH_BYTES: usize = 103;

unsafe extern "C" {
    fn getuid() -> u32;
}

/// Return the canonical short, UID-private runtime root for resident owner
/// sockets. The endpoint receipt remains in project state; only the
/// Unix-domain socket uses this platform-bounded path.
pub fn workspace_db_owner_runtime_base() -> PathBuf {
    PathBuf::from("/tmp").join(format!("asp-wdb-{}", unsafe { getuid() }))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDbOwnerEndpoint {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub owner_artifact_digest: String,
    pub owner_epoch: u64,
    pub binding_token: String,
    pub socket_path: String,
}

impl WorkspaceDbOwnerEndpoint {
    pub fn validate_for_workspace(
        &self,
        workspace_identity: &str,
        owner_artifact_digest: &str,
    ) -> Result<(), String> {
        if self.schema_id != super::workspace_db_ipc::WORKSPACE_DB_OWNER_ENDPOINT_SCHEMA_ID
            || self.schema_version != super::workspace_db_ipc::WORKSPACE_DB_OWNER_SCHEMA_VERSION
        {
            return Err("workspace owner endpoint schema identity mismatch".to_owned());
        }
        if self.workspace_identity != workspace_identity {
            return Err(format!(
                "workspace owner endpoint identity mismatch: expected={workspace_identity} actual={}",
                self.workspace_identity
            ));
        }
        if self.owner_artifact_digest != owner_artifact_digest {
            return Err(format!(
                "workspace owner endpoint artifact mismatch: expected={owner_artifact_digest} actual={}",
                self.owner_artifact_digest
            ));
        }
        if self.owner_epoch == 0 || self.binding_token.is_empty() || self.socket_path.is_empty() {
            return Err("workspace owner endpoint is incomplete".to_owned());
        }
        if !Path::new(&self.socket_path).is_absolute() {
            return Err("workspace owner endpoint socket path must be absolute".to_owned());
        }
        Ok(())
    }
}

pub fn prepare_workspace_db_owner_endpoint(
    runtime_base: &Path,
    workspace_identity: &str,
    owner_artifact_digest: &str,
    owner_epoch: u64,
    binding_token: &str,
) -> Result<WorkspaceDbOwnerEndpoint, String> {
    if workspace_identity.is_empty()
        || owner_artifact_digest.is_empty()
        || binding_token.is_empty()
        || owner_epoch == 0
    {
        return Err("workspace owner endpoint requires identity, epoch, and token".to_owned());
    }
    std::fs::create_dir_all(runtime_base).map_err(|error| {
        format!(
            "failed to create workspace owner runtime directory {}: {error}",
            runtime_base.display()
        )
    })?;
    std::fs::set_permissions(runtime_base, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("failed to protect workspace owner runtime directory: {error}"))?;
    let metadata = runtime_base
        .metadata()
        .map_err(|error| format!("failed to inspect workspace owner runtime directory: {error}"))?;
    let current_uid = unsafe { getuid() };
    if metadata.uid() != current_uid || metadata.mode() & 0o777 != 0o700 {
        return Err(
            "workspace owner runtime directory is not private to the current UID".to_owned(),
        );
    }
    let digest = blake3::hash(workspace_identity.as_bytes()).to_hex();
    let socket_path = runtime_base.join(format!("w-{}.sock", &digest[..16]));
    if socket_path.as_os_str().as_bytes().len() > MAX_UNIX_SOCKET_PATH_BYTES {
        return Err(format!(
            "workspace owner socket path exceeds Unix sun_path budget: {}",
            socket_path.display()
        ));
    }
    Ok(WorkspaceDbOwnerEndpoint {
        schema_id: super::workspace_db_ipc::WORKSPACE_DB_OWNER_ENDPOINT_SCHEMA_ID.to_owned(),
        schema_version: super::workspace_db_ipc::WORKSPACE_DB_OWNER_SCHEMA_VERSION.to_owned(),
        workspace_identity: workspace_identity.to_owned(),
        owner_artifact_digest: owner_artifact_digest.to_owned(),
        owner_epoch,
        binding_token: binding_token.to_owned(),
        socket_path: socket_path.to_string_lossy().into_owned(),
    })
}

pub fn bind_workspace_db_owner(
    endpoint: &WorkspaceDbOwnerEndpoint,
) -> Result<UnixListener, String> {
    UnixListener::bind(PathBuf::from(&endpoint.socket_path))
        .map_err(|error| format!("failed to bind workspace owner endpoint: {error}"))
}
