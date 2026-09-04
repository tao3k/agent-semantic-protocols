use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;

use agent_semantic_client_core::state_core::ResolvedState;
use agent_semantic_client_db::ProviderIncrementalScoped;
use tempfile::TempDir;

pub(crate) struct TestDir(TempDir);

impl TestDir {
    pub(crate) fn new(label: &str) -> Self {
        let repository = gix::discover(env!("CARGO_MANIFEST_DIR"))
            .expect("discover the owner-backed test repository with Gix");
        let worktree = repository
            .worktree()
            .expect("workspace database tests require a non-bare owner checkout");
        let fixture_root = worktree.base().join("target/asp-live-project-fixtures");
        std::fs::create_dir_all(&fixture_root)
            .expect("create owner-backed live-project fixture root");
        let fixture = tempfile::Builder::new()
            .prefix(&format!("{label}-"))
            .tempdir_in(fixture_root)
            .expect("create isolated owner-backed live-project fixture");
        Self(fixture)
    }

    pub(crate) fn path(&self) -> &Path {
        self.0.path()
    }
}

pub(crate) fn environment_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn performance_lock_is_acquirable() {
    drop(performance_lock());
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
