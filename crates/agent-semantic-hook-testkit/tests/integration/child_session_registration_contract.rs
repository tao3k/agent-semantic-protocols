const CHILD_REGISTRATION_SCHEMA: &str =
    include_str!("../../../../schemas/codex-child-session-registration-receipt.schema.json");
const CHILD_REGISTRATION_REQUEST_SCHEMA: &str =
    include_str!("../../../../schemas/codex-child-session-registration-request.schema.json");

#[test]
fn child_registration_request_requires_explicit_parent_child_and_agent_path() {
    let schema: serde_json::Value = serde_json::from_str(CHILD_REGISTRATION_REQUEST_SCHEMA)
        .expect("child registration request schema");
    let validator =
        jsonschema::validator_for(&schema).expect("compile child registration request schema");
    let request = serde_json::json!({
        "schemaId": "agent.semantic-protocols.codex-child-session-registration-request",
        "schemaVersion": 1,
        "rootSessionId": "root-1",
        "parentThreadId": "parent-1",
        "childThreadId": "child-1",
        "agentName": "asp_explorer",
        "agentPath": "/root/asp_explorer",
        "routeKey": "asp_explorer"
    });
    assert!(validator.is_valid(&request));

    for missing in [
        "rootSessionId",
        "parentThreadId",
        "childThreadId",
        "agentPath",
    ] {
        let mut invalid = request.clone();
        invalid
            .as_object_mut()
            .expect("request object")
            .remove(missing);
        assert!(
            !validator.is_valid(&invalid),
            "registration request schema must reject missing {missing}"
        );
    }
}

#[test]
fn child_registration_receipt_requires_explicit_parent_child_and_agent_path() {
    let schema: serde_json::Value =
        serde_json::from_str(CHILD_REGISTRATION_SCHEMA).expect("child registration schema");
    let validator = jsonschema::validator_for(&schema).expect("compile child registration schema");
    let receipt = serde_json::json!({
        "schemaId": "agent.semantic-protocols.codex-child-session-registration-receipt",
        "schemaVersion": 1,
        "state": "registered",
        "platform": "codex",
        "projectId": "workspace-1",
        "rootSessionId": "root-1",
        "parentThreadId": "parent-1",
        "childThreadId": "child-1",
        "agentName": "asp_explorer",
        "agentPath": "/root/asp_explorer",
        "routeKey": "asp_explorer",
        "physicalGeneration": 1,
        "registryOwner": "runtime-server-agent-session-registry",
        "transport": "grpc-client-frame"
    });
    assert!(validator.is_valid(&receipt));

    for missing in [
        "rootSessionId",
        "parentThreadId",
        "childThreadId",
        "agentPath",
        "physicalGeneration",
        "transport",
    ] {
        let mut invalid = receipt.clone();
        invalid
            .as_object_mut()
            .expect("receipt object")
            .remove(missing);
        assert!(
            !validator.is_valid(&invalid),
            "registration schema must reject missing {missing}"
        );
    }

    let mut wrong_transport = receipt;
    wrong_transport["transport"] = serde_json::json!("legacy-raw-workspace-db");
    assert!(!validator.is_valid(&wrong_transport));
}
