// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use asp_rust_project_harness_policy::asp_workspace_member_forbidden_normal_dependencies;
use asp_rust_project_harness_policy::asp_workspace_member_policies;

#[test]
fn retired_codex_claude_hook_and_legacy_standalone_crates_are_absent_from_workspace() {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for retired in [
        "crates/agent-semantic-hook",
        "crates/agent-semantic-hook-testkit",
        "crates/agent-semantic-shell-parser",
        "crates/agent-semantic-client",
        "crates/agent-semantic-client-core",
        "crates/agent-semantic-client-db",
        "crates/agent-semantic-client-server",
        "crates/agent-semantic-context-product",
        "crates/agent-semantic-loop",
        "crates/agent-semantic-runtime",
        "crates/agent-semantic-runtime-observability",
        "crates/agent-semantic-runtime-process-observation",
        "crates/agent-semantic-runtime-server",
        "crates/agent-semantic-scheme-syntax",
        "crates/agent-semantic-tree-sitter",
        "crates/agent-semantic-mrr",
        ".github/actions/mrr-search-owner",
        ".github/actions/orgize-ffi",
        ".github/scripts/fetch-orgize-ffi.sh",
        "asp-codex-plugin",
        "agents",
    ] {
        assert!(
            !repository_root.join(retired).exists(),
            "retired legacy surface `{retired}` must not be recreated"
        );
    }
    assert_eq!(
        asp_workspace_member_forbidden_normal_dependencies("agent-semantic-artifacts"),
        ["turso"],
    );
}

#[test]
fn central_policy_registry_contains_migrated_member_crates() {
    let package_names: Vec<_> = asp_workspace_member_policies()
        .iter()
        .map(|policy| policy.package_name)
        .collect();

    assert_eq!(
        package_names,
        vec![
            "agent-semantic-artifacts",
            "agent-semantic-http-json",
            "agent-semantic-provider-protocol",
            "agent-semantic-provider-transport",
            "agent-semantic-search",
            "agent-semantic-search-projection",
            "agent-semantic-topology",
            "agent-semantic-schema-manager",
        ]
    );
}

#[test]
fn every_member_policy_has_a_content_addressed_declaration() {
    for policy in asp_workspace_member_policies() {
        let digest = policy.contract_digest();
        assert!(digest.starts_with("blake3-256:"));
        assert_eq!(digest, policy.contract_digest());
    }
}

#[test]
fn every_member_build_uses_the_shared_source_policy_crate() {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for policy in asp_workspace_member_policies() {
        let manifest =
            std::fs::read_to_string(repository_root.join(policy.crate_root).join("Cargo.toml"))
                .unwrap_or_else(|error| panic!("{} manifest: {error}", policy.package_name));
        assert!(
            manifest.contains("[dev-dependencies]") && manifest.contains("asp-rust"),
            "{} must declare asp-rust strictly under [dev-dependencies]",
            policy.package_name
        );
        let policy_test = repository_root
            .join(policy.crate_root)
            .join("tests/unit/policy.rs");
        let source = std::fs::read_to_string(&policy_test)
            .unwrap_or_else(|error| panic!("read {}: {error}", policy_test.display()));
        assert!(
            source.contains("asp_rust_cargo_test_gate!"),
            "{} must mount asp_rust_cargo_test_gate! in tests/unit/policy.rs",
            policy.package_name
        );
    }
}

#[test]
#[cfg(feature = "workspace-policy")]
fn every_workspace_package_is_one_cargo_owned_policy_atom() {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dag = asp_rust::asp_rust_workspace_build_dag(
        &repository_root,
        &asp_rust::AspRustConfig::default(),
    )
    .expect("derive the real workspace dependency DAG");

    for package in dag
        .packages
        .iter()
        .filter(|package| package.package_name != "asp-rust-project-harness-policy")
    {
        let manifest_path = package.package_root.join("Cargo.toml");
        let manifest = std::fs::read_to_string(&manifest_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", manifest_path.display()));
        assert!(
            manifest.contains("[dev-dependencies]") && manifest.contains("asp-rust"),
            "{} must declare asp-rust strictly under [dev-dependencies]",
            package.package_name
        );

        let policy_test = package.package_root.join("tests/unit/policy.rs");
        let source = std::fs::read_to_string(&policy_test)
            .unwrap_or_else(|error| panic!("read {}: {error}", policy_test.display()));
        assert!(
            source.contains("asp_rust_cargo_test_gate!"),
            "{} must mount asp_rust_cargo_test_gate! in tests/unit/policy.rs",
            package.package_name
        );
    }
}

#[test]
fn shared_build_policy_compiles_one_default_full_scanner() {
    let policy_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !policy_root.join("build.rs").exists(),
        "the shared member gate must not have its own build script"
    );
    let manifest = std::fs::read_to_string(policy_root.join("Cargo.toml"))
        .expect("read shared build-policy manifest");
    assert!(
        !manifest.contains("[build-dependencies]"),
        "the shared policy library must not compile a second build-script scanner"
    );
    assert!(manifest.contains("default = [\"workspace-policy\"]"));
    assert!(manifest.contains("workspace-policy = [\"dep:asp-rust\"]"));
    assert!(manifest.contains("asp-rust = { workspace = true, optional = true }"));
}

#[test]
fn central_policy_preserves_member_specific_verification_owners() {
    let policies = asp_workspace_member_policies();
    let provider_transport = policies
        .iter()
        .find(|policy| policy.package_name == "agent-semantic-provider-transport")
        .expect("provider-transport policy");
    let search = policies
        .iter()
        .find(|policy| policy.package_name == "agent-semantic-search")
        .expect("search policy");
    let topology = policies
        .iter()
        .find(|policy| policy.package_name == "agent-semantic-topology")
        .expect("topology policy");

    assert_eq!(
        provider_transport.verification_label,
        Some("provider transport")
    );
    assert!(
        provider_transport
            .availability_stability_owners
            .iter()
            .any(|owner| owner.path == "src/transport.rs")
    );
    assert!(
        search
            .latency_sensitive_performance_owners
            .iter()
            .any(|owner| owner.path == "src/converged_resident_index/query.rs")
    );
    assert!(
        topology
            .latency_sensitive_performance_owners
            .iter()
            .any(|owner| owner.path == "src/topology_index.rs")
    );
}

#[test]
fn member_policy_digest_is_stable_and_distinguishes_declarations() {
    let policies = asp_workspace_member_policies();
    let topology = policies
        .iter()
        .find(|policy| policy.package_name == "agent-semantic-topology")
        .copied()
        .expect("topology policy");
    let search = policies
        .iter()
        .find(|policy| policy.package_name == "agent-semantic-search")
        .copied()
        .expect("search policy");

    let first = topology.contract_digest();
    assert_eq!(first, topology.contract_digest());
    assert!(first.starts_with("blake3-256:"));
    assert_ne!(first, search.contract_digest());
}
