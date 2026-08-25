//! Runtime publication and mmap lookup contract for compact projection identity.

use agent_semantic_client_db::runtime_server_workspace::{
    ExactProjectionKind, RuntimeServerWorkspaceRegistry, WorkspaceExactProjectionDataPlaneClient,
    WorkspaceGenerationBuild, WorkspaceGenerationPointerReader, WorkspaceMemoryGeneration,
    WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorRead, WorkspaceSelectorSnapshot,
    workspace_generation_pointer_path,
};
use std::io::{Seek, Write};

#[tokio::test(flavor = "multi_thread")]
async fn runtime_publication_interns_callable_identity_and_serves_compact_bytes() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let selector = "rust://src/lib.rs#item/function/run";
    let source = b"fn run() {}";
    let projection =
        crate::projection_fixture::callable_skeleton_projection_fixture(selector, "run");
    let owner = owner_with_projection(selector, source, projection);

    registry
        .publish(
            "compact-callable-identity",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(owner, 1),
        )
        .await
        .expect("publish compact callable generation");
    let pointer = workspace_generation_pointer_path(
        temporary.path(),
        "workspace-compact-callable-identity",
        &project_root(),
    )
    .expect("resident pointer");
    let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
        .await
        .expect("open compact callable generation");

    let bytes = match client
        .read_runtime_selector(ExactProjectionKind::CallableSkeleton, selector)
        .expect("read compact callable projection")
    {
        WorkspaceRuntimeSelectorRead::Projection { bytes, .. } => bytes,
        read => panic!("callable projection must be resident: {read:?}"),
    };
    assert!(
        bytes.len() <= 1_024,
        "root callable projection exceeds 1 KiB"
    );
    let compact: agent_semantic_content_identity::semantic_projection::SemanticProjection<
        agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonPayload,
    > = serde_json::from_slice(&bytes).expect("decode referenced projection");
    compact.validate().expect("validate semantic projection");
    compact
        .payload
        .validate()
        .expect("validate projection payload");
    let context = client
        .projection_evidence_context(&compact.evidence_context_ref)
        .expect("resolve evidence context")
        .expect("evidence context is resident");
    let context: agent_semantic_content_identity::projection_evidence_context::ProjectionEvidenceContext =
        serde_json::from_slice(&context).expect("decode evidence context");
    context.validate().expect("validate evidence context");
    assert_eq!(context.evidence_context_ref, compact.evidence_context_ref);

    let snapshot = WorkspaceGenerationPointerReader::open(&pointer)
        .await
        .expect("pointer reader")
        .read()
        .expect("published snapshot");
    let exact_path =
        std::path::PathBuf::from(snapshot.mmap_segment_path).with_extension("exact.mmap");
    let mut exact_file = std::fs::OpenOptions::new()
        .write(true)
        .open(exact_path)
        .expect("open prior exact mmap");
    exact_file
        .seek(std::io::SeekFrom::Start(152))
        .expect("seek context count");
    exact_file
        .write_all(&usize::MAX.to_le_bytes())
        .expect("corrupt cached prior layout");
    exact_file.sync_all().expect("sync prior layout corruption");

    let next_source = b"fn run() { todo!() }";
    let next_projection =
        crate::projection_fixture::callable_skeleton_projection_fixture(selector, "run");
    registry
        .publish(
            "compact-callable-identity-refresh",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                owner_with_projection(selector, next_source, next_projection),
                2,
            ),
        )
        .await
        .expect("publication must invalidate the prior exact mmap cell before reopening");
}

fn project_root() -> std::path::PathBuf {
    std::path::PathBuf::from("/runtime-server-workspace-fixture")
        .join("workspace-compact-callable-identity")
}

fn owner_with_projection(
    selector: &str,
    source: &[u8],
    projection: agent_semantic_client_db::runtime_server_workspace::WorkspaceDerivedProjectionSnapshot,
) -> WorkspaceOwnerSnapshot {
    WorkspaceOwnerSnapshot {
        authority: None,
        owner_path: "src/lib.rs".to_owned(),
        content_digest: format!("blake3-256:{}", blake3::hash(source).to_hex()),
        bytes: source.to_vec(),
        selectors: vec![WorkspaceSelectorSnapshot {
            selector: selector.to_owned(),
            byte_start: 0,
            byte_end: source.len(),
            derived_projections: vec![projection],
        }],
    }
}

fn generation(owner: WorkspaceOwnerSnapshot, active_epoch: u64) -> WorkspaceMemoryGeneration {
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [(owner.owner_path.clone(), owner.content_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!("blake3-256:{}", blake3::hash(b"fixture-provider").to_hex()),
    );
    WorkspaceMemoryGeneration::try_from_build(WorkspaceGenerationBuild {
        projection_capability: crate::fixture::overlay_projection_capability_manifest_fixture(),
        relations: Vec::new(),
        workspace_identity: "workspace-compact-callable-identity".to_owned(),
        project_root: project_root().display().to_string(),
        active_epoch,
        workspace_snapshot,
        source_snapshot,
        module_graph_digest: format!("blake3-256:{}", blake3::hash(b"module-graph").to_hex()),
        project_resolutions: Vec::new(),
        owners: vec![owner],
    })
    .expect("typed runtime workspace generation")
}
