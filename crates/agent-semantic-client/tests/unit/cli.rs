// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::render_cli_error;

#[test]
fn ipc_permission_denial_preserves_argv_and_transfers_authority() {
    let rendered = render_cli_error(
        &[
            "rust".to_owned(),
            "search".to_owned(),
            "owner with spaces".to_owned(),
        ],
        "failed to connect Runtime Server data endpoint reasonKind=host-local-ipc-permission-denied errorKind=permission-denied".to_owned(),
    );
    let receipt: serde_json::Value = serde_json::from_str(&rendered).expect("typed receipt");
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/host-native-execution-required.v1.schema.json"
    ))
    .expect("host-native execution schema");
    jsonschema::validator_for(&schema)
        .expect("compile host-native execution schema")
        .validate(&receipt)
        .expect("receipt satisfies host-native execution schema");
    assert_eq!(
        receipt["schemaId"],
        "agent.semantic-protocols.host-native-execution-required"
    );
    assert_eq!(receipt["schemaVersion"], "1");
    assert_eq!(receipt["state"], "deferred");
    assert_eq!(receipt["executionAuthority"], "host-native");
    assert_eq!(receipt["retryPolicy"], "do-not-retry-in-current-sandbox");
    assert_eq!(
        receipt["argv"],
        serde_json::json!(["asp", "rust", "search", "owner with spaces"])
    );
}

#[test]
fn unrelated_cli_errors_are_not_rewritten() {
    let error = "invalid selector".to_owned();
    assert_eq!(render_cli_error(&["rust".to_owned()], error.clone()), error);
}

#[test]
fn runtime_connect_deadline_is_exactly_one_typed_non_retryable_terminal() {
    let rendered = render_cli_error(
        &["rust".to_owned(), "search".to_owned()],
        "reasonKind=runtime-client-connect-deadline-exceeded budgetMs=2000 retryAdmitted=false"
            .to_owned(),
    );
    let receipt: serde_json::Value = serde_json::from_str(&rendered).expect("typed receipt");
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/runtime-client-terminal.schema.json"
    ))
    .expect("runtime client terminal schema");
    jsonschema::validator_for(&schema)
        .expect("compile runtime client terminal schema")
        .validate(&receipt)
        .expect("receipt satisfies runtime client terminal schema");
    assert_eq!(receipt["state"], "failed");
    assert_eq!(
        receipt["reasonKind"],
        "runtime-client-connect-deadline-exceeded"
    );
    assert_eq!(receipt["retryAdmitted"], false);
}

#[test]
fn missing_generation_has_one_runtime_owned_non_recursive_action() {
    let rendered = render_cli_error(
        &["rust".to_owned(), "search".to_owned(), "owner.rs".to_owned()],
        "state=source-unavailable reasonKind=active-workspace-generation-required: active workspace generation lease is required".to_owned(),
    );
    let receipt: serde_json::Value = serde_json::from_str(&rendered).expect("typed receipt");
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/workspace-generation-required.schema.json"
    ))
    .expect("workspace generation schema");
    jsonschema::validator_for(&schema)
        .expect("compile workspace generation schema")
        .validate(&receipt)
        .expect("receipt satisfies workspace generation schema");
    assert_eq!(receipt["state"], "in-progress");
    assert_eq!(receipt["admissionTrigger"], "query-demand");
    assert_eq!(receipt["buildOwner"], "runtime-server");
    assert_eq!(receipt["requestLifetimeIndependent"], true);
    assert_eq!(receipt["retryPolicy"], "retry-after-runtime-progress");
    assert!(receipt.get("choicePlaneCommand").is_none());
    assert_eq!(
        receipt["argv"],
        serde_json::json!(["asp", "rust", "search", "owner.rs"])
    );
    assert!(
        !receipt["nextAction"]
            .as_str()
            .expect("next action")
            .contains("@asp_")
    );
    assert!(
        !receipt["nextAction"]
            .as_str()
            .unwrap()
            .contains("cache import")
    );
    assert!(
        !receipt["nextAction"]
            .as_str()
            .unwrap()
            .contains("source-index refresh")
    );
    assert!(
        !receipt["nextAction"]
            .as_str()
            .unwrap()
            .contains("choice-plane")
    );
}
