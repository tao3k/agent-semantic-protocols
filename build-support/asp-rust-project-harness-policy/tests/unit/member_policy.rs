// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use asp_rust_project_harness_policy::asp_workspace_member_forbidden_normal_dependencies;
use asp_rust_project_harness_policy::asp_workspace_member_policies;

#[test]
fn runtime_provider_consumers_forbid_direct_hook_dependencies() {
    for package in [
        "agent-semantic-client-core",
        "agent-semantic-runtime-server",
    ] {
        assert_eq!(
            asp_workspace_member_forbidden_normal_dependencies(package),
            ["agent-semantic-hook"],
            "{package} must consume provider DTOs from Provider Protocol"
        );
    }
}

#[test]
fn hook_source_rejects_retired_manifest_activation_routing() {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let hook_source = repository_root.join("crates/agent-semantic-hook/src");
    assert!(
        !hook_source.join("protocol_activation").exists(),
        "Hook source must not recreate the retired protocol_activation module"
    );

    let forbidden = [
        "pub struct HookRoutes",
        "pub struct HookRouteBindings",
        "pub struct HookActivation",
        "pub struct ActivatedProvider",
        "pub struct ProviderManifest",
        "fn materialize_provider_routes",
    ];
    let mut pending = vec![hook_source];
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        {
            let path = entry.expect("Hook source entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            for retired in forbidden {
                assert!(
                    !source.contains(retired),
                    "{} recreates retired Hook routing surface `{retired}`",
                    path.display()
                );
            }
        }
    }
}

#[test]
#[cfg(feature = "workspace-policy")]
fn runtime_server_member_source_policy_accepts_split_route_owners() {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let crate_root = repository_root.join("crates/agent-semantic-runtime-server");

    let report =
        asp_rust_project_harness_policy::evaluate_asp_rust_project_harness_member_source_policy(
            "agent-semantic-runtime-server",
            &crate_root,
        )
        .expect("evaluate Runtime Server member source policy");

    assert!(!report.findings.iter().any(|finding| {
        finding.rule_id == "RUST-MOD-R002"
            && finding.location.path.as_ref().is_some_and(|path| {
                path.ends_with("src/runtime_asp_client_resolved_route.rs")
                    || path.ends_with("src/runtime_asp_client_query_playbook.rs")
            })
    }));
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
            "agent-semantic-client-core",
            "agent-semantic-client-db",
            "agent-semantic-client-server",
            "agent-semantic-client",
            "agent-semantic-hook",
            "agent-semantic-hook-testkit",
            "agent-semantic-http-json",
            "agent-semantic-provider-protocol",
            "agent-semantic-provider-transport",
            "agent-semantic-search",
            "agent-semantic-search-projection",
            "agent-semantic-schema-manager",
            "agent-semantic-tree-sitter",
            "agent-semantic-runtime",
            "agent-semantic-runtime-server",
            "orgize",
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
    for policy in asp_workspace_member_policies()
        .iter()
        .filter(|policy| policy.package_name != "orgize")
    {
        let manifest =
            std::fs::read_to_string(repository_root.join(policy.crate_root).join("Cargo.toml"))
                .unwrap_or_else(|error| panic!("{} manifest: {error}", policy.package_name));
        assert!(
            manifest.contains("[build-dependencies]")
                && manifest.contains("asp-rust-project-harness-policy"),
            "{} must share the Cargo Build Support unit",
            policy.package_name
        );
        let conventional_build = repository_root.join(policy.crate_root).join("build.rs");
        let source = std::fs::read_to_string(&conventional_build)
            .unwrap_or_else(|error| panic!("read {}: {error}", conventional_build.display()));
        assert!(
            source.contains("assert_asp_rust_project_harness_member_policy_from_env"),
            "{} must execute one package-local policy atom",
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
            manifest.contains("[build-dependencies]")
                && manifest.contains("asp-rust-project-harness-policy"),
            "{} must participate in the shared Cargo policy DAG",
            package.package_name
        );

        let build_path = package.package_root.join("build.rs");
        let build = std::fs::read_to_string(&build_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", build_path.display()));
        assert!(
            build.contains("assert_asp_rust_project_harness_member_policy_from_env"),
            "{} must run exactly one package-local policy atom",
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
    let client_db = policies
        .iter()
        .find(|policy| policy.package_name == "agent-semantic-client-db")
        .expect("client-db policy");
    let client = policies
        .iter()
        .find(|policy| policy.package_name == "agent-semantic-client")
        .expect("client policy");
    let runtime_server = policies
        .iter()
        .find(|policy| policy.package_name == "agent-semantic-runtime-server")
        .expect("Runtime Server policy");

    assert_eq!(client_db.verification_label, Some("client db"));
    assert!(
        client_db
            .latency_sensitive_performance_owners
            .iter()
            .any(|owner| owner.path == "src/engine/facade.rs")
    );
    assert!(
        client
            .availability_stability_owners
            .iter()
            .any(|owner| owner.path == "src/cli.rs")
    );
    assert!(
        runtime_server
            .latency_sensitive_performance_owners
            .iter()
            .any(|owner| owner.path == "src/query_generation.rs")
    );
    assert!(
        runtime_server
            .availability_stability_owners
            .iter()
            .any(|owner| owner.path == "src/query_generation.rs")
    );
}

#[test]
fn member_policy_digest_is_stable_and_distinguishes_declarations() {
    let policies = asp_workspace_member_policies();
    let client_db = policies
        .iter()
        .find(|policy| policy.package_name == "agent-semantic-client-db")
        .copied()
        .expect("client-db policy");
    let search = policies
        .iter()
        .find(|policy| policy.package_name == "agent-semantic-search")
        .copied()
        .expect("search policy");

    let first = client_db.contract_digest();
    assert_eq!(first, client_db.contract_digest());
    assert!(first.starts_with("blake3-256:"));
    assert_ne!(first, search.contract_digest());
}
