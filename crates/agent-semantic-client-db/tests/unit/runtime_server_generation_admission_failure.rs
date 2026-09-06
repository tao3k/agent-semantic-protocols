// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::sync::Arc;

use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState;
use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus;
use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryEvent;

use super::candidate_identity;

#[tokio::test]
async fn panicking_generation_builder_publishes_a_failed_terminal_receipt() {
    let bus = RuntimeTelemetryBus::new();
    let mut telemetry = bus.receiver;
    let admission = WorkspaceGenerationAdmission::new_with_telemetry_sender(
        Arc::new(
            |_workspace_identity,
             _project_root,
             _candidate,
             _build_mode,
             _changed_paths,
             _provider_target,
             _cancellation| {
                Box::pin(async move {
                    panic!("synthetic generation builder panic");
                })
            },
        ),
        bus.sender,
    );
    let project_root = std::env::temp_dir().join("asp-generation-admission-panic");

    admission
        .admit(
            "workspace-panicking-builder",
            project_root.clone(),
            candidate_identity(),
        )
        .await
        .expect("schedule panicking builder");
    let receipt = admission
        .wait_terminal("workspace-panicking-builder", &project_root)
        .await
        .expect("supervisor publishes a terminal failure");

    assert_eq!(receipt.state, WorkspaceGenerationAdmissionState::Failed);
    assert!(!receipt.accepted);
    assert!(
        receipt
            .error
            .as_deref()
            .is_some_and(|error| error.contains("terminated before a receipt")),
        "panic must be represented by the typed terminal receipt: {receipt:?}"
    );
    receipt.validate().expect("valid panic failure receipt");
    let terminal_event = loop {
        match telemetry.recv().await.expect("generation lifecycle event") {
            RuntimeTelemetryEvent::Lifecycle(event)
                if event.transition == "generation-terminal" =>
            {
                break event;
            }
            RuntimeTelemetryEvent::Lifecycle(_)
            | RuntimeTelemetryEvent::SearchIncident(_)
            | RuntimeTelemetryEvent::Performance(_) => {}
        }
    };
    assert_eq!(terminal_event.state, "failed");
    assert_eq!(
        terminal_event.workspace_identity.as_deref(),
        Some("workspace-panicking-builder")
    );
    admission.shutdown().await.expect("shutdown admission");
}

#[tokio::test]
async fn shutdown_cancels_tracked_generation_builds() {
    let started = Arc::new(tokio::sync::Notify::new());
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let started = Arc::clone(&started);
        move |_workspace_identity,
              _project_root,
              _candidate,
              _build_mode,
              _changed_paths,
              _provider_target,
              _cancellation| {
            let started = Arc::clone(&started);
            Box::pin(async move {
                started.notify_one();
                std::future::pending::<
                    Result<
                        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildCompletion,
                        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildFailure,
                    >,
                >()
                .await
            })
        }
    }));
    admission
        .admit(
            "workspace-shutdown",
            std::env::temp_dir().join("asp-generation-admission-shutdown"),
            candidate_identity(),
        )
        .await
        .expect("schedule tracked builder");
    started.notified().await;

    assert_eq!(
        admission.shutdown().await.expect("cancel tracked builder"),
        1
    );
}

#[tokio::test]
async fn dropping_the_request_handle_does_not_cancel_the_runtime_owned_build() {
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let admission = Arc::new(WorkspaceGenerationAdmission::new(Arc::new({
        let started = Arc::clone(&started);
        let release = Arc::clone(&release);
        move |_workspace_identity,
              _project_root,
              _candidate,
              _build_mode,
              _changed_paths,
              _provider_target,
              _cancellation| {
            let started = Arc::clone(&started);
            let release = Arc::clone(&release);
            Box::pin(async move {
                started.notify_one();
                release.notified().await;
                panic!("synthetic terminal build after request drop");
            })
        }
    })));
    let workspace_identity = "workspace-request-drop";
    let project_root = std::env::temp_dir().join("asp-generation-admission-request-drop");
    let request_handle = Arc::clone(&admission);
    let request_project_root = project_root.clone();
    let request = tokio::spawn(async move {
        request_handle
            .admit(
                workspace_identity,
                request_project_root,
                candidate_identity(),
            )
            .await
    });

    let submitted = request
        .await
        .expect("request task")
        .expect("submit Runtime-owned build");
    assert_eq!(submitted.state, WorkspaceGenerationAdmissionState::Queued);
    started.notified().await;
    release.notify_one();

    let terminal = admission
        .wait_terminal(workspace_identity, &project_root)
        .await
        .expect("observe terminal build after request drop");
    assert_eq!(terminal.state, WorkspaceGenerationAdmissionState::Failed);
    assert!(
        terminal
            .error
            .as_deref()
            .is_some_and(|error| error.contains("terminated before a receipt"))
    );
    assert!(
        terminal
            .error
            .as_deref()
            .is_none_or(|error| !error.contains("cancelled"))
    );
    terminal.validate().expect("valid terminal receipt");
    admission.shutdown().await.expect("shutdown Runtime owner");
}

#[test]
fn dispatcher_abort_paths_retain_entry_bound_terminal_authority() {
    let source = include_str!("../../src/runtime_server_admission_dispatcher.rs");
    assert!(source.contains("struct AdmissionBuildEnvelope"));
    assert!(source.contains("start: Option<AdmissionBuildStartFuture>"));
    assert!(source.contains("terminalize: Option<AdmissionBuildFuture>"));
    assert!(source.contains("impl Drop for AdmissionBuildEnvelope"));
    assert!(source.contains("join_next_with_id"));
    assert!(source.contains("terminalizers.remove(&error.id())"));
    assert!(source.contains("terminalizers.terminalize_remaining().await"));
    assert!(source.contains("impl Drop for AdmissionBuildTerminalizers"));
    assert!(!source.contains("Spawn(AdmissionBuildFuture)"));
    assert!(!source.contains("Some(_) = builds.join_next()"));
}
