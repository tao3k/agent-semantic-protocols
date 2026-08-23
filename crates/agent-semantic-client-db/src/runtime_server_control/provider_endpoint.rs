use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub(super) fn provider_plane_socket_path(
    runtime_base: &Path,
    endpoint_digest: &str,
    max_socket_path_bytes: usize,
) -> Result<PathBuf, String> {
    let path = runtime_base.join(format!("r-{}.providers.sock", &endpoint_digest[..16]));
    if path.as_os_str().as_bytes().len() > max_socket_path_bytes {
        return Err(format!(
            "Runtime Server provider-plane socket path exceeds Unix sun_path budget: {}",
            path.display()
        ));
    }
    Ok(path)
}

pub fn provider_register_state_path(provider_plane_socket_path: &Path) -> Result<PathBuf, String> {
    let runtime_base = provider_plane_socket_path.parent().ok_or_else(|| {
        "Runtime Server provider-plane socket path has no runtime directory".to_owned()
    })?;
    Ok(runtime_base.join("provider-register-state.json"))
}
