use crate::workspace_db_ipc::{
    AgentHostLifecycleEventIpc, AgentHostLifecycleEventKind, AgentSessionRegistryIpcResult,
};

#[test]
fn host_start_registers_and_host_stop_archives_one_physical_generation() {
    let root = tempfile::tempdir().expect("Host lifecycle registry tempdir");
    let registry =
        crate::AgentSessionRegistry::open_or_create_state_root(root.path().join("state"))
            .expect("open Host lifecycle registry");
    let start = AgentHostLifecycleEventIpc {
        kind: AgentHostLifecycleEventKind::Started,
        platform: "codex".to_owned(),
        project_id: "workspace-1".to_owned(),
        root_session_id: "root-1".to_owned(),
        parent_session_id: "parent-1".to_owned(),
        child_session_id: "child-1".to_owned(),
        host_task_name: "asp_explorer".to_owned(),
        platform_host_agent_name: "asp_explorer".to_owned(),
        route_key: "asp_explorer".to_owned(),
        profile_id: "agents/asp_explorer.toml".to_owned(),
        role: "explore".to_owned(),
        model: "gpt-test".to_owned(),
        model_digest: "blake3-256:model".to_owned(),
        profile_digest: "blake3-256:profile".to_owned(),
        sandbox_mode: "read-only".to_owned(),
        session_lifetime: "resident".to_owned(),
        payload_digest: "blake3-256:payload".to_owned(),
        transcript_path: Some("/tmp/child.jsonl".to_owned()),
        observed_at: 10,
    };

    let registered = super::record_host_lifecycle_event(&registry, start.clone())
        .expect("record Host start event");
    let AgentSessionRegistryIpcResult::Registered { session } = registered else {
        panic!("Host start must return the registered generation");
    };
    assert_eq!(session.session_id.as_str(), "child-1");
    assert_eq!(session.parent_session_id.as_deref(), Some("parent-1"));
    assert_eq!(session.name.as_str(), "asp_explorer");
    assert_eq!(session.status.as_str(), "active");
    assert_eq!(
        session.configured_agent_type.as_deref(),
        Some("asp_explorer")
    );
    assert_eq!(session.model.as_deref(), Some("gpt-test"));
    assert!(session.metadata_json.contains("blake3-256:profile"));
    assert!(session.metadata_json.contains("agent-session-host-binding"));
    assert!(
        session
            .metadata_json
            .contains("\"matchDecision\":\"matched\"")
    );

    let stopped = super::record_host_lifecycle_event(
        &registry,
        AgentHostLifecycleEventIpc {
            kind: AgentHostLifecycleEventKind::Stopped,
            observed_at: 20,
            ..start
        },
    )
    .expect("record Host stop event");
    assert_eq!(
        stopped,
        AgentSessionRegistryIpcResult::Changed { changed: true }
    );
    let archived = registry
        .session_by_id("workspace-1", "child-1")
        .expect("query archived generation")
        .expect("archived generation exists");
    assert_eq!(archived.status.as_str(), "archived");
}

#[test]
fn resident_namespace_prefers_live_instance_and_rejects_terminal_reuse() {
    let root = tempfile::tempdir().expect("Host lifecycle registry tempdir");
    let registry =
        crate::AgentSessionRegistry::open_or_create_state_root(root.path().join("state"))
            .expect("open Host lifecycle registry");
    let start = AgentHostLifecycleEventIpc {
        kind: AgentHostLifecycleEventKind::Started,
        platform: "codex".to_owned(),
        project_id: "workspace-1".to_owned(),
        root_session_id: "root-1".to_owned(),
        parent_session_id: "root-1".to_owned(),
        child_session_id: "child-1".to_owned(),
        host_task_name: "asp_explorer".to_owned(),
        platform_host_agent_name: "asp_explorer".to_owned(),
        route_key: "asp_explorer".to_owned(),
        profile_id: "agents/asp_explorer.toml".to_owned(),
        role: "explore".to_owned(),
        model: "gpt-test".to_owned(),
        model_digest: "blake3-256:model".to_owned(),
        profile_digest: "blake3-256:profile".to_owned(),
        sandbox_mode: "read-only".to_owned(),
        session_lifetime: "resident".to_owned(),
        payload_digest: "blake3-256:payload".to_owned(),
        transcript_path: Some("/tmp/child.jsonl".to_owned()),
        observed_at: 10,
    };
    super::record_host_lifecycle_event(&registry, start.clone())
        .expect("first resident registration");

    let duplicate = AgentHostLifecycleEventIpc {
        child_session_id: "child-2".to_owned(),
        observed_at: 11,
        ..start.clone()
    };
    let duplicate_error = super::record_host_lifecycle_event(&registry, duplicate)
        .expect_err("a live resident must be called or resumed before another spawn");
    assert!(duplicate_error.starts_with("resident-first-call-resume-required:"));

    super::record_host_lifecycle_event(
        &registry,
        AgentHostLifecycleEventIpc {
            kind: AgentHostLifecycleEventKind::Stopped,
            observed_at: 12,
            ..start.clone()
        },
    )
    .expect("terminalize exact resident");
    let reuse_error = super::record_host_lifecycle_event(
        &registry,
        AgentHostLifecycleEventIpc {
            observed_at: 13,
            ..start
        },
    )
    .expect_err("terminal child identity must not be reused");
    assert!(reuse_error.starts_with("terminal-host-instance-reuse:"));
}
