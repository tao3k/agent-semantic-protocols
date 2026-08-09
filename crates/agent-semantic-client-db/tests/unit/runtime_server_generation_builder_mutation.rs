use super::publish_mutation_generation;

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
            blake3::hash(b"mutation-builder-provider").to_hex()
        ),
    );
    crate::runtime_server_workspace::WorkspaceMemoryGeneration::try_from_build(
        crate::runtime_server_workspace::WorkspaceGenerationBuild {
            relations: Vec::new(),
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.display().to_string(),
            active_epoch: 1,
            workspace_snapshot,
            source_snapshot,
            module_graph_digest: format!(
                "blake3-256:{}",
                blake3::hash(b"mutation-builder-module-graph").to_hex()
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
    .expect("typed mutation builder generation")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mutation_builder_projects_only_changed_owner_and_publishes_one_epoch() {
    let temp = tempfile::tempdir().expect("temporary mutation builder root");
    let project_root = temp.path().join("project");
    tokio::fs::create_dir_all(project_root.join("src"))
        .await
        .expect("create mutation builder source root");
    let changed_bytes = b"fn changed() {}\n";
    tokio::fs::write(project_root.join("src/lib.rs"), changed_bytes)
        .await
        .expect("write changed owner");
    let workspace_identity = "workspace-mutation-builder";
    let registry = std::sync::Arc::new(
        crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            temp.path().join("runtime"),
        )
        .expect("create mutation builder registry"),
    );
    registry
        .publish(
            "publish-mutation-builder-base",
            crate::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity, &project_root, b"fn previous() {}\n"),
        )
        .await
        .expect("publish mutation builder base");
    let projection_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let owner_builder: crate::runtime_server_admission::WorkspaceOwnerProjectionBuilder =
        std::sync::Arc::new({
            let projection_count = std::sync::Arc::clone(&projection_count);
            move |_workspace_identity, project_root, owner_path| {
                let projection_count = std::sync::Arc::clone(&projection_count);
                Box::pin(async move {
                    projection_count.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                    let bytes = tokio::fs::read(project_root.join(&owner_path))
                        .await
                        .map_err(|error| format!("read changed owner: {error}"))?;
                    Ok(crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
                        owner_path,
                        content_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
                        bytes,
                        selectors: Vec::new(),
                    })
                })
            }
        });

    let completion = publish_mutation_generation(
        &owner_builder,
        &registry,
        workspace_identity,
        &project_root,
        &std::collections::BTreeSet::from([project_root.join("src/lib.rs")]),
        "publish-one-owner-mutation".to_owned(),
        candidate(),
    )
    .await
    .expect("publish changed owner without full generation collection");

    assert_eq!(
        projection_count.load(std::sync::atomic::Ordering::Acquire),
        1
    );
    assert_eq!(completion.commit.active_epoch, 2);
    let mut expected_candidate_digest = blake3::Hasher::new();
    expected_candidate_digest.update(b"workspace-mutation-candidate-v1\0");
    expected_candidate_digest.update(completion.commit.source_root_digest.as_bytes());
    assert_eq!(
        completion.candidate.candidate_generation.digest,
        format!("blake3:{}", expected_candidate_digest.finalize())
    );
    let lease = registry
        .lease(workspace_identity, &project_root)
        .expect("lease mutation generation");
    assert_eq!(lease.epoch(), 2);
    assert_eq!(
        lease.owner("src/lib.rs").as_deref(),
        Some(changed_bytes.as_slice())
    );
    let pointer_path = crate::runtime_server_workspace::workspace_generation_pointer_path(
        &temp.path().join("runtime"),
        workspace_identity,
        &project_root,
    )
    .expect("resolve mutation generation pointer");
    let authority = crate::runtime_server_workspace::read_search_generation_authority_segment(
        &pointer_path,
        2,
        workspace_identity,
        &project_root.display().to_string(),
    )
    .await
    .expect("read mutation search generation authority");
    assert_eq!(authority.active_epoch, 2);
    assert_eq!(
        authority.generation_digest,
        completion.commit.generation_digest
    );
    assert_eq!(
        authority.source_snapshot.root_digest,
        lease.generation().source_snapshot.root_digest
    );

    registry.shutdown().await.expect("shutdown registry");
}
