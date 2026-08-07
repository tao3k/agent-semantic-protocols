use std::sync::Arc;

use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionState,
};

use super::candidate_identity;

#[tokio::test]
async fn panicking_generation_builder_publishes_a_failed_terminal_receipt() {
    let admission = WorkspaceGenerationAdmission::new(Arc::new(
        |_workspace_identity,
         _project_root,
         _candidate,
         _build_mode,
         _changed_paths,
         _cancellation| {
            Box::pin(async move {
                panic!("synthetic generation builder panic");
            })
        },
    ));
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
