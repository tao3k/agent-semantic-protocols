use super::AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID;
use super::AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID;
use super::AgentChildThreadId;
use super::AgentName;
use super::AgentParentThreadId;
use super::AgentPath;
use super::AgentRootSessionId;
use super::AgentRouteKey;
use super::AgentSessionPlatform;
use super::AgentSessionRegisterReceipt;
use super::AgentSessionRegisterRequest;
use super::AgentSessionRegisterState;
use super::AgentSessionRegistryOwner;
use super::AgentSessionTransport;
use super::ClientProjectId;
use super::ClientSchemaId;

#[test]
fn registration_request_binds_distinct_parent_child_and_canonical_path() {
    let request = AgentSessionRegisterRequest {
        schema_id: ClientSchemaId::new(AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID)
            .expect("request schema id"),
        schema_version: 1,
        root_session_id: AgentRootSessionId::new("root-1").expect("root session id"),
        parent_thread_id: AgentParentThreadId::new("parent-1").expect("parent thread id"),
        child_thread_id: AgentChildThreadId::new("child-1").expect("child thread id"),
        agent_name: AgentName::new("asp_testing").expect("agent name"),
        agent_path: AgentPath::new("/root/asp_testing").expect("agent path"),
        route_key: AgentRouteKey::new("asp_testing").expect("route key"),
    };
    request.validate().expect("valid registration request");

    let mut invalid = request.clone();
    invalid.child_thread_id =
        AgentChildThreadId::new(invalid.parent_thread_id.as_str()).expect("child thread id");
    assert!(invalid.validate().is_err());
    let mut invalid = request;
    invalid.agent_path = AgentPath::new("/root/asp_explorer").expect("agent path");
    assert!(invalid.validate().is_err());
}

#[test]
fn registration_receipt_requires_runtime_grpc_transport_and_generation() {
    let receipt = AgentSessionRegisterReceipt {
        schema_id: ClientSchemaId::new(AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID)
            .expect("receipt schema id"),
        schema_version: 1,
        state: AgentSessionRegisterState::Registered,
        platform: AgentSessionPlatform::Codex,
        project_id: ClientProjectId::new("workspace-1").expect("project id"),
        root_session_id: AgentRootSessionId::new("root-1").expect("root session id"),
        parent_thread_id: AgentParentThreadId::new("parent-1").expect("parent thread id"),
        child_thread_id: AgentChildThreadId::new("child-1").expect("child thread id"),
        agent_name: AgentName::new("asp_testing").expect("agent name"),
        agent_path: AgentPath::new("/root/asp_testing").expect("agent path"),
        route_key: AgentRouteKey::new("asp_testing").expect("route key"),
        physical_generation: 1,
        registry_owner: AgentSessionRegistryOwner::RuntimeServerAgentSessionRegistry,
        transport: AgentSessionTransport::GrpcClientFrame,
    };
    receipt.validate().expect("current registration receipt");

    let mut invalid_wire = serde_json::to_value(&receipt).expect("encode receipt");
    invalid_wire["transport"] = serde_json::json!("legacy-raw-workspace-db");
    assert!(serde_json::from_value::<AgentSessionRegisterReceipt>(invalid_wire).is_err());
    let mut invalid = receipt;
    invalid.physical_generation = 0;
    assert!(invalid.validate().is_err());
}
