// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::process::Command;

#[test]
fn runtime_server_uses_provider_protocol_without_hook_in_its_normal_dependency_graph() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.join("../..");
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args([
            "tree",
            "-p",
            "agent-semantic-runtime-server",
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .output()
        .expect("inspect Runtime Server dependency graph");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).expect("UTF-8 cargo tree");
    let packages = tree
        .lines()
        .filter_map(|line| line.split_ascii_whitespace().next())
        .collect::<Vec<_>>();
    assert!(
        packages.contains(&"agent-semantic-provider-protocol"),
        "{tree}"
    );
    assert!(!packages.contains(&"agent-semantic-hook"), "{tree}");
}
