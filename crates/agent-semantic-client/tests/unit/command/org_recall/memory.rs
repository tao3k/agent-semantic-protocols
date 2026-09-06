// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use super::state_home_memory_engine_binary;

static ASP_STATE_HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn memory_engine_binary_is_only_resolved_from_state_home_runtime() {
    let _lock = ASP_STATE_HOME_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = TestRoot::new();
    std::fs::create_dir(root.path().join(".git")).expect("git marker");
    let state_home = root.path().join("state-home");
    let _state_home = EnvGuard::set("ASP_STATE_HOME", &state_home);
    let state_paths =
        agent_semantic_runtime::project_state_paths(root.path()).expect("project state paths");
    std::fs::create_dir_all(&state_paths.runtime_bin_dir).expect("runtime bin");
    let state_binary = state_paths.runtime_bin_dir.join("asp-memory-engine");
    std::fs::write(&state_binary, b"state authority").expect("state binary");

    assert_eq!(
        state_home_memory_engine_binary(root.path()),
        Some(state_binary.clone())
    );

    std::fs::remove_file(&state_binary).expect("remove state binary");
    let project_binary = root.path().join(".bin").join("asp-memory-engine");
    std::fs::create_dir_all(project_binary.parent().expect("project bin parent"))
        .expect("project bin");
    std::fs::write(&project_binary, b"legacy project binary").expect("project binary");

    assert_eq!(state_home_memory_engine_binary(root.path()), None);
}

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "asp-memory-engine-authority-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("project root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct EnvGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &Path) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match self.previous.take() {
                Some(previous) => std::env::set_var(self.key, previous),
                None => std::env::remove_var(self.key),
            }
        }
    }
}
