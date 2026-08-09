use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use agent_semantic_client_core::state_core::ResolvedState;
use agent_semantic_client_db::ProviderIncrementalScoped;

pub(crate) fn environment_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn performance_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) struct StateHomeGuard {
    previous: Option<OsString>,
}

impl StateHomeGuard {
    pub(crate) fn install(state_home: &Path) -> Self {
        let previous = std::env::var_os("ASP_STATE_HOME");
        unsafe {
            std::env::set_var("ASP_STATE_HOME", state_home);
        }
        Self { previous }
    }
}

impl Drop for StateHomeGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(previous) = &self.previous {
                std::env::set_var("ASP_STATE_HOME", previous);
            } else {
                std::env::remove_var("ASP_STATE_HOME");
            }
        }
    }
}

pub(crate) fn workspace(
    parent: &Path,
    name: &str,
) -> (PathBuf, ResolvedState, ProviderIncrementalScoped) {
    let project_root = parent.join(name);
    std::fs::create_dir_all(&project_root).expect("create workspace database test project");
    let git_init = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&project_root)
        .output()
        .expect("run git init for Gix-owned workspace identity");
    assert!(
        git_init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&git_init.stderr)
    );
    let project_root =
        std::fs::canonicalize(project_root).expect("canonicalize workspace database test project");
    let resolved = ResolvedState::resolve(&project_root).expect("resolve workspace database state");
    let scope = ProviderIncrementalScoped {
        project_root: project_root.to_string_lossy().into_owned(),
        workspace_identity: resolved.workspace.workspace_id.as_str().to_owned(),
        provider_workspace_identity_digest: format!("{:064x}", 17),
        language_id: "rust".to_owned(),
        provider_id: "rust-test-provider".to_owned(),
        provider_workspace_root: project_root.to_string_lossy().into_owned(),
    };
    (project_root, resolved, scope)
}
