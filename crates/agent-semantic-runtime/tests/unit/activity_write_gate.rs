// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! State activity write-gate tests.

use agent_semantic_runtime::project_runtime_state_with_state_home;
use agent_semantic_runtime::project_state_paths_with_state_home;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

struct TemporaryStateHome(PathBuf);

impl TemporaryStateHome {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must follow the Unix epoch")
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "asp-activity-write-gate-{}-{unique}",
            std::process::id()
        )))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryStateHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn materializing_state_does_not_write_legacy_activity_markers() {
    let state_home = TemporaryStateHome::new();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let paths = project_state_paths_with_state_home(project_root, state_home.path())
        .expect("project state paths must resolve");

    project_runtime_state_with_state_home(project_root, state_home.path())
        .expect("project runtime state must materialize");

    let project_dir = state_home
        .path()
        .join("projects/by-id")
        .join(&paths.repo_id.0);
    let workspace_dir = project_dir.join("workspaces").join(&paths.workspace_id.0);
    assert!(
        !project_dir.join(".last-seen-ms").exists(),
        "project identity materialization must not write activity telemetry"
    );
    assert!(
        !workspace_dir.join(".last-seen-ms").exists(),
        "workspace materialization must not write activity telemetry"
    );
}
