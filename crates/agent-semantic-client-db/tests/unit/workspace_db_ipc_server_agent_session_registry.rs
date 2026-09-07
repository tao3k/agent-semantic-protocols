// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::workspace_db_ipc::{
    AgentHostExecutionObservationIpc, AgentHostLifecycleEventIpc, AgentHostLifecycleEventKind,
    AgentSessionRegistryIpcResult,
};

#[tokio::test]
async fn host_stop_resume_and_achieve_preserve_one_durable_generation() {
    let root = tempfile::tempdir().expect("Host lifecycle registry tempdir");
    let registry =
        crate::AgentSessionRegistry::open_or_create_state_root(root.path().join("state"))
            .expect("open Host lifecycle registry");
    let start = AgentHostLifecycleEventIpc {
        host_event_id: "host-event-1".to_owned().into(),
        host_event_sequence: 1,
        namespace_id: "asp-testing".to_owned().into(),
        kind: AgentHostLifecycleEventKind::Started,
        platform: "codex".to_owned(),
        project_id: "workspace-1".to_owned().into(),
        root_session_id: "root-1".to_owned().into(),
        parent_session_id: "parent-1".to_owned().into(),
        child_session_id: "child-1".to_owned().into(),
        host_task_name: "asp_explorer".to_owned(),
        platform_host_agent_name: "asp_explorer".to_owned(),
        route_key: "asp_explorer".to_owned().into(),
        profile_id: "agents/asp_explorer.toml".to_owned().into(),
        role: "explore".to_owned(),
        model: "gpt-test".to_owned(),
        model_digest: "blake3-256:model".to_owned(),
        profile_digest: "blake3-256:profile".to_owned(),
        sandbox_mode: crate::workspace_db_ipc::AgentHostSandboxMode::ReadOnly,
        session_lifetime: "resident".to_owned(),
        payload_digest: "blake3-256:payload".to_owned(),
        transcript_path: Some("/tmp/child.jsonl".to_owned().into()),
        observed_at: 10,
    };

    let registered = super::record_host_lifecycle_event(&registry, start.clone())
        .await
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
            host_event_id: "host-event-2".to_owned().into(),
            host_event_sequence: 2,
            kind: AgentHostLifecycleEventKind::Stopped,
            observed_at: 20,
            ..start.clone()
        },
    )
    .await
    .expect("record Host stop event");
    assert_eq!(
        stopped,
        AgentSessionRegistryIpcResult::Changed { changed: true }
    );
    let stopped = registry
        .session_by_id("workspace-1", "child-1")
        .await
        .expect("query stopped generation")
        .expect("stopped generation exists");
    assert_eq!(stopped.status.as_str(), "stopped");
    assert_eq!(stopped.physical_generation, 1);

    super::record_host_lifecycle_event(
        &registry,
        AgentHostLifecycleEventIpc {
            host_event_id: "host-event-3".to_owned().into(),
            host_event_sequence: 3,
            kind: AgentHostLifecycleEventKind::Resumed,
            observed_at: 30,
            ..start.clone()
        },
    )
    .await
    .expect("resume exact durable namespace");
    let resumed = registry
        .session_by_id("workspace-1", "child-1")
        .await
        .expect("query resumed generation")
        .expect("resumed generation exists");
    assert_eq!(resumed.status.as_str(), "active");
    assert_eq!(resumed.physical_generation, 1);

    super::record_host_lifecycle_event(
        &registry,
        AgentHostLifecycleEventIpc {
            host_event_id: "host-event-4".to_owned().into(),
            host_event_sequence: 4,
            kind: AgentHostLifecycleEventKind::Achieved,
            observed_at: 40,
            ..start
        },
    )
    .await
    .expect("achieve exact durable namespace");
    let achieved = registry
        .session_by_id("workspace-1", "child-1")
        .await
        .expect("query achieved generation")
        .expect("achieved generation exists");
    assert_eq!(achieved.status.as_str(), "achieved");
    assert_eq!(achieved.physical_generation, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn host_execution_observation_resumes_exact_namespace_without_new_generation() {
    let root = tempfile::tempdir().expect("Host execution registry tempdir");
    let registry = std::sync::Arc::new(
        crate::AgentSessionRegistry::open_or_create_state_root(root.path().join("state"))
            .expect("open Host execution registry"),
    );
    let start = AgentHostLifecycleEventIpc {
        host_event_id: "host-event-1".to_owned().into(),
        host_event_sequence: 1,
        namespace_id: "asp-explorer".to_owned().into(),
        kind: AgentHostLifecycleEventKind::Started,
        platform: "codex".to_owned(),
        project_id: "workspace-1".to_owned().into(),
        root_session_id: "root-1".to_owned().into(),
        parent_session_id: "root-1".to_owned().into(),
        child_session_id: "child-1".to_owned().into(),
        host_task_name: "asp_explorer".to_owned(),
        platform_host_agent_name: "asp_explorer".to_owned(),
        route_key: "asp_explorer".to_owned().into(),
        profile_id: "agents/asp_explorer.toml".to_owned().into(),
        role: "explore".to_owned(),
        model: "gpt-test".to_owned(),
        model_digest: "blake3-256:model".to_owned(),
        profile_digest: "blake3-256:profile".to_owned(),
        sandbox_mode: crate::workspace_db_ipc::AgentHostSandboxMode::ReadOnly,
        session_lifetime: "resident".to_owned(),
        payload_digest: "blake3-256:payload".to_owned(),
        transcript_path: Some("/tmp/child.jsonl".to_owned().into()),
        observed_at: 10,
    };
    super::record_host_lifecycle_event(&registry, start.clone())
        .await
        .expect("start namespace");
    super::record_host_lifecycle_event(
        &registry,
        AgentHostLifecycleEventIpc {
            host_event_id: "host-event-2".to_owned().into(),
            host_event_sequence: 2,
            kind: AgentHostLifecycleEventKind::Stopped,
            observed_at: 20,
            ..start
        },
    )
    .await
    .expect("stop namespace");

    let tasks = (0..64).map(|index| {
        let registry = std::sync::Arc::clone(&registry);
        tokio::spawn(async move {
            registry
                .record_host_execution_observation_local(&AgentHostExecutionObservationIpc {
                    observation_id: format!("tool-{index}"),
                    project_id: "workspace-1".to_owned().into(),
                    root_session_id: "root-1".to_owned().into(),
                    child_session_id: "child-1".to_owned().into(),
                    platform_host_agent_name: "asp_explorer".to_owned(),
                    transcript_path: "/tmp/child.jsonl".to_owned(),
                    observed_at: 30 + index,
                })
                .await
        })
    });
    for task in tasks {
        assert!(
            task.await
                .expect("observation task")
                .expect("observe child")
        );
    }

    let resumed = registry
        .query_sessions_local(
            "workspace-1".to_owned(),
            Some("root-1".into()),
            Some("asp_explorer".into()),
        )
        .await
        .expect("query resumed namespace")
        .pop()
        .expect("resumed namespace exists");
    assert_eq!(resumed.status(), "active");
    assert_eq!(resumed.physical_generation, 1);
    assert_eq!(resumed.message_target_id(), Some("child-1"));
    assert!(resumed.metadata_json().contains("lastExecutionObservation"));
    assert!(resumed.metadata_json().contains("\"routable\":true"));
}

#[tokio::test]
async fn host_execution_observation_never_creates_or_rebinds_a_namespace() {
    let root = tempfile::tempdir().expect("Host execution registry tempdir");
    let registry =
        crate::AgentSessionRegistry::open_or_create_state_root(root.path().join("state"))
            .expect("open Host execution registry");
    let observation = AgentHostExecutionObservationIpc {
        observation_id: "tool-1".to_owned(),
        project_id: "workspace-1".to_owned().into(),
        root_session_id: "root-1".to_owned().into(),
        child_session_id: "child-1".to_owned().into(),
        platform_host_agent_name: "asp_explorer".to_owned(),
        transcript_path: "/tmp/child.jsonl".to_owned(),
        observed_at: 30,
    };

    let absent = registry
        .record_host_execution_observation_local(&observation)
        .await
        .expect("pre-lifecycle execution evidence must be deferred");
    assert!(
        !absent,
        "an execution observation must not create a namespace"
    );
    assert!(
        registry
            .query_sessions_local("workspace-1".to_owned(), None, None)
            .await
            .expect("query empty registry")
            .is_empty()
    );

    let start = AgentHostLifecycleEventIpc {
        host_event_id: "host-event-1".to_owned().into(),
        host_event_sequence: 1,
        namespace_id: "asp-explorer".to_owned().into(),
        kind: AgentHostLifecycleEventKind::Started,
        platform: "codex".to_owned(),
        project_id: "workspace-1".to_owned().into(),
        root_session_id: "root-1".to_owned().into(),
        parent_session_id: "root-1".to_owned().into(),
        child_session_id: "child-1".to_owned().into(),
        host_task_name: "asp_explorer".to_owned(),
        platform_host_agent_name: "asp_explorer".to_owned(),
        route_key: "asp_explorer".to_owned().into(),
        profile_id: "agents/asp_explorer.toml".to_owned().into(),
        role: "explore".to_owned(),
        model: "gpt-test".to_owned(),
        model_digest: "blake3-256:model".to_owned(),
        profile_digest: "blake3-256:profile".to_owned(),
        sandbox_mode: crate::workspace_db_ipc::AgentHostSandboxMode::ReadOnly,
        session_lifetime: "resident".to_owned(),
        payload_digest: "blake3-256:payload".to_owned(),
        transcript_path: Some("/tmp/child.jsonl".to_owned().into()),
        observed_at: 10,
    };
    super::record_host_lifecycle_event(&registry, start.clone())
        .await
        .expect("start namespace");

    let wrong_child = registry
        .record_host_execution_observation_local(&AgentHostExecutionObservationIpc {
            child_session_id: "child-2".to_owned().into(),
            ..observation.clone()
        })
        .await
        .expect_err("an execution observation must not rebind another child");
    assert!(
        wrong_child.starts_with("host-execution-observation-identity-mismatch:"),
        "{wrong_child}"
    );

    super::record_host_lifecycle_event(
        &registry,
        AgentHostLifecycleEventIpc {
            host_event_id: "host-event-2".to_owned().into(),
            host_event_sequence: 2,
            kind: AgentHostLifecycleEventKind::Achieved,
            observed_at: 40,
            ..start
        },
    )
    .await
    .expect("achieve namespace");
    let achieved = registry
        .record_host_execution_observation_local(&observation)
        .await
        .expect_err("execution evidence must not revive an achieved namespace");
    assert!(!achieved.is_empty());
    let terminal = registry
        .session_by_id("workspace-1", "child-1")
        .await
        .expect("query achieved namespace")
        .expect("achieved namespace exists");
    assert_eq!(terminal.status, "achieved");
    assert_eq!(terminal.physical_generation, 1);
}

#[tokio::test]
async fn existing_namespace_rejects_second_start_and_requires_native_resume() {
    let root = tempfile::tempdir().expect("Host lifecycle registry tempdir");
    let registry =
        crate::AgentSessionRegistry::open_or_create_state_root(root.path().join("state"))
            .expect("open Host lifecycle registry");
    let start = AgentHostLifecycleEventIpc {
        host_event_id: "host-event-1".to_owned().into(),
        host_event_sequence: 1,
        namespace_id: "asp-testing".to_owned().into(),
        kind: AgentHostLifecycleEventKind::Started,
        platform: "codex".to_owned(),
        project_id: "workspace-1".to_owned().into(),
        root_session_id: "root-1".to_owned().into(),
        parent_session_id: "root-1".to_owned().into(),
        child_session_id: "child-1".to_owned().into(),
        host_task_name: "asp_explorer".to_owned(),
        platform_host_agent_name: "asp_explorer".to_owned(),
        route_key: "asp_explorer".to_owned().into(),
        profile_id: "agents/asp_explorer.toml".to_owned().into(),
        role: "explore".to_owned(),
        model: "gpt-test".to_owned(),
        model_digest: "blake3-256:model".to_owned(),
        profile_digest: "blake3-256:profile".to_owned(),
        sandbox_mode: crate::workspace_db_ipc::AgentHostSandboxMode::ReadOnly,
        session_lifetime: "resident".to_owned(),
        payload_digest: "blake3-256:payload".to_owned(),
        transcript_path: Some("/tmp/child.jsonl".to_owned().into()),
        observed_at: 10,
    };
    super::record_host_lifecycle_event(&registry, start.clone())
        .await
        .expect("first resident registration");

    let duplicate = AgentHostLifecycleEventIpc {
        child_session_id: "child-2".to_owned().into(),
        observed_at: 11,
        ..start.clone()
    };
    let duplicate_error = super::record_host_lifecycle_event(&registry, duplicate)
        .await
        .expect_err("a live resident must be called or resumed before another spawn");
    assert!(duplicate_error.starts_with("existing-host-namespace-identity-mismatch:"));

    super::record_host_lifecycle_event(
        &registry,
        AgentHostLifecycleEventIpc {
            host_event_id: "host-event-2".to_owned().into(),
            host_event_sequence: 2,
            kind: AgentHostLifecycleEventKind::Stopped,
            observed_at: 12,
            ..start.clone()
        },
    )
    .await
    .expect("terminalize exact resident");
    let reuse_error = super::record_host_lifecycle_event(
        &registry,
        AgentHostLifecycleEventIpc {
            host_event_id: "host-event-3".to_owned().into(),
            host_event_sequence: 3,
            payload_digest: "blake3-256:restart-attempt".to_owned(),
            observed_at: 13,
            ..start.clone()
        },
    )
    .await
    .expect_err("existing child identity must use resume");
    assert!(reuse_error.starts_with("existing-host-namespace-requires-resume:"));

    super::record_host_lifecycle_event(
        &registry,
        AgentHostLifecycleEventIpc {
            host_event_id: "host-event-4".to_owned().into(),
            host_event_sequence: 4,
            kind: AgentHostLifecycleEventKind::Resumed,
            observed_at: 14,
            ..start
        },
    )
    .await
    .expect("resume existing child identity");
    let resumed = registry
        .session_by_id("workspace-1", "child-1")
        .await
        .expect("query resumed namespace")
        .expect("resumed namespace exists");
    assert_eq!(resumed.status.as_str(), "active");
    assert_eq!(resumed.physical_generation, 1);
}
