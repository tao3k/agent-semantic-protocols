// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Process-cold Query replay fixtures shared by the focused test modules.

use super::{
    AuthorityStamp, ContentBinding, ContentIdentity, ContentPublicationCommit,
    RuntimeExecutionBinding, RuntimeExecutionBindingInput, RuntimeWorkspaceExecutionPublication,
    RuntimeWorkspaceExecutionPublicationInput, digest,
};

pub(super) fn process_cold_generation(
    project_root: &std::path::Path,
    workspace_identity: &str,
    selector: &str,
    source: &[u8],
) -> agent_semantic_client_db::runtime_server_workspace::WorkspaceMemoryGeneration {
    use agent_semantic_client_db::active_generation_projection_capability::{
        ActiveGenerationProjectionCapabilityManifest, ActiveGenerationProjectionMode,
    };
    use agent_semantic_client_db::runtime_server_workspace::{
        WorkspaceGenerationBuild, WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot,
    };
    use agent_semantic_search::{
        ContentSearchGenerationReceipt, SearchGenerationConstructionStage,
        SearchGenerationIdentity, SearchGenerationStageReceipt, canonical_blake3_digest,
    };

    let owner_path = "src/lib.rs";
    let owner_digest = format!("blake3-256:{}", blake3::hash(source).to_hex());
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [(owner_path.to_owned(), owner_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        digest('b'),
    );
    let module_graph_digest = digest('c');
    let project_id = "repo-0000000000000001";
    let runtime_provider_execution_binding = agent_semantic_artifacts::runtime_provider_execution_binding::RuntimeProviderExecutionBinding::build(
        project_id.to_owned(),
        workspace_identity.to_owned(),
        digest('1'),
        digest('2'),
        digest('3'),
        source_snapshot
            .root_integrity_reference()
            .expect("process-cold source integrity reference"),
        module_graph_digest.clone(),
    )
    .expect("process-cold provider execution binding");
    let search_identity = SearchGenerationIdentity {
        project_id: project_id.to_owned(),
        workspace_id: workspace_identity.to_owned(),
        source_root_digest: canonical_blake3_digest(&source_snapshot.root_digest)
            .expect("process-cold source root digest"),
        provider_digest: canonical_blake3_digest(&source_snapshot.provider_digest)
            .expect("process-cold provider digest"),
        schema_digest: canonical_blake3_digest(
            &agent_semantic_content_identity::project_resolution_schema_digest(),
        )
        .expect("process-cold schema digest"),
        generation_candidate_digest: digest('d'),
    };
    let content_search_generation =
        ContentSearchGenerationReceipt::new(SearchGenerationStageReceipt {
            stage: SearchGenerationConstructionStage::SourceByteAcquisition,
            identity: search_identity,
            artifact_digest: digest('e'),
            worker_id: "process-cold-query-test".to_owned(),
            complete: true,
        })
        .expect("process-cold search generation");
    let projection_capability = ActiveGenerationProjectionCapabilityManifest::single_selector(
        digest('f'),
        selector.to_owned(),
        owner_path.to_owned(),
        std::collections::BTreeSet::from([ActiveGenerationProjectionMode::Source]),
    )
    .expect("process-cold projection capability");
    agent_semantic_client_db::runtime_server_workspace::WorkspaceMemoryGeneration::try_from_build(
        WorkspaceGenerationBuild {
            projection_capability,
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.display().to_string(),
            active_epoch: 1,
            workspace_snapshot,
            source_snapshot,
            module_graph_digest,
            runtime_provider_execution_binding: Some(runtime_provider_execution_binding),
            content_search_generation,
            project_resolutions: Vec::new(),
            auxiliary_owners: Vec::new(),
            owners: vec![WorkspaceOwnerSnapshot {
                authority: None,
                owner_path: owner_path.to_owned(),
                content_digest: owner_digest,
                bytes: source.to_vec(),
                native_syntax_diagnostic: None,
                selectors: vec![WorkspaceSelectorSnapshot {
                    selector: selector.to_owned(),
                    byte_start: 0,
                    byte_end: source.len(),
                    query_keys: Vec::new(),
                    derived_projections: Vec::new(),
                }],
            }],
            relations: Vec::new(),
        },
    )
    .expect("process-cold workspace generation")
}

pub(super) fn execution_publication_for_exact_generation(
    workspace_identity: &str,
    generation_digest: &str,
    source_root_digest: &str,
    runtime_bundle_digest: &str,
    project_workspace: agent_semantic_content_identity::ProjectWorkspaceBinding,
) -> RuntimeWorkspaceExecutionPublication {
    let identity = ContentIdentity {
        runtime_artifact_digest: digest('1'),
        workspace_snapshot_digest: source_root_digest.to_owned(),
        source_generation_digest: generation_digest.to_owned(),
        source_index_digest: digest('4'),
        schema_digest: digest('5'),
        provider_catalog_digest: digest('2'),
    };
    let content_binding = ContentBinding::new(
        identity.clone(),
        AuthorityStamp {
            key_id: "runtime-server-complete-generation".into(),
            canonical_digest: identity.digest(),
            signature: digest('7'),
        },
    )
    .expect("process-cold content binding");
    let runtime_execution_binding = RuntimeExecutionBinding::new(RuntimeExecutionBindingInput {
        project_workspace,
        worktree_instance_id: "worktree-main".into(),
        publication_nonce: "process-cold-publication".into(),
        content_binding,
        runtime_artifact_digest: digest('1').into(),
        evaluator_policy_digest: digest('8').into(),
        active_artifact_receipt_digest: digest('9').into(),
        evaluator_abi_digest: digest('a').into(),
    })
    .expect("process-cold runtime binding");
    let content_publication_commit = ContentPublicationCommit::linearize(
        runtime_execution_binding.content_binding.identity.clone(),
        runtime_execution_binding
            .content_binding
            .authority_stamp
            .clone(),
    )
    .expect("process-cold content publication commit");
    RuntimeWorkspaceExecutionPublication::new(RuntimeWorkspaceExecutionPublicationInput {
        workspace_identity: workspace_identity.to_owned(),
        generation_digest: generation_digest.to_owned().into(),
        source_root_digest: source_root_digest.to_owned().into(),
        content_publication_commit,
        runtime_execution_binding,
        runtime_bundle_digest: runtime_bundle_digest.to_owned().into(),
    })
    .expect("process-cold execution publication")
}

pub(super) fn process_cold_resources() -> (
    agent_semantic_workspace_scheduler::RuntimeServerResourceSupervisor,
    agent_semantic_workspace_scheduler::RuntimeServerTaskScope,
) {
    (
        agent_semantic_workspace_scheduler::RuntimeServerResourceSupervisor::new(
            4,
            16 * 1024 * 1024,
        ),
        agent_semantic_workspace_scheduler::RuntimeServerTaskScope::new("process-cold-query-test"),
    )
}
