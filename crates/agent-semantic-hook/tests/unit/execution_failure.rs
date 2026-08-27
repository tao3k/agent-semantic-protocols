use super::{HookExecutionFailure, HookExecutionFailureKind, HookExecutionPhase};

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
