use super::submit_mutation;

fn candidate() -> crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
    crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
        candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
            algorithm: "blake3-worktree-state-v1".to_owned(),
            digest: "blake3:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_owned(),
            authorities: vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex],
        },
        policy_overlay_digest:
            "blake3:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_owned(),
    }
}

fn generation(
    workspace_identity: &str,
    project_root: &std::path::Path,
    bytes: &[u8],
) -> crate::runtime_server_workspace::WorkspaceMemoryGeneration {
    let content_digest = format!("blake3-256:{}", blake3::hash(bytes).to_hex());
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [("src/lib.rs", content_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"non-git-mutation-provider").to_hex()
        ),
    );
    crate::runtime_server_workspace::WorkspaceMemoryGeneration::try_from_build(
        crate::runtime_server_workspace::WorkspaceGenerationBuild {
    projection_capability: agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::single_selector("blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(), "rust://fixture/src/lib.rs#item/function/fixture".to_owned(), "src/lib.rs".to_owned(), std::collections::BTreeSet::from([agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionMode::Source])).expect("test projection capability manifest"),
            relations: Vec::new(),
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.display().to_string(),
            active_epoch: 1,
            workspace_snapshot,
            source_snapshot,
            module_graph_digest: format!(
                "blake3-256:{}",
                blake3::hash(b"non-git-mutation-module-graph").to_hex()
            ),
            project_resolutions: Vec::new(),
            owners: vec![crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
                owner_path: "src/lib.rs".to_owned(),
                content_digest,
                bytes: bytes.to_vec(),
                selectors: Vec::new(),
            }],
        },
    )
    .expect("typed non-Git generation")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn admitted_mutation_rediscovers_server_owned_non_git_candidate() {
    let temp = tempfile::tempdir().expect("temporary non-Git Runtime root");
    let project_root = temp.path().join("project");
    tokio::fs::create_dir_all(project_root.join("src"))
        .await
        .expect("create non-Git source root");
    tokio::fs::write(project_root.join("src/lib.rs"), b"fn changed() {}\n")
        .await
        .expect("write changed non-Git owner");
    let workspace_identity = "workspace-non-git-mutation";
    let registry = std::sync::Arc::new(
        crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            temp.path().join("runtime"),
        )
        .expect("create non-Git workspace registry"),
    );
    registry
        .publish(
            "publish-non-git-base",
            crate::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity, &project_root, b"fn previous() {}\n"),
        )
        .await
        .expect("publish non-Git base generation");
    let build_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let build_changed = std::sync::Arc::new(tokio::sync::Notify::new());
    let observed_candidates = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = std::sync::Arc::new(
        crate::runtime_server_admission::WorkspaceGenerationAdmission::new(std::sync::Arc::new({
            let build_count = std::sync::Arc::clone(&build_count);
            let build_changed = std::sync::Arc::clone(&build_changed);
            let observed_candidates = std::sync::Arc::clone(&observed_candidates);
            move |_workspace_identity,
                  _project_root,
                  candidate,
                  _mode,
                  _paths,
                  _provider_target,
                  _cancellation| {
                let build_count = std::sync::Arc::clone(&build_count);
                let build_changed = std::sync::Arc::clone(&build_changed);
                let observed_candidates = std::sync::Arc::clone(&observed_candidates);
                Box::pin(async move {
                    build_count.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                    observed_candidates
                        .lock()
                        .expect("candidate observations")
                        .push(candidate.clone());
                    build_changed.notify_waiters();
                    crate::runtime_server_admission::WorkspaceGenerationBuildCompletion::new(
                            candidate,
                            crate::runtime_server_admission::WorkspaceGenerationCommitReceipt {
    projection_capability: crate::fixture::ready_projection_capability_fixture(
        "workspace-test",
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111",
        "blake3-256:2222222222222222222222222222222222222222222222222222222222222222",
        1,
    ),
                                active_epoch: 1,
                                generation_digest: "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
                                source_root_digest: "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
                            },
                        )
                        .map_err(|error| {
                            crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                                crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                                error,
                            )
                        })
                })
            }
        })),
    );
    admission
        .admit(workspace_identity, project_root.clone(), candidate())
        .await
        .expect("admit non-Git base candidate");
    admission
        .wait_terminal(workspace_identity, &project_root)
        .await
        .expect("base candidate ready");

    let result = submit_mutation(
        std::sync::Arc::clone(&registry),
        Some(std::sync::Arc::clone(&admission)),
        workspace_identity.to_owned(),
        "non-git-owner-change".to_owned(),
        project_root.display().to_string(),
        vec![project_root.join("src/lib.rs").display().to_string()],
    );
    assert!(matches!(
        result,
        crate::workspace_db_ipc::WorkspaceDbIpcResult::RuntimeGenerationMutationSubmission { .. }
    ));
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while build_count.load(std::sync::atomic::Ordering::Acquire) < 2 {
            build_changed.notified().await;
        }
    })
    .await
    .expect("mutation build completed without Git discovery");
    assert_eq!(
        admission
            .wait_terminal(workspace_identity, &project_root)
            .await
            .expect("non-Git mutation terminal receipt")
            .state,
        crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
    );
    let observed_candidates = observed_candidates.lock().expect("candidate observations");
    assert_eq!(observed_candidates.len(), 2);
    assert_ne!(
        observed_candidates[0].candidate_generation.digest,
        observed_candidates[1].candidate_generation.digest,
        "each mutation must rediscover current workspace candidate identity"
    );
    assert_eq!(
        observed_candidates[1].candidate_generation.authorities,
        vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::ServerResident]
    );
    drop(observed_candidates);

    admission.shutdown().await.expect("shutdown admission");
    registry.shutdown().await.expect("shutdown registry");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resident_reads_fail_closed_while_a_new_generation_is_building() {
    let temp = tempfile::tempdir().expect("temporary generation gate root");
    let project_root = temp.path().join("project");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("project root");
    let workspace_identity = "workspace-terminal-read-gate";
    let build_index = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mutation_started = std::sync::Arc::new(tokio::sync::Notify::new());
    let release_mutation = std::sync::Arc::new(tokio::sync::Notify::new());
    let admission = crate::runtime_server_admission::WorkspaceGenerationAdmission::new(
        std::sync::Arc::new({
            let build_index = std::sync::Arc::clone(&build_index);
            let mutation_started = std::sync::Arc::clone(&mutation_started);
            let release_mutation = std::sync::Arc::clone(&release_mutation);
            move |_workspace, _root, candidate, _mode, _paths, _provider_target, _cancellation| {
                let index = build_index.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                let mutation_started = std::sync::Arc::clone(&mutation_started);
                let release_mutation = std::sync::Arc::clone(&release_mutation);
                Box::pin(async move {
                    if index > 0 {
                        mutation_started.notify_one();
                        release_mutation.notified().await;
                    }
                    crate::runtime_server_admission::WorkspaceGenerationBuildCompletion::new(
                        candidate,
                        crate::runtime_server_admission::WorkspaceGenerationCommitReceipt {
    projection_capability: crate::fixture::ready_projection_capability_fixture(
        "workspace-test",
        format!("blake3-256:{:064x}", index + 1),
        format!("blake3-256:{:064x}", index + 2),
        index as u64 + 1,
    ),
                            active_epoch: index as u64 + 1,
                            generation_digest: format!("blake3-256:{:064x}", index + 1),
                            source_root_digest: format!("blake3-256:{:064x}", index + 2),
                        },
                    )
                    .map_err(|error| {
                        crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                            error,
                        )
                    })
                })
            }
        }),
    );
    let registry = crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
        temp.path().join("runtime-read-guard"),
    )
    .expect("create runtime workspace registry");
    admission
        .admit(workspace_identity, project_root.clone(), candidate())
        .await
        .expect("admit base generation");
    admission
        .wait_terminal(workspace_identity, &project_root)
        .await
        .expect("base generation ready");
    admission
        .admit_observed_mutation(
            "mutation-read-gate",
            workspace_identity,
            project_root.clone(),
            vec![project_root.join("src/lib.rs")],
            candidate(),
        )
        .await
        .expect("queue mutation generation");
    mutation_started.notified().await;

    let error = crate::workspace_db_ipc_server::generation::require_terminal_generation_for_read(
        &registry,
        Some(&admission),
        workspace_identity,
        &project_root,
    )
    .expect_err("Building generation must hide the prior exact/search segment");
    assert!(error.contains("reasonKind=active-workspace-generation-required"));

    release_mutation.notify_one();
    admission
        .wait_terminal(workspace_identity, &project_root)
        .await
        .expect("mutation generation ready");
    crate::workspace_db_ipc_server::generation::require_terminal_generation_for_read(
        &registry,
        Some(&admission),
        workspace_identity,
        &project_root,
    )
    .expect("Ready committed generation admits resident reads");
    admission.shutdown().await.expect("shutdown admission");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_ipc_mutation_bootstraps_a_server_owned_generation() {
    let temp = tempfile::tempdir().expect("temporary first-mutation Runtime root");
    let project_root = temp.path().join("project");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("create first-mutation source root");
    let workspace_identity = "workspace-first-ipc-mutation";
    let registry = std::sync::Arc::new(
        crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            temp.path().join("runtime"),
        )
        .expect("create first-mutation workspace registry"),
    );
    let build_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let admission = std::sync::Arc::new(
        crate::runtime_server_admission::WorkspaceGenerationAdmission::new(std::sync::Arc::new({
            let build_count = std::sync::Arc::clone(&build_count);
            move |_workspace, _root, _candidate, _mode, _paths, _provider_target, _cancellation| {
                build_count.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                Box::pin(async move {
                    Err(
                        crate::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
                            crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceBuilder,
                            "bootstrap fixture stops after proving the builder was scheduled",
                        ),
                    )
                })
            }
        })),
    );

    let started = std::time::Instant::now();
    let result = submit_mutation(
        std::sync::Arc::clone(&registry),
        Some(std::sync::Arc::clone(&admission)),
        workspace_identity.to_owned(),
        "first-ipc-owner-change".to_owned(),
        project_root.display().to_string(),
        vec![project_root.join("src/lib.rs").display().to_string()],
    );
    let elapsed = started.elapsed();

    match result {
        crate::workspace_db_ipc::WorkspaceDbIpcResult::RuntimeGenerationMutationSubmission {
            receipt,
        } => {
            assert_eq!(receipt.mutation_id, "first-ipc-owner-change");
            assert_eq!(
                receipt.state,
                crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Queued
            );
        }
        result => panic!("first mutation must bootstrap generation admission, got {result:?}"),
    }
    assert!(elapsed < std::time::Duration::from_millis(1), "{elapsed:?}");
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while build_count.load(std::sync::atomic::Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("first mutation scheduled bootstrap builder");
    assert_eq!(build_count.load(std::sync::atomic::Ordering::Acquire), 1);

    admission.shutdown().await.expect("shutdown admission");
    registry.shutdown().await.expect("shutdown registry");
}
