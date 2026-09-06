// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::process::Command;

#[test]
fn query_playbook_is_a_real_facade_before_runtime_handoff() {
    let state_home = tempfile::tempdir().expect("temporary ASP State Home");
    let workspace = std::env::current_dir().expect("current workspace");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "query",
            "playbook",
            "--selector",
            "org://docs/10-19-rfcs/10.06-agent-search-projection/10.06.14-search-evidence-output-reflection.org#item/heading/SDD-Search-Evidence-Output",
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
        terminal.contains("reasonKind=runtime-client-handoff-unavailable"),
        "Query facade must fail at the typed Runtime handoff boundary: {terminal}"
    );
}
