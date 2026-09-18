// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::HookExecutionFailure;
use super::HookExecutionFailureKind;
use super::HookExecutionPhase;

#[test]
fn failure_receipt_is_single_valid_json_document() {
    let receipt = HookExecutionFailure::new(
        HookExecutionPhase::Bootstrap,
        HookExecutionFailureKind::RuntimePanic,
        Some("post-tool".to_owned()),
        Some("codex".to_owned()),
        "panic text",
    )
    .to_string();
    let value: serde_json::Value = serde_json::from_str(&receipt).expect("valid JSON receipt");
    assert_eq!(value["state"], "failed");
    assert_eq!(value["failureKind"], "runtime-panic");
    assert_eq!(value["event"], "post-tool");
    assert!(value["runtimeArtifactFingerprint"].is_string());

    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/semantic-agent-hook-execution-failure.schema.json"
    ))
    .expect("execution failure schema JSON");
    jsonschema::validator_for(&schema)
        .expect("execution failure schema")
        .validate(&value)
        .expect("receipt satisfies execution failure schema");
}

#[test]
fn launcher_exit_code_is_typed_by_the_execution_failure_schema() {
    let mut failure = HookExecutionFailure::new(
        HookExecutionPhase::Bootstrap,
        HookExecutionFailureKind::LauncherTargetUnavailable,
        Some("pre-tool".to_owned()),
        Some("codex".to_owned()),
        "launcher target unavailable",
    );
    failure.exit_code = Some(127);
    let value = serde_json::to_value(failure).expect("serialize launcher failure");
    assert_eq!(value["failureKind"], "launcher-target-unavailable");
    assert_eq!(value["exitCode"], 127);

    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/semantic-agent-hook-execution-failure.schema.json"
    ))
    .expect("execution failure schema JSON");
    jsonschema::validator_for(&schema)
        .expect("execution failure schema")
        .validate(&value)
        .expect("launcher receipt satisfies execution failure schema");
}
