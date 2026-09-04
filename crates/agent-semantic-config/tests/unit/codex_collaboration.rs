use super::CodexCollaborationOperation;
use super::CodexCollaborationToolCall;
use super::CodexMultiAgentV2Interface;
use super::CollaborationDispatchAction;
use super::CollaborationDispatchState;
use super::CollaborationHostResultKind;
use super::CollaborationLifecycleTool;
use super::CollaborationLiveAgents;
use super::CollaborationRegistrationState;
use super::state_after_interrupt;

const TOOL_FIXTURE_ROOT: &str = "../../schemas/fixtures/codex-collaboration-tool-call";

fn snapshot(value: serde_json::Value) -> CollaborationLiveAgents {
    serde_json::from_value(value).expect("valid collaboration.list_agents response")
}

#[test]
fn current_registration_resumes_existing_agent_without_recreating_it() {
    let agents = snapshot(serde_json::json!({
        "agents": [
            {"agent_name": "/root", "agent_status": "running"},
            {"agent_name": "/root/asp_testing", "agent_status": {"completed": "done"}},
            {"agent_name": "/root/asp_explorer", "agent_status": "running"}
        ]
    }));

    assert_eq!(
        CodexMultiAgentV2Interface::choose_dispatch(
            &agents,
            "/root/asp_testing",
            CollaborationRegistrationState::Current,
        )
        .unwrap(),
        CollaborationDispatchAction::FollowupTask
    );
    assert_eq!(
        agents
            .dispatch_action(
                "/root/asp_explorer",
                CollaborationRegistrationState::Current
            )
            .unwrap(),
        CollaborationDispatchAction::FollowupTask
    );
    assert_eq!(
        agents
            .dispatch_action("/root/asp_coding", CollaborationRegistrationState::Current)
            .unwrap(),
        CollaborationDispatchAction::SpawnAgentAndRegister
    );
}

#[test]
fn stale_registration_is_repaired_in_place_without_inventing_a_host_path() {
    let agents = snapshot(serde_json::json!({
        "agents": [
            {"agent_name": "/root", "agent_status": "running"},
            {"agent_name": "/root/asp_testing", "agent_status": {"completed": "done"}},
            {"agent_name": "/root/asp_explorer", "agent_status": "running"}
        ]
    }));

    assert_eq!(
        agents
            .dispatch_action(
                "/root/asp_testing",
                CollaborationRegistrationState::MissingOrStale
            )
            .unwrap(),
        CollaborationDispatchAction::FollowupTaskAndRegister
    );
    assert_eq!(
        agents
            .dispatch_action(
                "/root/asp_explorer",
                CollaborationRegistrationState::MissingOrStale
            )
            .unwrap(),
        CollaborationDispatchAction::FollowupTaskAndRegister
    );
    assert_eq!(
        agents
            .dispatch_action(
                "/root/asp_coding",
                CollaborationRegistrationState::MissingOrStale
            )
            .unwrap(),
        CollaborationDispatchAction::SpawnAgentAndRegister
    );
}

#[test]
fn absent_path_requires_spawn_and_existing_paths_use_host_status() {
    let agents = snapshot(serde_json::json!({
        "agents": [
            {"agent_name": "/root", "agent_status": "running"},
            {"agent_name": "/root/asp_testing", "agent_status": {"completed": "hello"}},
            {"agent_name": "/root/asp_explorer", "agent_status": "running"}
        ]
    }));

    assert_eq!(
        agents.dispatch_state("/root/asp_coding").unwrap(),
        CollaborationDispatchState::Absent
    );
    assert_eq!(
        agents.dispatch_state("/root/asp_testing").unwrap(),
        CollaborationDispatchState::Reusable
    );
    assert_eq!(
        agents.dispatch_state("/root/asp_explorer").unwrap(),
        CollaborationDispatchState::Running
    );
}

#[test]
fn missing_root_duplicate_or_invented_path_fails_closed() {
    for value in [
        serde_json::json!({"agents": []}),
        serde_json::json!({"agents": [
            {"agent_name": "/root", "agent_status": "running"},
            {"agent_name": "/root", "agent_status": "running"}
        ]}),
        serde_json::json!({"agents": [
            {"agent_name": "/root", "agent_status": "running"},
            {"agent_name": "/other/asp_testing", "agent_status": "running"}
        ]}),
        serde_json::json!({"agents": [
            {"agent_name": "/root", "agent_status": "running"},
            {"agent_name": "/root/asp_testing", "agent_status": "imaginary"}
        ]}),
        serde_json::json!({"agents": [
            {"agent_name": "/root", "agent_status": "running"},
            {"agent_name": "/root/asp_testing", "agent_status": "not_found"}
        ]}),
    ] {
        let invalid = match serde_json::from_value::<CollaborationLiveAgents>(value) {
            Ok(agents) => agents.validate_root_tree().is_err(),
            Err(_) => true,
        };
        assert!(invalid);
    }
}

#[test]
fn required_dispatch_is_invariant_when_running_becomes_completed() {
    let running = snapshot(serde_json::json!({
        "agents": [
            {"agent_name": "/root", "agent_status": "running"},
            {"agent_name": "/root/asp_testing", "agent_status": "running"}
        ]
    }));
    let completed = snapshot(serde_json::json!({
        "agents": [
            {"agent_name": "/root", "agent_status": "running"},
            {"agent_name": "/root/asp_testing", "agent_status": {"completed": null}}
        ]
    }));

    for registration in [
        CollaborationRegistrationState::Current,
        CollaborationRegistrationState::MissingOrStale,
    ] {
        assert_eq!(
            running
                .dispatch_action("/root/asp_testing", registration)
                .unwrap(),
            completed
                .dispatch_action("/root/asp_testing", registration)
                .unwrap()
        );
    }
}

#[test]
fn lifecycle_tools_keep_turn_creation_observation_and_interruption_distinct() {
    assert!(CollaborationLifecycleTool::SpawnAgent.starts_turn());
    assert!(CollaborationLifecycleTool::FollowupTask.starts_turn());
    assert!(!CollaborationLifecycleTool::SendMessage.starts_turn());
    assert!(!CollaborationLifecycleTool::InterruptAgent.starts_turn());
    assert!(CollaborationLifecycleTool::ListAgents.observes_only());
    assert!(CollaborationLifecycleTool::WaitAgent.observes_only());
    assert!(CollaborationLifecycleTool::SendMessage.requires_existing_target());
    assert!(CollaborationLifecycleTool::FollowupTask.requires_existing_target());
    assert!(CollaborationLifecycleTool::InterruptAgent.requires_existing_target());
    assert!(!CollaborationLifecycleTool::WaitAgent.requires_existing_target());
}

#[test]
fn interrupt_preserves_the_agent_path_for_later_followup() {
    assert_eq!(
        state_after_interrupt(CollaborationDispatchState::Running),
        CollaborationDispatchState::Reusable
    );
    assert_eq!(
        state_after_interrupt(CollaborationDispatchState::Reusable),
        CollaborationDispatchState::Reusable
    );
    assert_eq!(
        state_after_interrupt(CollaborationDispatchState::Absent),
        CollaborationDispatchState::Absent
    );
    assert!(CollaborationLifecycleTool::InterruptAgent.preserves_existing_agent_path());
    assert!(!CollaborationLifecycleTool::SpawnAgent.preserves_existing_agent_path());
}

#[test]
fn one_typed_interface_decodes_and_validates_every_codex_collaboration_v2_call() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (fixture, expected_tool) in [
        (
            "valid-spawn-agent.v1.json",
            CollaborationLifecycleTool::SpawnAgent,
        ),
        (
            "valid-list-agents.v1.json",
            CollaborationLifecycleTool::ListAgents,
        ),
        (
            "valid-list-agents-root.v1.json",
            CollaborationLifecycleTool::ListAgents,
        ),
        (
            "valid-followup-task.v1.json",
            CollaborationLifecycleTool::FollowupTask,
        ),
        (
            "valid-send-message.v1.json",
            CollaborationLifecycleTool::SendMessage,
        ),
        (
            "valid-interrupt-agent.v1.json",
            CollaborationLifecycleTool::InterruptAgent,
        ),
        (
            "valid-wait-agent.v1.json",
            CollaborationLifecycleTool::WaitAgent,
        ),
    ] {
        let bytes = std::fs::read(manifest_dir.join(TOOL_FIXTURE_ROOT).join(fixture))
            .expect("read Codex Collaboration fixture");
        let call = CodexMultiAgentV2Interface::decode_tool_call(&bytes)
            .expect("decode typed Collaboration call");
        assert_eq!(call.tool(), expected_tool, "fixture={fixture}");
    }
}

#[test]
fn v2_interface_keeps_dispatch_context_and_lifecycle_control_distinct() {
    assert!(CollaborationLifecycleTool::SpawnAgent.dispatches_required_work());
    assert!(CollaborationLifecycleTool::FollowupTask.dispatches_required_work());
    for tool in [
        CollaborationLifecycleTool::ListAgents,
        CollaborationLifecycleTool::SendMessage,
        CollaborationLifecycleTool::InterruptAgent,
        CollaborationLifecycleTool::WaitAgent,
    ] {
        assert!(!tool.dispatches_required_work(), "tool={tool:?}");
    }
    assert_eq!(
        CollaborationLifecycleTool::SpawnAgent.host_result_kind(),
        CollaborationHostResultKind::SpawnedAgent
    );
    assert_eq!(
        CollaborationLifecycleTool::ListAgents.host_result_kind(),
        CollaborationHostResultKind::LiveAgents
    );
    assert_eq!(
        CollaborationLifecycleTool::FollowupTask.host_result_kind(),
        CollaborationHostResultKind::EmptyActivity
    );
    assert_eq!(
        CollaborationLifecycleTool::InterruptAgent.host_result_kind(),
        CollaborationHostResultKind::PreviousStatus
    );
    assert_eq!(
        CollaborationLifecycleTool::WaitAgent.host_result_kind(),
        CollaborationHostResultKind::WaitSummary
    );
}

#[test]
fn typed_interface_rejects_v1_or_invented_collaboration_shapes() {
    for value in [
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.codex-collaboration-tool-call",
            "schemaVersion": "1",
            "namespace": "multi_agent_v1",
            "toolName": "spawn_agent",
            "toolInput": {"task_name": "asp_testing", "message": "test"}
        }),
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.codex-collaboration-tool-call",
            "schemaVersion": "1",
            "namespace": "collaboration",
            "toolName": "send_input",
            "toolInput": {"target": "/root/asp_testing", "message": "test"}
        }),
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.codex-collaboration-tool-call",
            "schemaVersion": "1",
            "namespace": "collaboration",
            "toolName": "followup_task",
            "toolInput": {"target": "/root", "message": "test"}
        }),
    ] {
        let rejected = serde_json::from_value::<CodexCollaborationToolCall>(value)
            .map_or(true, |call| call.validate().is_err());
        assert!(rejected);
    }

    let call = CodexCollaborationToolCall::new(CodexCollaborationOperation::WaitAgent(
        super::WaitAgentInput {
            timeout_ms: Some(9_999),
        },
    ));
    assert!(call.validate().is_err());
}
