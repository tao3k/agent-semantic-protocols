//! Resident Runtime Server generation reuse regressions.

use agent_semantic_client_db::runtime_server_workspace::{
    RuntimeServerWorkspaceRegistry, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
    WorkspaceRecoverySource, WorkspaceRuntimeSelectorOverlay, WorkspaceRuntimeSelectorRead,
    WorkspaceSelectorSnapshot,
};
use std::sync::Arc;
use tempfile::tempdir;

fn owner(path: &str, selector: &str, bytes: &[u8]) -> WorkspaceOwnerSnapshot {
    WorkspaceOwnerSnapshot {
        owner_path: path.to_owned(),
        content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
        bytes: bytes.to_vec(),
        selectors: vec![WorkspaceSelectorSnapshot {
            selector: selector.to_owned(),
            byte_start: 0,
            byte_end: bytes.len(),
            derived_projections: Vec::new(),
        }],
    }
}

fn generation(
    workspace_identity: &str,
    project_root: &std::path::Path,
    epoch: u64,
    owner: WorkspaceOwnerSnapshot,
) -> WorkspaceMemoryGeneration {
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [(owner.owner_path.clone(), owner.content_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"resident-ready-fixture-provider").to_hex()
        ),
    );
    WorkspaceMemoryGeneration::try_from_build(
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationBuild {
            relations: Vec::new(),
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.display().to_string(),
            active_epoch: epoch,
            workspace_snapshot,
            source_snapshot,
            module_graph_digest: format!(
                "blake3-256:{}",
                blake3::hash(b"resident-ready-fixture-module-graph").to_hex()
            ),
            project_resolutions: Vec::new(),
            owners: vec![owner],
        },
    )
    .expect("typed resident ready generation")
}

#[tokio::test(flavor = "multi_thread")]
async fn one_workspace_handle_isolates_same_owner_path_across_project_resolutions() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry = Arc::new(
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry"),
    );
    let workspace_identity = "workspace-monorepo";
    let first_root = temporary.path().join("packages/first");
    let second_root = temporary.path().join("packages/second");
    let selector = "rust://src/lib.rs#item/function/run";
    let first_bytes = b"fn run() { first_package() }";
    let second_bytes = b"fn run() { second_package() }";

    registry
        .publish(
            "publish-first-scope",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                workspace_identity,
                &first_root,
                1,
                owner("src/lib.rs", selector, first_bytes),
            ),
        )
        .await
        .expect("publish first project scope");
    registry
        .publish(
            "publish-second-scope",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                workspace_identity,
                &second_root,
                1,
                owner("src/lib.rs", selector, second_bytes),
            ),
        )
        .await
        .expect("publish second project scope");

    let mut readers = Vec::with_capacity(128);
    for index in 0..128 {
        let registry = Arc::clone(&registry);
        let first_root = first_root.clone();
        let second_root = second_root.clone();
        readers.push(tokio::spawn(async move {
            let (root, expected) = if index % 2 == 0 {
                (&first_root, first_bytes.as_slice())
            } else {
                (&second_root, second_bytes.as_slice())
            };
            for _ in 0..256 {
                let lease = registry
                    .lease(workspace_identity, root)
                    .expect("scoped generation lease");
                assert_eq!(
                    lease
                        .project(selector)
                        .expect("scoped exact projection")
                        .bytes(),
                    expected
                );
                tokio::task::yield_now().await;
            }
        }));
    }
    for reader in readers {
        reader.await.expect("join scoped reader");
    }

    assert_eq!(registry.workspace_count(), 1);
    let receipt = registry
        .shutdown()
        .await
        .expect("drain workspace writer lane");
    assert_eq!(receipt.workspace_count, 1);
    assert_eq!(receipt.writer_lane_count, 1);
    receipt.validate().expect("single-writer shutdown receipt");
}

#[tokio::test(flavor = "multi_thread")]
async fn ready_recovery_receipt_reuses_resident_generation_without_resetting_overlays() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let source = b"fn first() {}\nfn second() {}";
    let selector = "rust://src/lib.rs#item/function/second";

    registry
        .publish(
            "resident-generation",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-a",
                temporary.path(),
                4,
                owner(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/first",
                    source,
                ),
            ),
        )
        .await
        .expect("publish resident generation");
    registry
        .publish_selector_overlay(
            "workspace-a",
            temporary.path(),
            WorkspaceRuntimeSelectorOverlay {
                projection_kind: "source".to_owned(),
                structural_selector: selector.to_owned(),
                owner_path: "src/lib.rs".to_owned(),
                owner_content_digest: format!("blake3-256:{}", blake3::hash(source).to_hex()),
                byte_start: 14,
                byte_end: source.len(),
                projection_bytes: b"fn second() {}".to_vec(),
            },
        )
        .await
        .expect("publish selector overlay");

    let receipt = registry
        .ready_recovery_receipt("warm-ready", "workspace-a", temporary.path())
        .expect("resident ready receipt");
    assert_eq!(receipt.active_epoch, 3);
    assert_eq!(receipt.target_epoch, 4);
    assert!(receipt.old_generation_readable);
    assert_eq!(receipt.counters.database_opens, 0);
    assert_eq!(receipt.counters.provider_spawns, 0);
    assert!(matches!(
        registry
            .read_runtime_selector("workspace-a", temporary.path(), "source", selector)
            .expect("read selector after ready receipt"),
        WorkspaceRuntimeSelectorRead::Projection { .. }
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_shutdown_drains_every_workspace_writer_lane() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().join("runtime")).expect("registry");
    registry
        .publish(
            "publish-before-shutdown",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-a",
                temporary.path(),
                1,
                owner(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/run",
                    b"fn run() {}",
                ),
            ),
        )
        .await
        .expect("publish before shutdown");
    let receipt = registry.shutdown().await.expect("drain writer lane");
    receipt.validate().expect("shutdown receipt");
    assert_eq!(receipt.writer_lane_count, 1);
    assert_eq!(receipt.forced_abort_count, 0);
    let error = registry
        .publish_owner_overlay(
            "publish-after-shutdown",
            "workspace-a",
            temporary.path(),
            owner(
                "src/lib.rs",
                "rust://src/lib.rs#item/function/run",
                b"fn run() {}",
            ),
        )
        .await
        .expect_err("shutdown writer lane must reject new work");
    assert!(error.contains("writer lane is unavailable"));
}
