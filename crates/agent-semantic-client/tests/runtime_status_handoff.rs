// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::process::Command;

#[test]
fn runtime_status_requires_host_handoff_and_never_rederives_a_legacy_endpoint() {
    let state_home = tempfile::tempdir().expect("create isolated ASP State Home");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["server", "status"])
        .env("ASP_STATE_HOME", state_home.path())
        .env_remove("ASP_RUNTIME_CLIENT_FD")
        .output()
        .expect("launch current-tree asp status");

    assert!(!output.status.success());
    let terminal = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        terminal.contains("runtime-client-handoff-unavailable"),
        "status must fail at the Host handoff boundary: {terminal}"
    );
    assert!(
        !terminal.contains("endpoint.v1.json"),
        "status must not rederive the retired endpoint authority: {terminal}"
    );
}
