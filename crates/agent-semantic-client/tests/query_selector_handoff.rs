// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::process::Command;

#[test]
fn exact_query_requires_content_bound_handoff_before_any_endpoint_lookup() {
    let state_home = tempfile::tempdir().expect("isolated State Home");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "query",
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
        terminal.contains("reasonKind=runtime-client-handoff-unavailable"),
        "Query must fail at the content-bound handoff boundary: {terminal}"
    );
    assert!(
        !terminal.contains("endpoint.v1.json")
            && !terminal.contains("failed to inspect Runtime Server endpoint"),
        "Query must not rederive retired endpoint authority: {terminal}"
    );
}
