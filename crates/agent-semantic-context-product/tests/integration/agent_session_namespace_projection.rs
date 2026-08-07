use agent_semantic_context_product::agent_session_namespace_projection::{
    AgentSessionNamespaceProjection, ChoicePlaneDecision, CodexHostAction, DurableNamespaceState,
    HostChildLiveness,
};

const HOST_LIVENESS_STATES: [HostChildLiveness; 3] = [
    HostChildLiveness::Running,
    HostChildLiveness::Idle,
    HostChildLiveness::Terminated,
];

#[test]
fn only_an_absent_namespace_creates_and_registers() {
    for host_liveness in HOST_LIVENESS_STATES {
        let projection = AgentSessionNamespaceProjection::new(
            "project-1",
            "workspace-1",
            "/repo",
            agent_semantic_context_product::agent_session_namespace_projection::PlatformSessionIdentity::from_platform_environment(
                "codex",
                "session-1",
            ),
            "root/session/@asp_explorer",
            "@asp_explorer",
            0,
            DurableNamespaceState::Absent,
            host_liveness,
        );

        assert_eq!(
            projection.namespace_action,
            agent_semantic_context_product::agent_session_namespace_projection::NamespaceAction::Create
        );
        assert_eq!(projection.decision, ChoicePlaneDecision::CreateNew);
        assert_eq!(projection.host_action, CodexHostAction::SpawnAgent);
        projection.validate().expect("absent projection is valid");
    }
}

#[test]
fn every_present_namespace_uses_followup_independent_of_host_liveness() {
    for host_liveness in HOST_LIVENESS_STATES {
        let projection = AgentSessionNamespaceProjection::new(
            "project-1",
            "workspace-1",
            "/repo",
            agent_semantic_context_product::agent_session_namespace_projection::PlatformSessionIdentity::from_platform_environment(
                "codex",
                "session-1",
            ),
            "root/session/@asp_explorer",
            "@asp_explorer",
            7,
            DurableNamespaceState::Present,
            host_liveness,
        );

        assert_eq!(
            projection.namespace_action,
            agent_semantic_context_product::agent_session_namespace_projection::NamespaceAction::Resume
        );
        assert_eq!(projection.decision, ChoicePlaneDecision::ContinueExisting);
        assert_eq!(projection.host_action, CodexHostAction::FollowupTask);
        projection
            .validate()
            .expect("present namespace projection is valid");
    }
}

#[test]
fn a_present_namespace_never_projects_spawn_agent() {
    for host_liveness in HOST_LIVENESS_STATES {
        let projection = AgentSessionNamespaceProjection::new(
            "project-1",
            "workspace-1",
            "/repo",
            agent_semantic_context_product::agent_session_namespace_projection::PlatformSessionIdentity::from_platform_environment(
                "codex",
                "session-1",
            ),
            "root/session/@asp_testing",
            "@asp_testing",
            3,
            DurableNamespaceState::Present,
            host_liveness,
        );

        assert_eq!(
            projection.namespace_action,
            agent_semantic_context_product::agent_session_namespace_projection::NamespaceAction::Resume
        );
        assert_eq!(projection.decision, ChoicePlaneDecision::ContinueExisting);
        assert_eq!(projection.host_action, CodexHostAction::FollowupTask);
        projection.validate().expect("achieved projection is valid");
    }
}

#[test]
fn a_forged_action_cannot_override_durable_namespace_state() {
    let mut projection = AgentSessionNamespaceProjection::new(
        "project-1",
        "workspace-1",
        "/repo",
        agent_semantic_context_product::agent_session_namespace_projection::PlatformSessionIdentity::from_platform_environment(
            "codex",
            "session-1",
        ),
        "root/session/@asp_explorer",
        "@asp_explorer",
        7,
        DurableNamespaceState::Present,
        HostChildLiveness::Terminated,
    );
    projection.decision = ChoicePlaneDecision::CreateNew;
    projection.host_action = CodexHostAction::SpawnAgent;

    assert_eq!(
        projection.validate().as_deref(),
        Err("agent session namespace transition does not match durable namespace state")
    );
}

#[test]
fn another_codex_session_cannot_reuse_the_current_namespace_binding() {
    let current = AgentSessionNamespaceProjection::new(
        "project-1",
        "workspace-1",
        "/repo",
        agent_semantic_context_product::agent_session_namespace_projection::PlatformSessionIdentity::from_platform_environment(
            "codex",
            "session-current",
        ),
        "root/session-current/@asp_explorer",
        "@asp_explorer",
        4,
        DurableNamespaceState::Present,
        HostChildLiveness::Running,
    );
    let stale = AgentSessionNamespaceProjection::new(
        "project-1",
        "workspace-1",
        "/repo",
        agent_semantic_context_product::agent_session_namespace_projection::PlatformSessionIdentity::from_platform_environment(
            "codex",
            "session-stale",
        ),
        "root/session-stale/@asp_explorer",
        "@asp_explorer",
        9,
        DurableNamespaceState::Present,
        HostChildLiveness::Terminated,
    );

    assert_ne!(current.namespace_key(), stale.namespace_key());
}

#[test]
fn codex_resume_serializes_the_exact_namespace_and_host_actions() {
    let projection = AgentSessionNamespaceProjection::new(
        "project-1",
        "workspace-1",
        "/repo",
        agent_semantic_context_product::agent_session_namespace_projection::PlatformSessionIdentity::from_platform_environment(
            "codex",
            "session-current",
        ),
        "root/session-current/@asp_explorer",
        "@asp_explorer",
        4,
        DurableNamespaceState::Present,
        HostChildLiveness::Terminated,
    );

    let value = serde_json::to_value(projection).expect("projection serializes");
    assert_eq!(value["sessionIdentity"]["platform"], "codex");
    assert_eq!(
        value["sessionIdentity"]["platformSessionId"],
        "session-current"
    );
    assert_eq!(value["namespaceAction"], "resume");
    assert_eq!(value["hostAction"], "followup_task");
    assert!(value.get("platformSessionId").is_none());
}

#[test]
fn another_workspace_cannot_reuse_the_current_namespace_binding() {
    let session_identity =
        agent_semantic_context_product::agent_session_namespace_projection::PlatformSessionIdentity::from_platform_environment(
            "codex",
            "session-current",
        );
    let left = AgentSessionNamespaceProjection::new(
        "project-1",
        "workspace-left",
        "/repo-left",
        session_identity.clone(),
        "root/session-current/@asp_explorer",
        "@asp_explorer",
        4,
        DurableNamespaceState::Present,
        HostChildLiveness::Running,
    );
    let right = AgentSessionNamespaceProjection::new(
        "project-1",
        "workspace-right",
        "/repo-right",
        session_identity,
        "root/session-current/@asp_explorer",
        "@asp_explorer",
        4,
        DurableNamespaceState::Present,
        HostChildLiveness::Running,
    );

    assert_ne!(left.namespace_key(), right.namespace_key());
}

#[test]
fn one_workspace_identity_cannot_bind_two_canonical_roots() {
    let session_identity =
        agent_semantic_context_product::agent_session_namespace_projection::PlatformSessionIdentity::from_platform_environment(
            "codex",
            "session-current",
        );
    let root = AgentSessionNamespaceProjection::new(
        "project-1",
        "workspace-1",
        "/repo",
        session_identity.clone(),
        "root/session-current/@asp_explorer",
        "@asp_explorer",
        4,
        DurableNamespaceState::Present,
        HostChildLiveness::Running,
    );
    let nested = AgentSessionNamespaceProjection::new(
        "project-1",
        "workspace-1",
        "/repo/crates/agent-semantic-protocol",
        session_identity,
        "root/session-current/@asp_explorer",
        "@asp_explorer",
        4,
        DurableNamespaceState::Present,
        HostChildLiveness::Running,
    );

    assert_ne!(root.namespace_key(), nested.namespace_key());
}
