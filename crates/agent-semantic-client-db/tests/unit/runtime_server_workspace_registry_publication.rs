static NEXT_TEST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn resident_generation(
    workspace_identity: &str,
    project_root: &std::path::Path,
) -> crate::runtime_server_workspace::WorkspaceMemoryGeneration {
    let owner = crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
        owner_path: "src/lib.rs".to_owned(),
        content_digest: format!("blake3-256:{}", blake3::hash(b"fn resident() {}").to_hex()),
        bytes: b"fn resident() {}".to_vec(),
        selectors: Vec::new(),
    };
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        std::iter::once((owner.owner_path.clone(), owner.content_digest.clone())),
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
    );
    crate::runtime_server_workspace::WorkspaceMemoryGeneration::try_from_build(
        crate::runtime_server_workspace::WorkspaceGenerationBuild {
            projection_capability: crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::single_selector(
                "blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
                "rust://src/lib.rs#item/function/resident".to_owned(),
                "src/lib.rs".to_owned(),
                std::collections::BTreeSet::from([
                    crate::active_generation_projection_capability::ActiveGenerationProjectionMode::Source,
                ]),
            )
            .expect("projection capability"),
            relations: Vec::new(),
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.display().to_string(),
            active_epoch: 2,
            workspace_snapshot,
            source_snapshot,
            module_graph_digest:
                "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
            project_resolutions: Vec::new(),
            owners: vec![owner],
        },
    )
    .expect("resident generation")
}

async fn assert_resident_ready_receipt_ignores_admission() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let project_root = temporary.path().join("project");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("project root");
    let registry = crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
        temporary.path().join("runtime"),
    )
    .expect("registry");
    registry
        .publish(
            "resident-missing-admission",
            crate::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            resident_generation("resident-missing-admission", &project_root),
        )
        .await
        .expect("publish resident generation");
    let result = crate::workspace_db_ipc_server::generation::require_lifecycle_generation(
        &registry,
        None,
        "resident-missing-admission",
        project_root.display().to_string(),
    )
    .await;
    let crate::workspace_db_ipc::WorkspaceDbIpcResult::RuntimeGenerationResidentReady { receipt } =
        result
    else {
        panic!("resident readiness must not depend on admission watch: {result:?}");
    };
    assert_eq!(receipt.active_epoch, 1);
    assert_eq!(receipt.target_epoch, 2);
    assert_eq!(receipt.projection_capability.publication_epoch, 2);
    receipt.validate().expect("resident receipt validates");
}

#[tokio::test(flavor = "current_thread")]
async fn resident_ready_with_missing_admission_returns_resident_receipt() {
    assert_resident_ready_receipt_ignores_admission().await;
}

#[tokio::test(flavor = "current_thread")]
async fn resident_ready_with_building_admission_returns_resident_receipt() {
    assert_resident_ready_receipt_ignores_admission().await;
}

#[test]
fn reusable_receipt_epoch_binding_uses_generation_epoch_after_restart() {
    use std::collections::BTreeSet;

    let generation_active_epoch = 2;
    let previous_epoch = generation_active_epoch - 1;

    let manifest = crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::single_selector(
            "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
            "selector".to_owned(),
            "src/lib.rs".to_owned(),
            BTreeSet::from([
                crate::active_generation_projection_capability::ActiveGenerationProjectionMode::Source,
            ]),
        )
        .expect("valid projection capability manifest");
    let capability = manifest
        .into_ready_receipt(
            "workspace".to_owned(),
            "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_owned(),
            "blake3-256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_owned(),
            generation_active_epoch,
        )
        .expect("valid projection capability receipt");
    let receipt = crate::runtime_server_workspace::WorkspaceRecoveryReceipt {
        schema_id:
            agent_semantic_client_db::runtime_server_workspace::WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID
                .to_owned(),
        schema_version: "1".to_owned(),
        request_id: "request".to_owned(),
        workspace_identity: "workspace".to_owned(),
        source: crate::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
        state: crate::runtime_server_workspace::WorkspaceGenerationState::Ready,
        active_epoch: previous_epoch,
        target_epoch: generation_active_epoch,
        generation_digest:
            "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
        source_root_digest:
            "blake3-256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned(),
        projection_capability: capability,
        old_generation_readable: true,
        resident_publication_elapsed_micros: 0,
        counters: Default::default(),
    };

    receipt
        .validate()
        .expect("same epoch recovery receipt validates");
    assert_eq!(receipt.active_epoch, previous_epoch);
    assert_eq!(receipt.target_epoch, generation_active_epoch);
    assert_eq!(
        receipt.projection_capability.publication_epoch,
        generation_active_epoch
    );
}

#[tokio::test(flavor = "current_thread")]
async fn prepared_workspace_scope_waits_for_writer_lane_readiness() {
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        NEXT_TEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    );
    let root = std::env::temp_dir().join(format!("asp-writer-ready-{nonce}"));
    let project_root = root.join("project");
    let registry =
        crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(root.clone())
            .expect("create runtime workspace registry");

    let prepared = registry
        .prepare_resident_workspace_scope("workspace-writer-ready", &project_root)
        .await
        .expect("prepare resident workspace scope");

    assert_eq!(prepared, project_root);
    registry
        .shutdown()
        .await
        .expect("shutdown prepared workspace registry");
    let _ = tokio::fs::remove_dir_all(root).await;
}
