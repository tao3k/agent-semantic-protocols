// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::process::Command;

#[test]
fn evaluator_feature_has_no_direct_runtime_or_protocol_normal_dependencies() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.join("../..");
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args([
            "tree",
            "-p",
            "agent-semantic-hook",
            "--no-default-features",
            "--features",
            "evaluator",
            "-e",
            "normal",
            "--depth",
            "1",
        ])
        .output()
        .expect("inspect evaluator dependency graph");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).expect("UTF-8 cargo tree");
    let reachable = reachable_package_names(&tree);
    for forbidden in [
        "agent-semantic-client-db",
        "agent-semantic-runtime",
        "agent-semantic-search",
        "agent-semantic-provider-protocol",
        "agent-semantic-protocol",
        "tokio",
    ] {
        assert!(
            !reachable.contains(&forbidden),
            "evaluator direct dependency graph contains forbidden package {forbidden}:\n{tree}"
        );
    }
}

fn reachable_package_names(tree: &str) -> Vec<&str> {
    tree.lines()
        .filter_map(|line| {
            line.trim_start_matches([' ', '│', '├', '└', '─'])
                .split_ascii_whitespace()
                .next()
        })
        .collect()
}

#[test]
fn dependency_oracle_matches_package_nodes_not_workspace_path_text() {
    let unreachable_path = "agent-semantic-hook v0.1.0 (/workspace/agent-semantic-protocols/crates/agent-semantic-hook)\n└── serde v1.0.0";
    assert!(!reachable_package_names(unreachable_path).contains(&"agent-semantic-protocol"));

    let reachable = "agent-semantic-hook v0.1.0 (/workspace)\n└── agent-semantic-protocol v0.1.0 (/workspace/protocol)";
    assert!(reachable_package_names(reachable).contains(&"agent-semantic-protocol"));
}
