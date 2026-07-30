use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use crate::workspace_db_endpoint::WorkspaceDbOwnerEndpoint;

/// Process-lifetime election lease for the single resident owner of one
/// workspace. The lock is acquired before Turso is opened, so losing launchers
/// never contend for the database file.
pub struct WorkspaceDbOwnerElection {
    _file: std::fs::File,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceDbOwnerRetirement {
    Absent,
    ElectionHeld,
    Retired,
}

impl Drop for WorkspaceDbOwnerElection {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self._file);
    }
}

pub fn try_acquire_workspace_db_owner_election(
    runtime_base: &Path,
    workspace_identity: &str,
) -> Result<Option<WorkspaceDbOwnerElection>, String> {
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
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    let current_uid = unsafe { getuid() };
    if metadata.uid() != current_uid || metadata.mode() & 0o777 != 0o700 {
        return Err(
            "workspace owner runtime directory is not private to the current UID".to_owned(),
        );
    }
    let digest = blake3::hash(workspace_identity.as_bytes()).to_hex();
    let lock_path = runtime_base.join(format!("w-{}.owner.lock", &digest[..16]));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(&lock_path)
        .map_err(|error| {
            format!(
                "failed to open workspace owner election lock {}: {error}",
                lock_path.display()
            )
        })?;
    match fs2::FileExt::try_lock_exclusive(&file) {
        Ok(()) => Ok(Some(WorkspaceDbOwnerElection { _file: file })),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(format!(
            "failed to acquire workspace owner election lock {}: {error}",
            lock_path.display()
        )),
    }
}

pub fn remove_stale_workspace_db_owner_socket(
    _election: &WorkspaceDbOwnerElection,
    endpoint: &WorkspaceDbOwnerEndpoint,
) -> Result<(), String> {
    match std::fs::remove_file(&endpoint.socket_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale workspace owner socket {}: {error}",
            endpoint.socket_path
        )),
    }
}

pub fn try_retire_workspace_db_owner_endpoint(
    runtime_base: &Path,
    workspace_identity: &str,
    endpoint_path: &Path,
) -> Result<WorkspaceDbOwnerRetirement, String> {
    let Some(_election) =
        try_acquire_workspace_db_owner_election(runtime_base, workspace_identity)?
    else {
        return Ok(WorkspaceDbOwnerRetirement::ElectionHeld);
    };
    let bytes = match std::fs::read(endpoint_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WorkspaceDbOwnerRetirement::Absent);
        }
        Err(error) => {
            return Err(format!(
                "failed to read stale workspace resident endpoint {}: {error}",
                endpoint_path.display()
            ));
        }
    };
    let endpoint: WorkspaceDbOwnerEndpoint = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "stale workspace resident endpoint {} lacks recovery identity: {error}",
            endpoint_path.display()
        )
    })?;
    if endpoint.schema_id != super::workspace_db_ipc::WORKSPACE_DB_OWNER_ENDPOINT_SCHEMA_ID
        || endpoint.schema_version != super::workspace_db_ipc::WORKSPACE_DB_OWNER_SCHEMA_VERSION
        || endpoint.workspace_identity != workspace_identity
    {
        return Err(format!(
            "stale workspace resident endpoint {} has an invalid recovery identity",
            endpoint_path.display()
        ));
    }
    for path in [Path::new(&endpoint.socket_path), endpoint_path] {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to remove dead workspace resident endpoint {}: {error}",
                    path.display()
                ));
            }
        }
    }
    Ok(WorkspaceDbOwnerRetirement::Retired)
}

#[cfg(test)]
#[path = "../tests/unit/workspace_db_owner_election.rs"]
mod tests;
