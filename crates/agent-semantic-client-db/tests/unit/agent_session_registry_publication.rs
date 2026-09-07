// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::AGENT_SESSION_REGISTRY_DB_NAME;
use agent_semantic_client_db::AgentSessionRegistry;

#[tokio::test]
async fn route_slot_database_is_preserved_while_current_instance_registry_is_published() {
    let root = tempfile::tempdir().expect("fixture root");
    let state = root.path().join("state");
    std::fs::create_dir_all(&state).expect("state root");
    let archived = state.join("session-registry.turso");
    let archived_bytes = b"archived-route-slot-database";
    std::fs::write(&archived, archived_bytes).expect("archived route-slot fixture");

    let registry = AgentSessionRegistry::open_or_create_state_root(&state)
        .expect("publish current child-instance registry");
    assert_eq!(
        registry
            .db_path()
            .file_name()
            .and_then(std::ffi::OsStr::to_str),
        Some(AGENT_SESSION_REGISTRY_DB_NAME)
    );
    assert_eq!(
        std::fs::read(&archived).expect("archived database remains readable"),
        archived_bytes
    );
    let receipt: serde_json::Value = serde_json::from_slice(
        &std::fs::read(state.join("session-registry.current.v1.json"))
            .expect("current publication receipt"),
    )
    .expect("decode current publication receipt");
    assert_eq!(receipt["schemaVersion"], "1");
    assert_eq!(receipt["dbSchemaVersion"], 1);
    assert_eq!(receipt["currentDbFile"], AGENT_SESSION_REGISTRY_DB_NAME);

    let reopened =
        AgentSessionRegistry::open_or_create_state_root(&state).expect("publication is idempotent");
    assert_eq!(reopened.db_path(), registry.db_path());
}

#[tokio::test]
async fn route_slot_shape_is_typed_fail_closed_without_mutation() {
    let root = tempfile::tempdir().expect("fixture root");
    let state = root.path().join("state");
    std::fs::create_dir_all(&state).expect("state root");
    let current = state.join(AGENT_SESSION_REGISTRY_DB_NAME);
    let database = turso::Builder::new_local(current.to_string_lossy().as_ref())
        .build()
        .await
        .expect("route-slot database");
    let connection = database.connect().expect("route-slot connection");
    connection
        .execute_batch(
            "CREATE TABLE asp_agent_sessions (
                project_id TEXT NOT NULL DEFAULT 'default', root_session_id TEXT NOT NULL,
                session_id TEXT NOT NULL UNIQUE, physical_generation INTEGER NOT NULL DEFAULT 1,
                configured_agent_type TEXT, profile_evidence_json TEXT, message_target_id TEXT,
                parent_session_id TEXT, name TEXT NOT NULL, role TEXT NOT NULL, model TEXT,
                model_observation_source TEXT, model_observed_at INTEGER, model_evidence_ref TEXT,
                status TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                last_seen_at INTEGER, last_heartbeat_at INTEGER, expires_at INTEGER,
                archived_at INTEGER, last_tool_event TEXT, last_command TEXT,
                last_evidence_ref TEXT, metadata_json TEXT NOT NULL DEFAULT '{}',
                PRIMARY KEY(project_id, root_session_id, name)
            )",
        )
        .await
        .expect("route-slot schema");
    drop(connection);
    drop(database);
    std::fs::write(
        state.join("session-registry.current.v1.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.agent-session-registry-publication",
            "schemaVersion": "1",
            "dbSchemaVersion": 1,
            "currentDbFile": AGENT_SESSION_REGISTRY_DB_NAME,
            "archivedDbFile": "session-registry.turso"
        }))
        .expect("publication fixture"),
    )
    .expect("write publication fixture");
    let before = std::fs::read(&current).expect("route-slot bytes before validation");

    let error = match AgentSessionRegistry::open_or_create_state_root(&state) {
        Ok(_) => panic!("route-slot schema must not be decoded or migrated"),
        Err(error) => error,
    };
    assert!(error.contains("agent-session-v1-instance-schema-not-current"));
    assert_eq!(
        std::fs::read(&current).expect("route-slot bytes after validation"),
        before
    );
}
