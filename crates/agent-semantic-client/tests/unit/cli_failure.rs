use super::materialize_cli_failure;

#[test]
fn bare_eperm_becomes_an_explicit_agent_facing_boundary_receipt() {
    let rendered = materialize_cli_failure("Operation not permitted (os error 1)");
    let receipt: serde_json::Value = serde_json::from_str(&rendered).expect("typed CLI failure");

    assert_eq!(receipt["schemaVersion"], 1);
    assert_eq!(receipt["reasonKind"], "host-operation-not-permitted");
    assert_eq!(receipt["failureLayer"], "asp-cli-startup-or-ipc-boundary");
    assert_eq!(receipt["osError"], "EPERM");
    assert!(
        receipt["message"]
            .as_str()
            .is_some_and(|message| message.contains("not a Hook policy denial"))
    );
}

#[test]
fn already_typed_or_domain_errors_are_not_rewritten() {
    let message = "subagent-receipt-required: registration is missing";
    assert_eq!(materialize_cli_failure(message), message);
}

#[test]
fn runtime_construction_eperm_is_materialized_at_the_same_agent_boundary() {
    let rendered = materialize_cli_failure(
        "failed to create ASP CLI runtime: Operation not permitted (os error 1)",
    );
    let receipt: serde_json::Value = serde_json::from_str(&rendered).expect("typed CLI failure");

    assert_eq!(receipt["reasonKind"], "host-operation-not-permitted");
    assert_eq!(receipt["failureLayer"], "asp-cli-startup-or-ipc-boundary");
    assert!(
        receipt["message"]
            .as_str()
            .is_some_and(|message| message.contains("not a Hook policy denial"))
    );
}
