use asp_rust_project_harness_policy::{
    asp_workspace_member_policies, validate_asp_rust_project_harness_member_manifest,
};

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
            "agent-semantic-provider-transport",
            "agent-semantic-search",
            "agent-semantic-search-projection",
            "agent-semantic-schema-manager",
            "agent-semantic-tree-sitter",
            "agent-semantic-runtime",
            "orgize",
        ]
    );
}

#[test]
fn every_member_uses_the_constant_time_build_contract() {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for policy in asp_workspace_member_policies() {
        let receipt = validate_asp_rust_project_harness_member_manifest(
            policy.package_name,
            &repository_root.join(policy.crate_root),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", policy.package_name));
        assert_eq!(receipt.package_name, policy.package_name);
        assert_eq!(receipt.policy_digest, policy.contract_digest());
    }
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
