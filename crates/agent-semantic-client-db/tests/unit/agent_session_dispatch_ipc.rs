use agent_semantic_client_db::workspace_db_ipc::{
    AgentSessionRegistryIpcOperation, AgentSessionRegistryIpcResult,
};

#[test]
fn complete_dispatch_has_one_typed_v1_wire_shape() {
    let operation = AgentSessionRegistryIpcOperation::CompleteDispatch {
        project_id: "workspace-1".to_owned(),
        root_session_id: "root-1".to_owned(),
        name: "asp-testing".to_owned(),
        dispatch_identity: "dispatch-v1:identity".to_owned(),
        command_digest: "command-digest".to_owned(),
        evidence_ref: "receipt:test".to_owned(),
        now: 17,
    };
    let encoded = serde_json::to_value(&operation).expect("encode complete dispatch operation");
    assert_eq!(encoded["kind"], "complete-dispatch");
    assert_eq!(encoded["projectId"], "workspace-1");
    assert_eq!(encoded["evidenceRef"], "receipt:test");
    assert_eq!(
        serde_json::from_value::<AgentSessionRegistryIpcOperation>(encoded)
            .expect("decode complete dispatch operation"),
        operation
    );
}

#[test]
fn complete_dispatch_proxy_and_owner_share_the_typed_operation() {
    let api = include_str!("../../src/agent_session_registry/core/api.rs");
    let owner = include_str!("../../src/workspace_db_ipc_server_agent_session_registry.rs");

    assert!(api.contains("AgentSessionRegistryIpcOperation::CompleteDispatch"));
    assert!(api.contains("AgentSessionRegistryIpcResult::DispatchCompleted"));
    assert!(owner.contains("Operation::CompleteDispatch"));
    assert!(owner.contains("IpcResult::DispatchCompleted"));
    assert_eq!(
        api.matches("turso_complete_dispatch").count(),
        1,
        "only the RuntimeServer-owned fallback may open the registry database"
    );
}

#[test]
fn dispatch_completion_result_is_terminal_lease_data() {
    let result_source = include_str!("../../src/workspace_db_ipc/agent_session_registry.rs");
    assert!(result_source.contains("DispatchCompleted"));
    assert!(result_source.contains("AgentSessionDispatchLeaseRecord"));
    assert!(!result_source.contains("DispatchCompletionV2"));

    let _result_type = std::mem::size_of::<AgentSessionRegistryIpcResult>();
}
