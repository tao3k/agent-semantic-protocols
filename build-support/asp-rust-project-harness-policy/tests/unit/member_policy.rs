use asp_rust_project_harness_policy::asp_workspace_member_policies;

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
            "agent-semantic-client-local-cli",
            "agent-semantic-client",
            "agent-semantic-hook",
        ]
    );
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
    assert_eq!(client_db.latency_sensitive_performance_owners.len(), 2);
    assert_eq!(client.availability_stability_owners.len(), 2);
}
