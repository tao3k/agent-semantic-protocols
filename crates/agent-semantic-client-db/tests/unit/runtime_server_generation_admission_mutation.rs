use std::sync::Arc;

use super::{
    Mutex, RuntimeWorkspaceAdmissionCatalog, RuntimeWorkspaceAdmissionCatalogEntry,
    WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID, WorkspaceGenerationAdmission,
    WorkspaceGenerationAdmissionState, WorkspaceGenerationBuildFailure,
    WorkspaceGenerationBuildMode, WorkspaceGenerationFailureStage,
    WorkspaceGenerationMutationAdmissionReceipt, candidate_identity, completed_generation,
    ready_receipt,
};

#[test]
fn mutation_rebuild_does_not_attempt_to_restore_the_superseded_materialization() {
    assert!(WorkspaceGenerationBuildMode::RestoreOnly.attempts_durable_restore());
    assert!(WorkspaceGenerationBuildMode::RestoreOrBuild.attempts_durable_restore());
    assert!(!WorkspaceGenerationBuildMode::RebuildAfterMutation.attempts_durable_restore());
}

#[test]
fn mutation_receipt_rejects_count_drift_and_duplicate_workspace_identity() {
    let count_drift = WorkspaceGenerationMutationAdmissionReceipt {
        schema_id: WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        mutation_id: "mutation-count-drift".to_owned(),
        changed_path_count: 2,
        affected_workspace_count: 1,
        receipts: vec![
            ready_receipt("workspace-parent"),
            ready_receipt("workspace-nested"),
        ],
    };
    assert_eq!(
        count_drift.validate().expect_err("count drift must fail"),
        "workspace mutation admission receipt count does not match receipts"
    );

    let duplicate = WorkspaceGenerationMutationAdmissionReceipt {
        schema_id: WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        mutation_id: "mutation-duplicate-workspace".to_owned(),
        changed_path_count: 2,
        affected_workspace_count: 2,
        receipts: vec![
            ready_receipt("workspace-parent"),
            ready_receipt("workspace-parent"),
        ],
    };
    assert_eq!(
        duplicate
            .validate()
            .expect_err("duplicate workspace identity must fail"),
        "workspace mutation admission receipt repeats a workspace identity"
    );
}

#[tokio::test]
async fn mutation_admission_rejects_non_normalized_paths_and_workspace_identity_drift() {
    let temp = tempfile::tempdir().expect("temporary resident catalog");
    let parent_root = temp.path().join("repository");
    let conflicting_root = temp.path().join("other-checkout");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(temp.path().join("catalog.json"))
        .await
        .expect("load resident workspace catalog");
    catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-parent".to_owned(),
            project_root: parent_root.clone(),
        })
        .await
        .expect("record parent workspace");
    let admission =
        WorkspaceGenerationAdmission::new(Arc::new(|_, _, _, _, _changed_paths, _cancellation| {
            Box::pin(async {
                panic!("invalid mutation admission must not start a generation build")
            })
        }))
        .with_catalog(catalog);

    let path_error = admission
        .admit_observed_mutation(
            "mutation-path-normalization",
            "workspace-parent",
            parent_root.clone(),
            vec![std::path::PathBuf::from("src/../src/lib.rs")],
            candidate_identity(),
        )
        .await
        .expect_err("parent directory component must fail closed");
    assert!(path_error.contains("path must be normalized"));

    let identity_error = admission
        .admit_observed_mutation(
            "mutation-identity-drift",
            "workspace-parent",
            conflicting_root.clone(),
            vec![conflicting_root.join("src/lib.rs")],
            candidate_identity(),
        )
        .await
        .expect_err("workspace identity drift must fail closed");
    assert!(identity_error.contains("identity already owns a different resident root"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changed_paths_fan_out_to_each_workspace_resident_without_git_rediscovery() {
    let temp = tempfile::tempdir().expect("temporary resident catalog");
    let parent_root = temp.path().join("repository");
    let nested_root = parent_root.join("languages/rust-lang-project-harness");
    tokio::fs::create_dir_all(&nested_root)
        .await
        .expect("nested workspace root");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(temp.path().join("catalog.json"))
        .await
        .expect("load resident workspace catalog");
    catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-parent".to_owned(),
            project_root: parent_root.clone(),
        })
        .await
        .expect("record parent workspace");
    catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-nested".to_owned(),
            project_root: nested_root.clone(),
        })
        .await
        .expect("record nested workspace");

    let builds = Arc::new(Mutex::new(Vec::new()));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let builds = Arc::clone(&builds);
        move |workspace_identity,
              project_root,
              candidate,
              _build_mode,
              changed_paths,
              _cancellation| {
            let builds = Arc::clone(&builds);
            Box::pin(async move {
                builds.lock().await.push((
                    workspace_identity,
                    project_root,
                    (*changed_paths).clone(),
                ));
                completed_generation(candidate)
            })
        }
    }))
    .with_catalog(catalog);
    for (workspace_identity, workspace_root) in [
        ("workspace-parent", &parent_root),
        ("workspace-nested", &nested_root),
    ] {
        admission
            .admit(
                workspace_identity,
                workspace_root.clone(),
                candidate_identity(),
            )
            .await
            .expect("admit resident candidate evidence");
        admission
            .wait_terminal(workspace_identity, workspace_root)
            .await
            .expect("resident candidate evidence ready");
    }
    builds.lock().await.clear();

    let receipt = admission
        .admit_observed_mutation(
            "mutation-parent-and-nested",
            "workspace-parent",
            parent_root.clone(),
            vec![
                parent_root.join("README.md"),
                nested_root.join("src/exact_source.rs"),
            ],
            candidate_identity(),
        )
        .await
        .expect("fan out changed paths from resident evidence");
    receipt.validate().expect("valid mutation receipt");
    assert_eq!(receipt.changed_path_count, 2);
    assert_eq!(receipt.affected_workspace_count, 2);
    assert_eq!(
        receipt
            .receipts
            .iter()
            .map(|receipt| receipt.workspace_identity.as_str())
            .collect::<Vec<_>>(),
        vec!["workspace-nested", "workspace-parent"]
    );
    for (workspace_identity, workspace_root) in [
        ("workspace-parent", &parent_root),
        ("workspace-nested", &nested_root),
    ] {
        admission
            .wait_terminal(workspace_identity, workspace_root)
            .await
            .expect("workspace mutation terminal state");
    }
    let mut builds = builds.lock().await.clone();
    builds.sort();
    assert_eq!(
        builds,
        vec![
            (
                "workspace-nested".to_owned(),
                nested_root.clone(),
                std::collections::BTreeSet::from([nested_root.join("src/exact_source.rs")]),
            ),
            (
                "workspace-parent".to_owned(),
                parent_root.clone(),
                std::collections::BTreeSet::from([
                    parent_root.join("README.md"),
                    nested_root.join("src/exact_source.rs"),
                ]),
            ),
        ]
    );
    assert!(builds.iter().all(|(_, _, changed_paths)| {
        !changed_paths.contains(&parent_root.join("src/untouched.rs"))
            && !changed_paths.contains(&nested_root.join("src/untouched.rs"))
    }));

    admission
        .shutdown()
        .await
        .expect("drain changed-path admission lanes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn distinct_mutation_queued_during_build_runs_as_the_next_generation_attempt() {
    let build_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let release = Arc::clone(&release);
        move |_, _, candidate, _, _changed_paths, _cancellation| {
            let build_count = Arc::clone(&build_count);
            let release = Arc::clone(&release);
            Box::pin(async move {
                build_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                release
                    .acquire_owned()
                    .await
                    .map_err(|_| {
                        WorkspaceGenerationBuildFailure::new(
                            WorkspaceGenerationFailureStage::GenerationBuilder,
                            "mutation queue release closed",
                        )
                    })?
                    .forget();
                completed_generation(candidate)
            })
        }
    }));
    let root = std::env::temp_dir().join("asp-mutation-queue-workspace");
    let changed_path = root.join("src/lib.rs");

    let first = admission
        .admit_observed_mutation(
            "mutation-1",
            "workspace-mutation-queue",
            root.clone(),
            vec![changed_path.clone()],
            candidate_identity(),
        )
        .await
        .expect("admit first mutation");
    assert_eq!(first.receipts[0].attempt, 1);
    assert!(first.receipts[0].accepted);

    let second = admission
        .admit_observed_mutation(
            "mutation-2",
            "workspace-mutation-queue",
            root.clone(),
            vec![changed_path.clone()],
            candidate_identity(),
        )
        .await
        .expect("queue successor mutation");
    assert_eq!(second.receipts[0].attempt, 2);
    assert!(second.receipts[0].accepted);

    let third = admission
        .admit_observed_mutation(
            "mutation-3",
            "workspace-mutation-queue",
            root.clone(),
            vec![changed_path.clone()],
            candidate_identity(),
        )
        .await
        .expect("queue second successor mutation");
    assert_eq!(third.receipts[0].attempt, 3);
    assert!(third.receipts[0].accepted);

    let duplicate = admission
        .admit_observed_mutation(
            "mutation-2",
            "workspace-mutation-queue",
            root.clone(),
            vec![changed_path.clone()],
            candidate_identity(),
        )
        .await
        .expect("observe queued mutation");
    assert_eq!(duplicate.receipts[0].attempt, 2);
    assert!(!duplicate.receipts[0].accepted);

    release.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        while build_count.load(std::sync::atomic::Ordering::Relaxed) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("successor generation attempt must start");
    release.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        while build_count.load(std::sync::atomic::Ordering::Relaxed) < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("second successor generation attempt must start");
    release.add_permits(1);
    let terminal = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        admission.wait_terminal("workspace-mutation-queue", &root),
    )
    .await
    .expect("queued successor must reach terminal state within 100ms")
    .expect("queued successor must produce a terminal receipt");
    assert_eq!(terminal.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(terminal.attempt, 3);
    assert_eq!(build_count.load(std::sync::atomic::Ordering::Relaxed), 3);

    let completed_duplicate = admission
        .admit_observed_mutation(
            "mutation-3",
            "workspace-mutation-queue",
            root.clone(),
            vec![changed_path],
            candidate_identity(),
        )
        .await
        .expect("coalesce duplicate of the completed mutation");
    assert_eq!(completed_duplicate.receipts[0].attempt, 3);
    assert!(!completed_duplicate.receipts[0].accepted);
    assert_eq!(build_count.load(std::sync::atomic::Ordering::Relaxed), 3);

    tokio::time::timeout(std::time::Duration::from_millis(100), admission.shutdown())
        .await
        .expect("queued mutation generation lane must drain within 100ms")
        .expect("drain queued mutation generation lane");
}
