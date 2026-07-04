use std::fs;
use std::path::{Path, PathBuf};

use super::{
    ensure_project_client_cache_dir, ensure_project_hook_cache_dir, ensure_project_hook_state_dir,
    ensure_project_provider_bin_dir, ensure_project_provider_lock_dir, ensure_project_runtime_home,
    project_runtime_state,
};

static ASP_STATE_HOME_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct AspStateHomeGuard {
    _guard: std::sync::MutexGuard<'static, ()>,
    path: PathBuf,
}

impl AspStateHomeGuard {
    fn new(label: &str) -> Self {
        let guard = ASP_STATE_HOME_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path = temp_root(label);
        unsafe {
            std::env::set_var("ASP_STATE_HOME", &path);
        }
        Self {
            _guard: guard,
            path,
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for AspStateHomeGuard {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("ASP_STATE_HOME");
        }
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn runtime_state_materializes_config_layout_under_git_toplevel() {
    let root = temp_root("runtime-state-git");
    let state_home = AspStateHomeGuard::new("runtime-state-home");
    let package_root = root.join("crates/example");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");

    let state = project_runtime_state(&package_root).expect("runtime state");

    assert_eq!(state.layout.git_toplevel.as_deref(), Some(root.as_path()));
    assert_eq!(state.protocol_home, state_home.path());
    assert!(
        state
            .hook_cache_dir
            .starts_with(state_home.path().join("projects/by-id"))
    );
    assert!(state.hook_cache_dir.ends_with("live/hooks/cache"));
    assert!(
        state
            .hook_state_dir
            .starts_with(state_home.path().join("projects/by-id"))
    );
    assert!(state.hook_state_dir.ends_with("live/hooks/state"));
    assert_eq!(
        state.activation_path,
        state.hook_state_dir.join("activation.json")
    );
    assert!(
        state
            .client_cache_dir
            .starts_with(state_home.path().join("projects/by-id"))
    );
    assert!(state.client_cache_dir.ends_with("live/client"));
    assert!(
        state
            .artifacts_dir
            .starts_with(state_home.path().join("projects/by-id"))
    );
    assert!(state.artifacts_dir.ends_with("artifacts"));
    assert_eq!(state.runtime_home, state_home.path().join("runtime"));
    assert_eq!(
        state.provider_bin_dir,
        state_home.path().join("runtime/bin")
    );
    assert_eq!(state.runtime_bin_dir, state_home.path().join("runtime/bin"));
    assert_eq!(
        state.provider_lock_dir,
        state_home.path().join("runtime/provider-locks")
    );
    assert!(state.hook_cache_dir.is_dir());
    assert!(state.hook_state_dir.is_dir());
    assert!(state.client_cache_dir.is_dir());
    assert!(state.artifacts_dir.is_dir());
    assert!(state.runtime_home.is_dir());
    assert!(state.provider_bin_dir.is_dir());
    assert!(state.provider_lock_dir.is_dir());
    assert!(!root.join(".cache").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ensure_helpers_create_only_the_requested_runtime_dir() {
    let root = temp_root("runtime-state-single-dir");
    let state_home = AspStateHomeGuard::new("runtime-state-single-dir-home");
    let package_root = root.join("crates/example");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");

    let hook_dir = ensure_project_hook_cache_dir(&package_root).expect("hook cache dir");

    assert!(hook_dir.is_dir());
    assert!(hook_dir.starts_with(state_home.path().join("projects/by-id")));
    assert!(hook_dir.ends_with("live/hooks/cache"));
    assert!(
        !hook_dir
            .parent()
            .expect("hook parent")
            .join("state")
            .exists()
    );

    let hook_state_dir = ensure_project_hook_state_dir(&package_root).expect("hook state dir");
    let client_dir = ensure_project_client_cache_dir(&package_root).expect("client cache dir");
    let runtime_home = ensure_project_runtime_home(&package_root).expect("runtime home");
    let provider_bin_dir =
        ensure_project_provider_bin_dir(&package_root).expect("provider bin dir");
    let provider_lock_dir =
        ensure_project_provider_lock_dir(&package_root).expect("provider lock dir");

    assert!(hook_state_dir.is_dir());
    assert_eq!(
        hook_state_dir,
        hook_dir.parent().expect("hook parent").join("state")
    );
    assert!(client_dir.is_dir());
    assert!(client_dir.starts_with(state_home.path().join("projects/by-id")));
    assert!(client_dir.ends_with("live/client"));
    assert!(runtime_home.is_dir());
    assert!(provider_bin_dir.is_dir());
    assert!(provider_lock_dir.is_dir());
    assert!(!root.join(".cache").exists());
    let _ = fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-runtime-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    canonical(&root)
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
