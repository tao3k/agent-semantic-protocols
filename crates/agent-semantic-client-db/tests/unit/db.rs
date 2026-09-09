// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::source_index_fixture::build_fixture_source_index_import;

use agent_semantic_client_core::CacheGenerationId;
use agent_semantic_client_core::ClientCacheFileHash;
use agent_semantic_client_core::LanguageId;
use agent_semantic_client_core::ProviderId;
use agent_semantic_client_core::SemanticSchemaId;
use agent_semantic_client_core::SemanticSchemaVersion;
use agent_semantic_client_core::state_core::ResolvedState;
use agent_semantic_client_db::AGENT_SESSION_REGISTRY_DB_NAME;
use agent_semantic_client_db::AgentSessionDispatchClaimRequest;
use agent_semantic_client_db::AgentSessionDispatchCompleteRequest;
use agent_semantic_client_db::AgentSessionRegisterRequest;
use agent_semantic_client_db::AgentSessionRegistry;
use agent_semantic_client_db::AgentSessionToolEventRequest;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_PROVIDER_ID;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID;
use agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION;
use agent_semantic_client_db::ClientDbSourceIndexImportAssemblyRequest;
use agent_semantic_client_db::ClientDbSourceIndexImportFile;
use agent_semantic_client_db::ClientDbSourceIndexImportRequest;
use agent_semantic_client_db::ClientDbSourceIndexRefreshRequest;
use agent_semantic_client_db::ClientDbSourceIndexScopeFile;
use agent_semantic_client_db::ClientDbSourceIndexSource;
use agent_semantic_client_db::client_db_source_index_file_count;
use agent_semantic_client_db::source_index_relative_path;
use agent_semantic_client_db::source_index_scope_dirs;

#[tokio::test]
async fn schema_version_stays_on_first_turso_release_contract() {
    assert_eq!(
        agent_semantic_client_db::AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION,
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn runtime_registry_bootstrap_does_not_start_a_nested_tokio_runtime() {
    let root = temp_root("agent-session-async-bootstrap");
    let registry = AgentSessionRegistry::open_or_create_state_root_async(root.join("state"))
        .await
        .expect("bootstrap registry inside the resident Tokio runtime");

    assert!(registry.db_path().is_file());
}

#[tokio::test]
async fn absent_host_target_atomically_revokes_live_binding() {
    let root = temp_root("agent-session-absent-target");
    let registry = AgentSessionRegistry::open_or_create_state_root(root.join("state"))
        .expect("create registry");
    let metadata = serde_json::json!({
        "messageTargetBinding": {
            "source": "codex.subagent-start",
            "boundRootSessionId": "root-session",
            "childSessionId": "child-session",
            "messageTargetId": "child-session"
        },
        "dispatchLease": {
            "dispatchIdentity": "dispatch-1",
            "commandDigest": "sha256:command",
            "deliveryTargetId": "child-session",
            "status": "in-flight"
        },
        "preserved": true
    })
    .to_string();
    let record = registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1".into(),
            root_session_id: "root-session".into(),
            session_id: "child-session".into(),
            message_target_id: Some("child-session".into()),
            parent_session_id: Some("root-session".into()),
            name: "asp-explore".into(),
            role: "asp_explorer".into(),
            model_observation: None,
            status: "active".into(),
            expires_at: None,
            metadata_json: (&metadata).into(),
            now: 10,
        })
        .await
        .expect("register live child");
    assert!(
        agent_semantic_client_db::agent_session_message_target_is_live_bound(
            &record,
            "root-session"
        )
    );

    let orphaned = registry
        .invalidate_session_live_binding("project-1", "child-session", "orphan-risk", 11)
        .await
        .expect("invalidate live binding")
        .expect("updated session");

    assert_eq!(orphaned.status(), "orphan-risk");
    assert_eq!(orphaned.message_target_id, None);
    let metadata: serde_json::Value =
        serde_json::from_str(&orphaned.metadata_json).expect("metadata json");
    assert!(metadata.get("messageTargetBinding").is_none());
    assert_eq!(
        metadata.get("preserved"),
        Some(&serde_json::Value::Bool(true))
    );
    let dispatch = metadata
        .get("dispatchLease")
        .expect("preserved dispatch lease");
    assert_eq!(
        dispatch
            .get("dispatchIdentity")
            .and_then(serde_json::Value::as_str),
        Some("dispatch-1")
    );
    assert_eq!(
        dispatch
            .get("commandDigest")
            .and_then(serde_json::Value::as_str),
        Some("sha256:command")
    );
    assert_eq!(
        dispatch.get("status").and_then(serde_json::Value::as_str),
        Some("orphaned-awaiting-rebind")
    );
    assert!(
        dispatch
            .get("deliveryTargetId")
            .is_some_and(serde_json::Value::is_null)
    );
    assert_eq!(
        dispatch
            .get("revokedAt")
            .and_then(serde_json::Value::as_i64),
        Some(11)
    );
    assert!(
        !agent_semantic_client_db::agent_session_message_target_is_live_bound(
            &orphaned,
            "root-session"
        )
    );
}

#[tokio::test]
async fn typed_profile_evidence_survives_same_generation_heartbeat() {
    let root = temp_root("agent-session-profile-evidence");
    let registry = AgentSessionRegistry::open_or_create_state_root(root.join("state"))
        .expect("create registry");
    let typed_start = r#"{"event":"subagent-start","native":true,"rootSessionId":"root-session","childSessionId":"child-1","agentType":"asp_testing"}"#;
    registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1".into(),
            root_session_id: "root-session".into(),
            session_id: "child-1".into(),
            message_target_id: None,
            parent_session_id: Some("root-session".into()),
            name: "asp-testing".into(),
            role: "build,subagent,testing".into(),
            model_observation: None,
            status: "active".into(),
            expires_at: None,
            metadata_json: typed_start.into(),
            now: 10,
        })
        .await
        .expect("register typed start");
    let heartbeat = registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1".into(),
            root_session_id: "root-session".into(),
            session_id: "child-1".into(),
            message_target_id: None,
            parent_session_id: Some("root-session".into()),
            name: "asp-testing".into(),
            role: "build,subagent,testing".into(),
            model_observation: None,
            status: "idle".into(),
            expires_at: None,
            metadata_json: r#"{"event":"task_complete"}"#.into(),
            now: 11,
        })
        .await
        .expect("record same-generation heartbeat");
    assert_eq!(heartbeat.physical_generation, 1);
    assert_eq!(
        heartbeat.configured_agent_type.as_deref(),
        Some("asp_testing")
    );
    assert_eq!(
        heartbeat.profile_evidence_json.as_deref(),
        Some(typed_start)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn same_route_children_coexist_and_route_only_lookup_is_ambiguous() {
    let root = temp_root("agent-session-exact-replacement");
    let registry = AgentSessionRegistry::open_or_create_state_root(root.join("state"))
        .expect("create registry");
    registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1".into(),
            root_session_id: "root-session".into(),
            session_id: "child-old".into(),
            message_target_id: None,
            parent_session_id: Some("root-session".into()),
            name: "asp-explore".into(),
            role: "asp_explorer".into(),
            model_observation: None,
            status: "orphan-risk".into(),
            expires_at: None,
            metadata_json: "{}".into(),
            now: 10,
        })
        .await
        .expect("register old child");
    assert_eq!(
        registry
            .session_by_name("project-1", "root-session", "asp-explore")
            .await
            .expect("read initial route")
            .expect("initial route exists")
            .physical_generation,
        1
    );
    let second_child = registry.register_session(AgentSessionRegisterRequest {
        project_id: "project-1".into(),
        root_session_id: "root-session".into(),
        session_id: "child-rogue".into(),
        message_target_id: None,
        parent_session_id: Some("root-session".into()),
        name: "asp-explore".into(),
        role: "asp_explorer".into(),
        model_observation: None,
        status: "active".into(),
        expires_at: None,
        metadata_json: "{}".into(),
        now: 10,
    });
    let second_child = second_child
        .await
        .expect("ordinary registration creates a distinct child execution instance");
    assert_eq!(second_child.session_id(), "child-rogue");
    assert_eq!(second_child.physical_generation, 1);
    assert_eq!(
        registry
            .query_sessions(
                "project-1",
                Some("root-session".into()),
                Some("asp-explore".into()),
            )
            .await
            .expect("read both child instances")
            .len(),
        2
    );
    assert!(
        registry
            .session_by_name("project-1", "root-session", "asp-explore")
            .await
            .expect_err("route-only lookup must not choose between child instances")
            .contains("agent-session-route-ambiguous")
    );

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn dispatch_rebind_replays_once_and_terminal_receipt_stops_replay() {
    let root = temp_root("agent-session-dispatch-rebind");
    let registry = AgentSessionRegistry::open_or_create_state_root(root.join("state"))
        .expect("create registry");
    registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1".into(),
            root_session_id: "root-session".into(),
            session_id: "child-1".into(),
            message_target_id: Some("child-1".into()),
            parent_session_id: Some("root-session".into()),
            name: "asp-explore".into(),
            role: "asp_explorer".into(),
            model_observation: None,
            status: "active".into(),
            expires_at: None,
            metadata_json: r#"{"messageTargetBinding":{"source":"codex.subagent-start","boundRootSessionId":"root-session","childSessionId":"child-1","messageTargetId":"child-1"}}"#.into(),
            now: 10,
        })
.await.expect("register first child");

    let claim = |child_session_id, now| AgentSessionDispatchClaimRequest {
        project_id: "project-1",
        root_session_id: "root-session",
        child_session_id,
        name: "asp-explore",
        dispatch_identity: "dispatch-1",
        command_digest: "sha256:command",
        delivery_target_override: None,
        now,
    };
    let first = registry
        .claim_dispatch(claim("child-1", 11))
        .await
        .expect("claim dispatch");
    assert_eq!(first.action, "send");
    assert_eq!(first.lease.attempt_count, 1);
    assert_eq!(first.lease.delivery_target_id.as_deref(), Some("child-1"));
    assert_eq!(
        first.lease.delivery_generation_id.as_deref(),
        Some("child-1")
    );
    let duplicate = registry
        .claim_dispatch(claim("child-1", 12))
        .await
        .expect("poll dispatch");
    assert_eq!(duplicate.action, "wait");
    assert_eq!(duplicate.lease.attempt_count, 1);

    registry
        .invalidate_session_live_binding("project-1", "child-1", "orphan-risk", 13)
        .await
        .expect("invalidate first child")
        .expect("first child existed");
    let same_generation_replay = registry.claim_dispatch(AgentSessionDispatchClaimRequest {
        project_id: "project-1",
        root_session_id: "root-session",
        child_session_id: "child-1",
        name: "asp-explore",
        dispatch_identity: "dispatch-1",
        command_digest: "sha256:command",
        delivery_target_override: Some("child-1"),
        now: 13,
    });
    assert!(
        same_generation_replay
            .await
            .expect_err("orphaned delivery cannot replay within the same generation")
            .contains("not deliverable in the current generation")
    );
    registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1".into(),
            root_session_id: "root-session".into(),
            session_id: "child-2".into(),
            message_target_id: Some("child-2".into()),
            parent_session_id: Some("root-session".into()),
            name: "asp-explore".into(),
            role: "asp_explorer".into(),
            model_observation: None,
            status: "active".into(),
            expires_at: None,
            metadata_json: r#"{"messageTargetBinding":{"source":"codex.subagent-start","boundRootSessionId":"root-session","childSessionId":"child-2","messageTargetId":"child-2"}}"#.into(),
            now: 14,
        })
        .await
        .expect("register second child instance");

    let replay = registry
        .claim_dispatch(claim("child-2", 15))
        .await
        .expect("claim replay after verified rebind");
    assert_eq!(replay.action, "send");
    assert_eq!(replay.lease.attempt_count, 2);
    assert_eq!(replay.lease.delivery_target_id.as_deref(), Some("child-2"));
    assert_eq!(
        replay.lease.delivery_generation_id.as_deref(),
        Some("child-2")
    );
    let replay_duplicate = registry
        .claim_dispatch(claim("child-2", 16))
        .await
        .expect("poll replay");
    assert_eq!(replay_duplicate.action, "wait");
    assert_eq!(replay_duplicate.lease.attempt_count, 2);

    let terminal = registry
        .complete_dispatch(AgentSessionDispatchCompleteRequest {
            project_id: "project-1",
            root_session_id: "root-session",
            name: "asp-explore",
            dispatch_identity: "dispatch-1",
            command_digest: "sha256:command",
            evidence_ref: "receipt:done",
            now: 17,
        })
        .await
        .expect("record terminal receipt");
    assert_eq!(terminal.status, "terminal");
    assert_eq!(terminal.evidence_ref.as_deref(), Some("receipt:done"));
    let after_terminal = registry
        .claim_dispatch(claim("child-2", 18))
        .await
        .expect("poll terminal dispatch");
    assert_eq!(after_terminal.action, "complete");
    assert_eq!(after_terminal.lease.attempt_count, 2);

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn agent_session_registry_storage_is_turso_owned() {
    let root = temp_root("agent-session-registry");
    let state_root = root.join("agent");
    let db_path = AgentSessionRegistry::db_path_for_state_root(&state_root);

    assert_eq!(
        db_path.file_name().and_then(|name| name.to_str()),
        Some(AGENT_SESSION_REGISTRY_DB_NAME)
    );
    assert_eq!(
        AGENT_SESSION_REGISTRY_DB_NAME,
        "session-registry.current.turso"
    );
    assert!(
        AgentSessionRegistry::open_existing_state_root(&state_root)
            .await
            .expect("open missing session registry")
            .is_none()
    );

    let registry =
        AgentSessionRegistry::open_or_create_state_root(&state_root).expect("create registry");
    let record = registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1".into(),
            root_session_id: "root-session".into(),
            session_id: "child-session".into(),
            parent_session_id: Some("parent-session".into()),
            name: "asp-explore".into(),
            role: "search".into(),
            model_observation: Some(agent_semantic_client_db::AgentSessionModelObservationRef {
                model: "gpt-test",
                source: agent_semantic_client_db::AgentSessionModelObservationSource::CodexSubagentStart,
                observed_at: 10,
                evidence_ref: Some("turn:test"),
            }),
            message_target_id: None,
            status: "active".into(),
            expires_at: Some(1_900_000_000),
            metadata_json: "{\"route\":\"db-owned\"}".into(),
            now: 1_800_000_000,
        })
.await.expect("register session through Turso DB crate");

    assert_eq!(record.root_session_id(), "root-session");
    assert_eq!(record.session_id(), "child-session");
    assert_eq!(record.model.as_deref(), Some("gpt-test"));
    assert_eq!(
        record.model_observation_source.as_deref(),
        Some("codex.subagent-start")
    );
    assert_eq!(record.model_observed_at, Some(10));
    assert_eq!(record.model_evidence_ref.as_deref(), Some("turn:test"));
    assert!(record.is_routable_at(1_800_000_001));
    assert_eq!(
        registry
            .query_sessions(
                "project-1",
                Some(agent_semantic_client_db::AgentSessionRootSessionId::from(
                    "root-session",
                )),
                Some(agent_semantic_client_db::AgentSessionResidentName::from(
                    "asp-explore",
                )),
            )
            .await
            .expect("query session")
            .len(),
        1
    );

    registry
        .register_session(AgentSessionRegisterRequest {
            project_id: "project-1".into(),
            root_session_id: "root-session".into(),
            session_id: "child-session".into(),
            parent_session_id: Some("parent-session".into()),
            name: "asp-explore".into(),
            role: "search".into(),
            model_observation: Some(agent_semantic_client_db::AgentSessionModelObservationRef {
                model: "gpt-stale",
                source: agent_semantic_client_db::AgentSessionModelObservationSource::CodexRollout,
                observed_at: 9,
                evidence_ref: Some("rollout:stale"),
            }),
            message_target_id: None,
            status: "active".into(),
            expires_at: Some(1_900_000_000),
            metadata_json: "{\"route\":\"db-owned\"}".into(),
            now: 1_800_000_001,
        })
        .await
        .expect("ignore stale model observation");
    let retained = registry
        .session_by_id("project-1", "child-session")
        .await
        .expect("lookup model observation")
        .expect("session exists");
    assert_eq!(retained.model.as_deref(), Some("gpt-test"));
    assert_eq!(retained.model_observed_at, Some(10));
    assert_eq!(retained.model_evidence_ref.as_deref(), Some("turn:test"));

    assert!(
        registry
            .record_tool_event(AgentSessionToolEventRequest {
                session_id: "child-session".into(),
                tool_event: "search".into(),
                command: Some("asp search playbook --language rust --rg -n -e source-structure . --tantivy term source-structure".into()),
                evidence_ref: Some("receipt:1".into()),
                now: 1_800_000_010,
            })
            .expect("record tool event")
    );
    let updated = registry
        .session_by_id("project-1", "child-session")
        .await
        .expect("lookup updated session")
        .expect("session exists");
    assert_eq!(updated.last_tool_event.as_deref(), Some("search"));
    assert_eq!(
        updated.last_command.as_deref(),
        Some("asp search playbook --language rust --rg -n -e source-structure . --tantivy term source-structure")
    );
    assert_eq!(updated.last_evidence_ref.as_deref(), Some("receipt:1"));

    let _ = std::fs::remove_dir_all(root);
}

#[path = "db_cases/session_and_source_index.rs"]
mod session_and_source_index;

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let repository = gix::discover(env!("CARGO_MANIFEST_DIR"))
        .expect("discover owner-backed database fixture repository with Gix");
    repository
        .worktree()
        .expect("database fixtures require a non-bare owner checkout")
        .base()
        .join("target/asp-live-project-fixtures")
        .join(format!("asp-client-db-{name}-{nanos}"))
}
