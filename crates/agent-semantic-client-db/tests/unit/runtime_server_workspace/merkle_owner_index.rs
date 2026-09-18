// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    SearchGenerationSectionKind, SearchMerkleOwnerRecord, ValidatedSearchGenerationSegment,
    ValidatedSortedRecordTable, encode_workspace_search_generation_segment,
};
use crate::runtime_server_workspace::{
    WorkspaceGenerationBuild, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
};

#[test]
fn search_segment_publishes_a_verified_owner_inclusion_proof() {
    let bytes = b"pub fn indexed_owner() {}\n".to_vec();
    let sibling_bytes = b"pub fn sibling_owner() {}\n".to_vec();
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([
        ("src/lib.rs", bytes.as_slice()),
        ("src/sibling.rs", sibling_bytes.as_slice()),
    ]);
    let projection_capability =
        crate::active_generation_projection_capability::test_projection_capability_manifest();
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        projection_capability.provider_catalog_digest.clone(),
    );
    let module_graph_digest = format!("blake3-256:{}", blake3::hash(b"module-graph").to_hex());
    let runtime_provider_execution_binding =
        agent_semantic_artifacts::runtime_provider_execution_binding::RuntimeProviderExecutionBinding::build(
            "repo-merkle-proof".to_owned(),
            "workspace-merkle-proof".to_owned(),
            format!("blake3-256:{}", "1".repeat(64)),
            format!("blake3-256:{}", "2".repeat(64)),
            format!("blake3-256:{}", "3".repeat(64)),
            source_snapshot
                .root_integrity_reference()
                .expect("source snapshot integrity reference"),
            module_graph_digest.clone(),
        )
        .expect("Runtime provider execution binding");
    let generation = WorkspaceMemoryGeneration::try_from_build(WorkspaceGenerationBuild {
        projection_capability,
        workspace_identity: "workspace-merkle-proof".to_owned(),
        project_root: "/workspace/merkle-proof".to_owned(),
        active_epoch: 1,
        workspace_snapshot,
        content_search_generation:
            crate::runtime_server_workspace::test_fixture::content_search_generation_receipt(
                &runtime_provider_execution_binding.project_id,
                "workspace-merkle-proof",
                &source_snapshot,
            ),
        source_snapshot,
        module_graph_digest,
        runtime_provider_execution_binding: Some(runtime_provider_execution_binding.clone()),
        project_resolutions: Vec::new(),
        auxiliary_owners: Vec::new(),
        owners: vec![
            WorkspaceOwnerSnapshot {
                authority: None,
                owner_path: "src/lib.rs".to_owned(),
                content_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
                native_syntax_diagnostic: None,
                bytes,
                selectors: Vec::new(),
            },
            WorkspaceOwnerSnapshot {
                authority: None,
                owner_path: "src/sibling.rs".to_owned(),
                content_digest: format!("blake3-256:{}", blake3::hash(&sibling_bytes).to_hex()),
                native_syntax_diagnostic: None,
                bytes: sibling_bytes,
                selectors: Vec::new(),
            },
        ],
        relations: Vec::new(),
    })
    .expect("workspace generation");
    generation
        .validate()
        .expect("execution binding participates in generation validation");
    assert_eq!(
        generation
            .runtime_provider_execution_binding
            .as_ref()
            .expect("execution binding persisted")
            .generation,
        runtime_provider_execution_binding.generation
    );

    let encoded =
        encode_workspace_search_generation_segment(&generation).expect("encode search generation");
    let segment =
        ValidatedSearchGenerationSegment::parse(&encoded).expect("validate search generation");
    let (bytes, _, count) = segment.section(SearchGenerationSectionKind::MerkleOwnerIndex);
    assert_eq!(count, 2);
    let table = ValidatedSortedRecordTable::parse(bytes).expect("parse Merkle owner table");
    let value = table
        .get_checked(b"src/lib.rs")
        .expect("lookup Merkle owner")
        .expect("Merkle owner record");
    let record: SearchMerkleOwnerRecord =
        serde_json::from_slice(value).expect("decode Merkle owner record");
    assert!(!record.inclusion_proof.is_empty());
    let source_blob_digest =
        agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1(
            &record.source_blob_digest,
        )
        .expect("source digest");
    let owner_subtree_digest =
        agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1(
            &record.owner_subtree_digest,
        )
        .expect("owner subtree digest");
    let root_digest =
        agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1(
            &generation.source_snapshot.root_digest,
        )
        .expect("root digest");
    assert!(
        agent_semantic_content_identity::workspace_merkle_v1::verify_owner_inclusion_v1(
            agent_semantic_content_identity::workspace_merkle_v1::WorkspaceOwnerInclusionV1 {
                owner_path: &record.owner_path,
                source_blob_digest: &source_blob_digest,
                expected_owner_subtree_digest: &owner_subtree_digest,
                inclusion_proof: &record.inclusion_proof,
                expected_workspace_root_digest: &root_digest,
            },
        )
    );
    let scenario_receipt = crate::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofQualificationReceipt::qualified(
        crate::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofEvidenceLayer::Scenario,
        "scenario.search-segment.merkle-owner-proof",
        None,
        "rust",
        "asp-rust",
        "rust://src/lib.rs#item/function/main",
        0,
        crate::runtime_resident_read::RuntimeResidentReadWorkCounters::default(),
        crate::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead::Owner {
            schema_id: "agent.semantic-protocols.workspace-runtime-merkle-owner-read.v1"
                .to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: generation.workspace_identity.clone(),
            project_root: generation.project_root.clone(),
            active_epoch: generation.active_epoch,
            generation_digest: generation.generation_digest.clone(),
            root_digest: generation.source_snapshot.root_digest.clone(),
            owner_path: record.owner_path.clone(),
            source_blob_digest: record.source_blob_digest.clone(),
            owner_subtree_digest: record.owner_subtree_digest.clone(),
            inclusion_proof: record.inclusion_proof.clone(),
        },
    )
    .expect("qualify Scenario Merkle owner proof receipt");
    assert_eq!(
        scenario_receipt.evidence_layer,
        crate::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofEvidenceLayer::Scenario
    );
    assert_eq!(scenario_receipt.runtime_ecosystem, "tokio");
    assert_eq!(scenario_receipt.read_mode, "synchronous-mmap");
    assert_eq!(scenario_receipt.work_counters, Default::default());
    assert_eq!(scenario_receipt.status, "qualified");
    let serialized = serde_json::to_value(&scenario_receipt).expect("serialize Scenario receipt");
    assert_eq!(serialized["evidenceLayer"], "scenario");
    assert_eq!(serialized["ownerPath"], "src/lib.rs");
    assert_eq!(
        serialized["ownerSubtreeDigest"],
        record.owner_subtree_digest
    );
    assert_eq!(serialized["proofStepCount"], record.inclusion_proof.len());
}
