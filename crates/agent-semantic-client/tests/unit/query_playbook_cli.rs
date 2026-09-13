// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::process::Command;

#[test]
fn query_playbook_is_a_real_facade_before_workspace_admission() {
    let state_home = tempfile::tempdir().expect("temporary ASP State Home");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "query",
            "playbook",
            "--language",
            "org|rust",
            "--selector",
            "org://docs/10-19-rfcs/10.06-agent-search-projection/10.06.14-search-evidence-output-reflection.org#item/heading/SDD-Search-Evidence-Output",
            "--selector",
            "rust://crates/agent-semantic-client/src/language_command.rs#item/struct/RuntimeLanguageCommandClient",
            "--workspace",
        ])
        .arg(workspace)
        .env("ASP_STATE_HOME", state_home.path())
        .output()
        .expect("run current-tree asp query playbook");

    assert!(
        !output.status.success(),
        "empty State Home has no Runtime handoff"
    );
    let terminal = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !terminal.contains("query does not support option `playbook`"),
        "playbook must be parsed as the Query facade before Runtime admission: {terminal}"
    );
    assert!(
        terminal.contains("Runtime Server workspace admission catalog has no binding"),
        "Query facade must fail at the registered-workspace admission boundary: {terminal}"
    );
}
