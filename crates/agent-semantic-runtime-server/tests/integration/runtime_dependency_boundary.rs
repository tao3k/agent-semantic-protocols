// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::process::Command;

fn normal_packages(package: &str) -> Vec<String> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.join("../..");
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args([
            "tree", "-p", package, "--edges", "normal", "--prefix", "none", "--format", "{p}",
        ])
        .output()
        .expect("inspect package dependency graph");
    assert!(
        output.status.success(),
        "cargo tree failed for {package}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("UTF-8 cargo tree")
        .lines()
        .filter_map(|line| line.split_ascii_whitespace().next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn runtime_server_uses_provider_protocol_without_hook_in_its_normal_dependency_graph() {
    let packages = normal_packages("agent-semantic-runtime-server");
    assert!(
        packages
            .iter()
            .any(|name| name == "agent-semantic-provider-protocol"),
        "{packages:?}"
    );
    assert!(
        !packages.iter().any(|name| name == "agent-semantic-hook"),
        "{packages:?}"
    );
}

#[test]
fn telemetry_exporter_dependencies_stop_at_the_runtime_server_boundary() {
    let client_db = normal_packages("agent-semantic-client-db");
    assert!(
        client_db
            .iter()
            .any(|name| name == "agent-semantic-runtime-observability"),
        "{client_db:?}"
    );
    assert!(
        !client_db.iter().any(|name| name == "opentelemetry"),
        "client-db must not acquire exporter authority: {client_db:?}"
    );
    assert!(
        !client_db.iter().any(|name| name == "opentelemetry_sdk"),
        "client-db must not acquire exporter SDK authority: {client_db:?}"
    );

    let server = normal_packages("agent-semantic-runtime-server");
    for required in [
        "agent-semantic-runtime-observability",
        "agent-semantic-runtime-process-observation",
        "opentelemetry",
        "opentelemetry_sdk",
    ] {
        assert!(
            server.iter().any(|name| name == required),
            "Runtime Server is missing {required}: {server:?}"
        );
    }
}
