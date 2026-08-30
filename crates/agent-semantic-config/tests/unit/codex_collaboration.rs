use super::{CollaborationDispatchState, CollaborationLiveAgents};

fn snapshot(value: serde_json::Value) -> CollaborationLiveAgents {
    serde_json::from_value(value).expect("valid collaboration.list_agents response")
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
    ] {
        assert!(snapshot(value).validate().is_err());
    }
}
