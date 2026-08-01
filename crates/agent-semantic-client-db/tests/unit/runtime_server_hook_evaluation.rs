use agent_semantic_client_db::workspace_db_ipc::{WorkspaceDbIpcOperation, WorkspaceDbIpcResult};

#[test]
fn hook_evaluation_operation_has_a_typed_round_trip() {
    let operation = WorkspaceDbIpcOperation::EvaluateHook {
        project_root: "/workspace/alpha".to_owned(),
        arguments: vec![
            "hook".to_owned(),
            "stop".to_owned(),
            "--client".to_owned(),
            "codex".to_owned(),
        ],
        input: r#"{"cwd":"/workspace/alpha"}"#.to_owned(),
    };

    let encoded = serde_json::to_value(&operation).expect("encode hook evaluation operation");
    assert_eq!(encoded["kind"], "evaluate-hook");
    assert_eq!(encoded["projectRoot"], "/workspace/alpha");
    let decoded: WorkspaceDbIpcOperation =
        serde_json::from_value(encoded).expect("decode hook evaluation operation");
    assert_eq!(decoded, operation);
}

#[test]
fn hook_evaluation_result_preserves_the_final_platform_response() {
    let result = WorkspaceDbIpcResult::HookEvaluation {
        workspace_identity: "workspace-alpha".to_owned(),
        project_root: "/workspace/alpha".to_owned(),
        output: r#"{"hookSpecificOutput":{"hookEventName":"Stop"}}"#.to_owned(),
    };
    let encoded = serde_json::to_string(&result).expect("encode hook evaluation result");
    let decoded: WorkspaceDbIpcResult =
        serde_json::from_str(&encoded).expect("decode hook evaluation result");
    assert_eq!(decoded, result);
    let encoded: serde_json::Value =
        serde_json::from_str(&encoded).expect("decode hook evaluation result fields");
    assert_eq!(encoded["workspaceIdentity"], "workspace-alpha");
    assert_eq!(encoded["projectRoot"], "/workspace/alpha");
}
