use super::{
    mmap_exact_projection_for_scope, owner_path_from_selector, resident_owner_needs_repair,
};
use agent_semantic_client_db::runtime_server_workspace::{
    RuntimeServerWorkspaceRegistry, WorkspaceDerivedProjectionSnapshot, WorkspaceGenerationState,
    WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceRecoverySource,
    WorkspaceRuntimeOwnerFreshnessReceipt, WorkspaceRuntimeSelectorRead, WorkspaceSelectorSnapshot,
};
use std::sync::atomic::{AtomicU64, Ordering};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

fn freshness(changed: bool) -> WorkspaceRuntimeOwnerFreshnessReceipt {
    WorkspaceRuntimeOwnerFreshnessReceipt {
        schema_id: "asp.runtime-owner-freshness-receipt.v1".to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: "workspace-test".to_owned(),
        generation_digest: format!("blake3-256:{}", "a".repeat(64)),
        owner_path: "src/lib.rs".to_owned(),
        owner_content_digest: Some(format!("blake3-256:{}", "b".repeat(64))),
        changed,
        removed: false,
    }
}

fn owner_for_repair(selectors: Vec<WorkspaceSelectorSnapshot>) -> WorkspaceRuntimeSelectorRead {
    WorkspaceRuntimeSelectorRead::OwnerForRepair {
        generation_digest: format!("blake3-256:{}", "a".repeat(64)),
        root_digest: "root".to_owned(),
        owner: WorkspaceOwnerSnapshot {
            owner_path: "src/lib.rs".to_owned(),
            content_digest: format!("blake3-256:{}", "b".repeat(64)),
            bytes: b"fn run() {}".to_vec(),
            selectors,
        },
    }
}

#[test]
fn exact_selector_owner_path_is_workspace_relative() {
    assert_eq!(
        owner_path_from_selector("rust://src/lib.rs#item/function/run").unwrap(),
        "src/lib.rs"
    );
}

#[test]
fn changed_or_unparsed_owner_requires_provider_repair() {
    assert!(resident_owner_needs_repair(
        &freshness(true),
        &owner_for_repair(Vec::new()),
        "callable-skeleton",
        "rust://src/lib.rs#item/function/run",
    ));
    assert!(resident_owner_needs_repair(
        &freshness(false),
        &owner_for_repair(Vec::new()),
        "callable-skeleton",
        "rust://src/lib.rs#item/function/run",
    ));
    assert!(!resident_owner_needs_repair(
        &freshness(false),
        &owner_for_repair(vec![WorkspaceSelectorSnapshot {
            selector: "rust://src/lib.rs#item/function/existing".to_owned(),
            byte_start: 0,
            byte_end: 11,
            derived_projections: Vec::new(),
        }]),
        "callable-skeleton",
        "rust://src/lib.rs#item/function/run",
    ));
    assert!(resident_owner_needs_repair(
        &freshness(false),
        &owner_for_repair(vec![WorkspaceSelectorSnapshot {
            selector: "rust://src/lib.rs#item/function/run".to_owned(),
            byte_start: 0,
            byte_end: 11,
            derived_projections: Vec::new(),
        }]),
        "callable-skeleton",
        "rust://src/lib.rs#item/function/run",
    ));
    assert!(!resident_owner_needs_repair(
        &freshness(false),
        &owner_for_repair(vec![WorkspaceSelectorSnapshot {
            selector: "rust://src/lib.rs#item/function/run".to_owned(),
            byte_start: 0,
            byte_end: 11,
            derived_projections: vec![WorkspaceDerivedProjectionSnapshot {
                projection_kind: "callable-skeleton".to_owned(),
                bytes: b"fn run()".to_vec(),
            }],
        }]),
        "callable-skeleton",
        "rust://src/lib.rs#item/function/run",
    ));
}

fn generation(
    workspace_identity: &str,
    project_root: &std::path::Path,
    selector: &str,
    bytes: &[u8],
) -> WorkspaceMemoryGeneration {
    let owner = WorkspaceOwnerSnapshot {
        owner_path: "src/lib.rs".to_owned(),
        content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
        bytes: bytes.to_vec(),
        selectors: vec![WorkspaceSelectorSnapshot {
            selector: selector.to_owned(),
            byte_start: 0,
            byte_end: bytes.len(),
            derived_projections: Vec::new(),
        }],
    };
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [(owner.owner_path.clone(), owner.content_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "provider-resident-exact-mmap-fixture".to_owned(),
    );
    let workspace_generation = agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1 {
        root_digest: source_snapshot.root_digest.clone(),
        root_depth: 1,
        leaf_count: 1,
        owner_count: 1,
    };
    let generation_digest = format!(
        "blake3-256:{}",
        blake3::hash(format!("{workspace_identity}:1").as_bytes()).to_hex()
    );
    let project_resolutions = Vec::new();
    let workspace_source_scope_generation =
        agent_semantic_runtime::workspace_source_scope_generation_digest(&project_resolutions)
            .unwrap();
    WorkspaceMemoryGeneration {
        workspace_identity: workspace_identity.to_owned(),
        project_root: project_root.display().to_string(),
        state: WorkspaceGenerationState::Ready,
        active_epoch: 1,
        generation_digest: generation_digest.clone(),
        root_depth: [1, 0],
        workspace_snapshot,
        source_snapshot,
        workspace_generation,
        memory_backend_digest: generation_digest,
        workspace_source_scope_generation,
        project_resolutions,
        owners: vec![owner],
    }
}

#[tokio::test]
async fn exact_warm_hit_reads_scoped_mmap_without_a_runtime_control_session() {
    let checkout = std::env::temp_dir().join(format!(
        "asp-exact-mmap-fixture-{}-{}",
        std::process::id(),
        FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    tokio::fs::create_dir_all(&checkout).await.unwrap();
    let project_root = tokio::fs::canonicalize(&checkout).await.unwrap();
    let workspace_identity =
        super::super::runtime_server::runtime_server_workspace_identity_for_admission(
            &project_root,
        )
        .unwrap();
    let workspace_store =
        agent_semantic_client_db::runtime_server_runtime_base().join("workspaces");
    let registry = RuntimeServerWorkspaceRegistry::new(workspace_store.clone()).unwrap();
    let selector = "rust://src/lib.rs#item/function/mmap_fixture";
    let bytes = b"fn mmap_fixture() {}\n";
    registry
        .publish(
            "mmap-fixture",
            WorkspaceRecoverySource::TursoGeneration,
            generation(&workspace_identity, &project_root, selector, bytes),
        )
        .await
        .unwrap();

    let exact = super::super::provider_exact_args::ExactQueryArgs {
        structural_selector: selector.to_owned(),
        projection: "source".to_owned(),
        json: false,
    };
    let read = mmap_exact_projection_for_scope(&workspace_identity, &project_root, &exact)
        .await
        .unwrap();
    assert!(matches!(
        read,
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
            bytes: projection,
            ..
        } if projection == bytes
    ));

    registry.shutdown().await.unwrap();
    tokio::fs::remove_dir_all(workspace_store.join(workspace_identity))
        .await
        .unwrap();
    tokio::fs::remove_dir_all(checkout).await.unwrap();
}

#[tokio::test]
async fn exact_miss_reads_immutable_owner_without_control_or_provider_repair() {
    let checkout = std::env::temp_dir().join(format!(
        "asp-exact-mmap-miss-fixture-{}-{}",
        std::process::id(),
        FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    tokio::fs::create_dir_all(&checkout).await.unwrap();
    let project_root = tokio::fs::canonicalize(&checkout).await.unwrap();
    let workspace_identity =
        super::super::runtime_server::runtime_server_workspace_identity_for_admission(
            &project_root,
        )
        .unwrap();
    let workspace_store =
        agent_semantic_client_db::runtime_server_runtime_base().join("workspaces");
    let registry = RuntimeServerWorkspaceRegistry::new(workspace_store.clone()).unwrap();
    let existing_selector = "rust://src/lib.rs#item/function/existing";
    let requested_selector = "rust://src/lib.rs#item/function/missing";
    let bytes = b"fn existing() {}\n";
    registry
        .publish(
            "mmap-miss-fixture",
            WorkspaceRecoverySource::TursoGeneration,
            generation(&workspace_identity, &project_root, existing_selector, bytes),
        )
        .await
        .unwrap();

    let exact = super::super::provider_exact_args::ExactQueryArgs {
        structural_selector: requested_selector.to_owned(),
        projection: "source".to_owned(),
        json: false,
    };
    let read = mmap_exact_projection_for_scope(&workspace_identity, &project_root, &exact)
        .await
        .unwrap();
    assert!(matches!(
        read,
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::OwnerForRepair {
            owner,
            ..
        } if owner.bytes == bytes
    ));
    let counters = registry.data_plane_counters();
    assert_eq!(counters.database_opens, 0);
    assert_eq!(counters.provider_spawns, 0);
    assert_eq!(counters.schema_bootstraps, 0);
    assert_eq!(counters.lock_probes, 0);

    registry.shutdown().await.unwrap();
    tokio::fs::remove_dir_all(workspace_store.join(workspace_identity))
        .await
        .unwrap();
    tokio::fs::remove_dir_all(checkout).await.unwrap();
}
