// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::materialize_cli_failure;

#[test]
fn runtime_transport_unavailable_is_a_typed_pre_frame_terminal() {
    let rendered = materialize_cli_failure(
        "reasonKind=transport-unavailable ASP Server endpoint is unavailable",
    );
    let receipt: serde_json::Value = serde_json::from_str(&rendered).expect("typed receipt");
    assert_eq!(receipt["reasonKind"], "transport-unavailable");
    assert_eq!(receipt["failureLayer"], "runtime-transport-capability");
    assert_eq!(receipt["state"], "blocked");
}

#[test]
fn unauthenticated_runtime_status_observation_is_typed_without_lifecycle_claim() {
    let rendered = materialize_cli_failure(
        "state=blocked reasonKind=runtime-status-observation-failed operation=status originalError=early eof",
    );
    let receipt: serde_json::Value = serde_json::from_str(&rendered).expect("typed receipt");

    assert_eq!(receipt["reasonKind"], "runtime-status-observation-failed");
    assert_eq!(receipt["failureLayer"], "runtime-status-observation");
    assert_eq!(receipt["state"], "blocked");
    assert!(receipt.get("lifecycleState").is_none());
}

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
fn verified_runtime_endpoint_eperm_preserves_the_transport_boundary() {
    let rendered = materialize_cli_failure(
        "reasonKind=host-operation-not-permitted failureLayer=runtime-verified-endpoint-transport osError=EPERM originalError=Operation not permitted (os error 1)",
    );
    let receipt: serde_json::Value = serde_json::from_str(&rendered).expect("typed CLI failure");

    assert_eq!(receipt["reasonKind"], "host-operation-not-permitted");
    assert_eq!(
        receipt["failureLayer"],
        "runtime-verified-endpoint-transport"
    );
    assert_eq!(receipt["osError"], "EPERM");
    assert!(
        receipt["message"]
            .as_str()
            .is_some_and(|message| message.contains("proved the Runtime serving identity"))
    );
}

#[test]
fn already_typed_or_domain_errors_are_not_rewritten() {
    let message = "agent-choice-required: select the configured Agent";
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
