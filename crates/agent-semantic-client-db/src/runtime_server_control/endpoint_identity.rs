//! State Home identity and Runtime Server endpoint path derivation.

use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

unsafe extern "C" {
    fn getuid() -> u32;
}

pub fn runtime_server_runtime_base(state_home: &Path) -> Result<PathBuf, String> {
    let canonical_state_home = canonical_state_home_cache()
        .read()
        .map_err(|_| "Runtime Server State Home cache is poisoned".to_owned())?
        .get(state_home)
        .cloned();
    let canonical_state_home = match canonical_state_home {
        Some(identity) => identity,
        None => {
            let identity = std::fs::canonicalize(state_home).map_err(|error| {
                format!(
                    "failed to canonicalize ASP State Home {} for Runtime Server identity: {error}",
                    state_home.display()
                )
            })?;
            remember_state_home(state_home, &identity)?;
            identity
        }
    };
    runtime_server_runtime_base_for_identity(&canonical_state_home)
}

pub async fn runtime_server_runtime_base_async(state_home: &Path) -> Result<PathBuf, String> {
    let cached = canonical_state_home_cache()
        .read()
        .map_err(|_| "Runtime Server State Home cache is poisoned".to_owned())?
        .get(state_home)
        .cloned();
    let canonical_state_home = match cached {
        Some(identity) => identity,
        None => {
            let identity = tokio::fs::canonicalize(state_home).await.map_err(|error| {
                format!(
                    "failed to canonicalize ASP State Home {} for Runtime Server identity: {error}",
                    state_home.display()
                )
            })?;
            remember_state_home(state_home, &identity)?;
            identity
        }
    };
    runtime_server_runtime_base_for_identity(&canonical_state_home)
}

pub fn runtime_server_endpoint_path(state_home: &Path) -> Result<PathBuf, String> {
    Ok(runtime_server_runtime_base(state_home)?.join("endpoint.v1.json"))
}

pub async fn runtime_server_endpoint_path_async(state_home: &Path) -> Result<PathBuf, String> {
    Ok(runtime_server_runtime_base_async(state_home)
        .await?
        .join("endpoint.v1.json"))
}

fn runtime_server_runtime_base_for_identity(
    canonical_state_home: &Path,
) -> Result<PathBuf, String> {
    let mut identity = blake3::Hasher::new();
    identity.update(b"agent.semantic-protocols.runtime-server-state-home.v1\0");
    identity.update(canonical_state_home.as_os_str().as_bytes());
    let identity = identity.finalize().to_hex();
    Ok(PathBuf::from("/tmp")
        .join(format!("asp-runtime-server-{}", unsafe { getuid() }))
        .join(format!("state-{}", &identity[..24])))
}

fn remember_state_home(state_home: &Path, identity: &Path) -> Result<(), String> {
    canonical_state_home_cache()
        .write()
        .map_err(|_| "Runtime Server State Home cache is poisoned".to_owned())?
        .insert(state_home.to_path_buf(), identity.to_path_buf());
    Ok(())
}

fn canonical_state_home_cache() -> &'static RwLock<BTreeMap<PathBuf, PathBuf>> {
    static CACHE: OnceLock<RwLock<BTreeMap<PathBuf, PathBuf>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(BTreeMap::new()))
}
