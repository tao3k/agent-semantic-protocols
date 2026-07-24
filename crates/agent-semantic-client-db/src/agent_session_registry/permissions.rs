use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

#[cfg(unix)]
unsafe extern "C" {
    fn geteuid() -> u32;
}

pub(super) fn prepare_private_registry_path(db_path: &Path) -> Result<PathBuf, String> {
    let parent = db_path.parent().ok_or_else(|| {
        format!(
            "registryWriteStatus=denied reason=missing-parent path={}",
            db_path.display()
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| permission_error("create-directory", parent, error))?;
    verify_current_owner(parent)?;
    set_private_directory_permissions(parent)?;

    if !db_path.exists() {
        create_private_registry_file(db_path)?;
    }
    verify_current_owner(db_path)?;
    set_private_file_permissions(db_path)?;
    Ok(db_path.to_path_buf())
}

fn permission_error(operation: &str, path: &Path, error: std::io::Error) -> String {
    format!(
        "registryWriteStatus=denied reason=permission-error operation={operation} path={} error={error}",
        path.display()
    )
}

#[cfg(unix)]
fn current_uid() -> u32 {
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    unsafe { geteuid() }
}

#[cfg(unix)]
fn verify_current_owner(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| permission_error("inspect-owner", path, error))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "registryWriteStatus=denied reason=symlink-not-authoritative path={}",
            path.display()
        ));
    }
    let owner = metadata.uid();
    let current = current_uid();
    verify_owner_ids(path, owner, current)
}

#[cfg(unix)]
fn verify_owner_ids(path: &Path, owner: u32, current: u32) -> Result<(), String> {
    if owner != current {
        return Err(format!(
            "registryWriteStatus=denied reason=foreign-owner path={} ownerUid={owner} currentUid={current}; refusing-to-chown",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_current_owner(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn create_private_registry_file(path: &Path) -> Result<(), String> {
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map(|_| ())
        .map_err(|error| permission_error("create-registry", path, error))
}

#[cfg(not(unix))]
fn create_private_registry_file(path: &Path) -> Result<(), String> {
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map(|_| ())
        .map_err(|error| permission_error("create-registry", path, error))
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), String> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| permission_error("chmod-0700", path, error))
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<(), String> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| permission_error("chmod-0600", path, error))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(all(test, unix))]
#[path = "permissions_tests.rs"]
mod tests;
