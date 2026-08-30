use agent_semantic_config::{
    COLLABORATION_LIVE_AGENT_SNAPSHOT_SCHEMA_ID, CollaborationLiveAgentSnapshot,
};
use serde_json::json;

use super::{decode_live_agents, publish_snapshot};

#[test]
fn post_tool_live_agents_decode_direct_and_content_shapes() {
    let direct = json!({
        "agents": [
            { "agent_name": "/root", "agent_status": "running" },
            { "agent_name": "/root/asp_testing", "agent_status": { "completed": "ok" } }
        ]
    });
    let decoded = decode_live_agents(&direct).unwrap();
    decoded.validate().unwrap();

    let wrapped = json!({
        "content": [{ "type": "text", "text": serde_json::to_string(&direct).unwrap() }]
    });
    assert_eq!(decode_live_agents(&wrapped).unwrap(), decoded);
}

#[test]
fn snapshot_publication_is_atomic_and_session_keyed() {
    let state_home = tempfile::tempdir().unwrap();
    let snapshot = CollaborationLiveAgentSnapshot {
        schema_id: COLLABORATION_LIVE_AGENT_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        workspace_root: "/workspace".to_owned(),
        root_session_id: "root-session".to_owned(),
        observed_at_unix_ms: 1,
        agents: decode_live_agents(&json!({
            "agents": [{ "agent_name": "/root", "agent_status": "running" }]
        }))
        .unwrap(),
    };

    publish_snapshot(state_home.path(), &snapshot).unwrap();
    let entries = std::fs::read_dir(state_home.path().join("agents/live"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(entries.len(), 1);
    let persisted: CollaborationLiveAgentSnapshot =
        serde_json::from_slice(&std::fs::read(entries[0].path()).unwrap()).unwrap();
    assert_eq!(persisted, snapshot);
}
