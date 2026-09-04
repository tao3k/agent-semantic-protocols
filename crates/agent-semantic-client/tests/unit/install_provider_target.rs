use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

use super::resolve_provider_binary_install_target;

static ASP_STATE_HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn provider_install_target_uses_state_home_runtime_bin() {
    let _lock = ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let state_home = temp_state_home("install-target");
    let _state_home = StateHomeEnvGuard::set(&state_home);

    let target =
        resolve_provider_binary_install_target("rust", "asp-rust").expect("install target");

    assert_eq!(
        target.path,
        canonical_state_home(&state_home).join("runtime/bin/asp-rust")
    );
    assert_eq!(target.source, "state-home-runtime-bin");
}

#[test]
fn provider_install_target_accepts_logical_binary_override_under_state_home() {
    let _lock = ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let state_home = temp_state_home("logical-override");
    let _state_home = StateHomeEnvGuard::set(&state_home);

    let target = resolve_provider_binary_install_target("python", "custom-asp-python")
        .expect("logical provider override");

    assert_eq!(
        target.path,
        canonical_state_home(&state_home).join("runtime/bin/custom-asp-python")
    );
    assert_eq!(target.source, "state-home-runtime-bin");
}

#[test]
fn provider_install_target_rejects_paths_instead_of_creating_a_second_authority() {
    let _lock = ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let state_home = temp_state_home("reject-paths");
    let _state_home = StateHomeEnvGuard::set(&state_home);

    for binary in [".bin/asp-rust", "../asp-rust", "/tmp/asp-rust"] {
        let error = resolve_provider_binary_install_target("rust", binary)
            .expect_err("provider path override must fail");
        assert!(
            error.contains("logical basename resolved under State Home runtime/bin"),
            "{error}"
        );
    }
}

struct StateHomeEnvGuard {
    previous: Option<OsString>,
}

impl StateHomeEnvGuard {
    fn set(path: &Path) -> Self {
        let previous = std::env::var_os(agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV);
        unsafe {
            std::env::set_var(agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV, path);
        }
        Self { previous }
    }
}

impl Drop for StateHomeEnvGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe {
                std::env::set_var(
                    agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV,
                    value,
                );
            },
            None => unsafe {
                std::env::remove_var(agent_semantic_runtime::state_core::ASP_STATE_HOME_ENV);
            },
        }
    }
}

fn temp_state_home(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir();
    let canonical_temp_dir = std::fs::canonicalize(&temp_dir).unwrap_or(temp_dir);
    canonical_temp_dir.join(format!(
        "agent-semantic-protocol-{label}-{}-{unique}",
        std::process::id()
    ))
}

fn canonical_state_home(path: &Path) -> PathBuf {
    let parent = path.parent().expect("State Home parent");
    std::fs::canonicalize(parent)
        .unwrap_or_else(|_| parent.to_path_buf())
        .join(path.file_name().expect("State Home basename"))
}
