// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

#[test]
fn runtime_owner_probe_does_not_require_a_full_workspace_generation() {
    let operation = crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeOwner {
        project_root: "/workspace/cold-owner".to_owned(),
        owner_path: "src/lib.rs".to_owned(),
    };

    assert!(
        super::resident_read_project_root(&operation).is_none(),
        "an owner snapshot probe must return GenerationMissing so the typed provider-owner cold path can run"
    );
}
