// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::process::Command;

#[test]
fn exact_query_requires_a_registered_workspace_before_runtime_bootstrap() {
    let state_home = tempfile::tempdir().expect("isolated State Home");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "query",
            "playbook",
            "--language",
            "rust",
            "--selector",
            "rust://crates/agent-semantic-client/src/language_command.rs#item/struct/RuntimeLanguageCommandClient",
            "--workspace",
        ])
        .arg(workspace)
        .env("ASP_STATE_HOME", state_home.path())
        .env_remove("ASP_RUNTIME_CLIENT_FD")
        .output()
        .expect("launch current-tree exact Query");

    let terminal = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "isolated State Home has no handoff"
    );
    assert!(
        terminal.contains("Runtime Server workspace admission catalog has no binding"),
        "Query must fail at the registered-workspace admission boundary: {terminal}"
    );
    assert!(
        !terminal.contains("endpoint.v1.json")
            && !terminal.contains("failed to inspect Runtime Server endpoint"),
        "Query must not rederive retired endpoint authority: {terminal}"
    );
}
