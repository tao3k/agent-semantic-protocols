// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed generation fixtures shared by overlay projection scenarios.

use agent_semantic_client_db::runtime_server_workspace::WorkspaceMemoryGeneration;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot;
use agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path;

pub(super) fn project_root(workspace_identity: &str) -> std::path::PathBuf {
    std::path::PathBuf::from("/runtime-server-workspace-fixture").join(workspace_identity)
}

pub(super) fn resident_pointer(
    runtime_root: &std::path::Path,
    workspace_identity: &str,
) -> std::path::PathBuf {
    workspace_generation_pointer_path(
        runtime_root,
        workspace_identity,
        &project_root(workspace_identity),
    )
    .unwrap()
}

pub(super) fn owner(path: &str, selector: &str, bytes: &[u8]) -> WorkspaceOwnerSnapshot {
    WorkspaceOwnerSnapshot {
        authority: None,
        owner_path: path.to_owned(),
        content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
        bytes: bytes.to_vec(),
        native_syntax_diagnostic: None,
        selectors: vec![WorkspaceSelectorSnapshot {
            selector: selector.to_owned(),
            byte_start: 0,
            byte_end: bytes.len(),
            query_keys: Vec::new(),
            derived_projections: Vec::new(),
        }],
    }
}

pub(super) fn generation(
    workspace_identity: &str,
    epoch: u64,
    owner: WorkspaceOwnerSnapshot,
) -> WorkspaceMemoryGeneration {
    generation_with_owners(workspace_identity, epoch, vec![owner])
}

pub(super) fn generation_with_owners(
    workspace_identity: &str,
    epoch: u64,
    owners: Vec<WorkspaceOwnerSnapshot>,
) -> WorkspaceMemoryGeneration {
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        owners
            .iter()
            .map(|owner| (owner.owner_path.clone(), owner.content_digest.clone())),
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"runtime-workspace-fixture-provider").to_hex()
        ),
    );
    let module_graph_digest = format!(
        "blake3-256:{}",
        blake3::hash(b"runtime-workspace-fixture-module-graph").to_hex()
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
                .expect("source snapshot integrity reference"),
            module_graph_digest.clone(),
        )
        .expect("Runtime provider execution binding");
    WorkspaceMemoryGeneration::try_from_build(
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationBuild {
            projection_capability: crate::fixture::overlay_projection_capability_manifest_fixture(),
            relations: Vec::new(),
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root(workspace_identity).display().to_string(),
            active_epoch: epoch,
            workspace_snapshot,
            content_search_generation: crate::fixture::content_search_generation_receipt(
                workspace_identity,
                &source_snapshot,
            ),
            source_snapshot,
            module_graph_digest,
            runtime_provider_execution_binding: Some(runtime_provider_execution_binding),
            project_resolutions: Vec::new(),
            auxiliary_owners: Vec::new(),
            owners,
        },
    )
    .expect("typed runtime workspace generation")
}
