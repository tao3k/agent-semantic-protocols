// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::selector_is_workspace_owned;

fn runtime(project_root: &std::path::Path) -> crate::HookRuntime {
    crate::HookRuntime {
        project_root: project_root.display().to_string(),
        policy_providers: Vec::new(),
    }
}

#[test]
fn selector_workspace_ownership_rejects_absolute_and_parent_escape() {
    let workspace = std::env::current_dir()
        .expect("current directory")
        .join("workspace-owned-selector-test");
    let runtime = runtime(&workspace);

    assert!(selector_is_workspace_owned(&runtime, "src/lib.rs"));
    assert!(selector_is_workspace_owned(
        &runtime,
        &workspace.join("README.md").display().to_string()
    ));
    assert!(!selector_is_workspace_owned(&runtime, "../MEMORY.md"));
    assert!(!selector_is_workspace_owned(
        &runtime,
        &std::env::temp_dir()
            .join("external.md")
            .display()
            .to_string()
    ));
}
