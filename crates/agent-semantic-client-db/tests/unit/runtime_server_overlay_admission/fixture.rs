// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Shared typed generation fixtures for overlay admission scenarios.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use agent_semantic_client_db::runtime_server_workspace::WorkspaceMemoryGeneration;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot;

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

pub(super) fn fixture_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "asp-runtime-server-overlay-admission-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(super) fn generation(
    workspace_identity: &str,
    project_root: &std::path::Path,
    epoch: u64,
    bytes: &[u8],
) -> WorkspaceMemoryGeneration {
    generation_with_selectors(workspace_identity, project_root, epoch, bytes, Vec::new())
}

pub(super) fn generation_with_selectors(
    workspace_identity: &str,
    project_root: &std::path::Path,
    epoch: u64,
    bytes: &[u8],
    selectors: Vec<agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot>,
) -> WorkspaceMemoryGeneration {
    let content_digest = format!("blake3-256:{}", blake3::hash(bytes).to_hex());
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [("src/lib.rs", content_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"runtime-overlay-fixture-provider").to_hex()
        ),
    );
    let module_graph_digest = format!(
        "blake3-256:{}",
        blake3::hash(b"runtime-overlay-fixture-module-graph").to_hex()
    );
    let runtime_provider_execution_binding =
        agent_semantic_artifacts::runtime_provider_execution_binding::RuntimeProviderExecutionBinding::build(
            crate::fixture::FIXTURE_PROJECT_ID.to_owned(),
            workspace_identity.to_owned(),
            format!("blake3-256:{}", "1".repeat(64)),
            format!("blake3-256:{}", "2".repeat(64)),
            format!("blake3-256:{}", "3".repeat(64)),
            source_snapshot
                .root_integrity_reference()
                .expect("fixture source snapshot integrity reference"),
            module_graph_digest.clone(),
        )
        .expect("fixture Runtime provider execution binding");
    let content_search_generation =
        crate::fixture::content_search_generation_receipt(workspace_identity, &source_snapshot);
    WorkspaceMemoryGeneration::try_from_build(
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationBuild {
            projection_capability: crate::fixture::projection_capability_manifest_fixture(),
            relations: Vec::new(),
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.display().to_string(),
            active_epoch: epoch,
            workspace_snapshot,
            content_search_generation,
            source_snapshot,
            module_graph_digest,
            runtime_provider_execution_binding: Some(runtime_provider_execution_binding),
            project_resolutions: Vec::new(),
            auxiliary_owners: Vec::new(),
            owners: vec![WorkspaceOwnerSnapshot {
                authority: None,
                owner_path: "src/lib.rs".to_owned(),
                content_digest,
                bytes: bytes.to_vec(),
                native_syntax_diagnostic: None,
                selectors,
            }],
        },
    )
    .expect("typed runtime overlay generation")
}
