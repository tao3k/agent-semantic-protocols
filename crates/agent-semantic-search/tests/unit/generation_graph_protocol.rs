use agent_semantic_content_identity::SourceSnapshotEvidence;
use agent_semantic_content_identity::SourceSnapshotKind;
use agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation;
use agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint;
use agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1;

use crate::ContentSearchGenerationReceipt;
use crate::SEARCH_GENERATION_GRAPH_RECEIPT_SCHEMA_ID;
use crate::SearchGenerationConstructionStage;
use crate::SearchGenerationGraphReceipt;
use crate::SearchGenerationGraphRequest;
use crate::SearchGenerationIdentity;
use crate::SearchGenerationStageReceipt;
use crate::build_resident_graph_generation;
use crate::canonical_blake3_digest;
use crate::open_resident_graph_generation;
use crate::resident_graph_search::materialize_resident_graph_generation;
use crate::stable_graph_node_id;

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn relation() -> ProviderProjectedRelation {
    ProviderProjectedRelation {
        from: ProviderProjectedRelationEndpoint {
            kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
            id: "src/lib.rs".to_owned(),
        },
        kind: "contains".into(),
        to: ProviderProjectedRelationEndpoint {
            kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
            id: "SearchGeneration::admit".to_owned(),
        },
    }
}

fn content_generation(identity: &SearchGenerationIdentity) -> ContentSearchGenerationReceipt {
    ContentSearchGenerationReceipt::new(SearchGenerationStageReceipt {
        stage: SearchGenerationConstructionStage::SourceByteAcquisition,
        identity: identity.clone(),
        artifact_digest: digest('e'),
        worker_id: "acquisition".to_owned(),
        complete: true,
    })
    .unwrap()
}

fn request() -> SearchGenerationGraphRequest {
    let source_snapshot = SourceSnapshotEvidence::new(
        "a".repeat(64),
        SourceSnapshotKind::Filesystem,
        1,
        digest('b'),
    );
    let identity = SearchGenerationIdentity {
        project_id: "project-test".to_owned(),
        workspace_id: "workspace-test".to_owned(),
        source_root_digest: canonical_blake3_digest(&source_snapshot.root_digest).unwrap(),
        provider_digest: canonical_blake3_digest(&source_snapshot.provider_digest).unwrap(),
        schema_digest: digest('c'),
        generation_candidate_digest: digest('d'),
    };
    SearchGenerationGraphRequest::new(
        &content_generation(&identity),
        source_snapshot,
        WorkspaceGenerationEvidenceV1 {
            root_digest: "a".repeat(64),
            root_depth: 1,
            leaf_count: 1,
            owner_count: 1,
        },
        ["src/lib.rs".to_owned()],
        [relation()],
    )
    .unwrap()
}

fn receipt(request: &SearchGenerationGraphRequest) -> SearchGenerationGraphReceipt {
    let graph = materialize_resident_graph_generation(
        &request.source_snapshot,
        &request.workspace_generation,
        request.owner_paths.iter().cloned(),
        request.relations.iter().cloned(),
    )
    .unwrap();
    SearchGenerationGraphReceipt {
        schema_id: SEARCH_GENERATION_GRAPH_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        identity: request.identity.clone(),
        content_generation_digest: request.content_generation_digest.clone(),
        entry_owner_ids: request.owner_paths.clone(),
        entry_node_ids: vec![stable_graph_node_id("owner", "src/lib.rs")],
        candidate_owner_ids: request.owner_paths.clone(),
        artifact_digest: graph.digest().to_owned(),
        complete: true,
    }
}

#[test]
fn rust_constructs_base_resident_graph_without_python_receipt() {
    let request = std::sync::Arc::new(request());
    let graph = build_resident_graph_generation(std::sync::Arc::clone(&request)).unwrap();
    assert_eq!(graph.digest(), receipt(&request).artifact_digest);
}

#[test]
fn v1_graph_request_accepts_complete_overlay_lineage() {
    let mut request = request();
    request.source_snapshot.base_root_digest = Some("e".repeat(64));
    request.source_snapshot.dirty_paths_digest = Some("f".repeat(64));
    request.validate().unwrap();
}

#[test]
fn v1_graph_request_rejects_half_overlay_lineage() {
    let mut request = request();
    request.source_snapshot.base_root_digest = Some("e".repeat(64));
    let error = request.validate().unwrap_err();
    assert_eq!(
        error,
        "search generation graph overlay evidence is incomplete"
    );
}

#[test]
fn optional_python_receipt_cannot_change_rust_base_graph_digest() {
    let request = std::sync::Arc::new(request());
    let mut receipt = receipt(&request);
    receipt.artifact_digest = digest('f');
    receipt.validate_for(&request).unwrap();
    let graph = build_resident_graph_generation(std::sync::Arc::clone(&request)).unwrap();
    assert_ne!(graph.digest(), receipt.artifact_digest);
}

#[test]
fn graph_receipt_from_another_content_generation_fails_closed() {
    let request = request();
    let mut receipt = receipt(&request);
    receipt.content_generation_digest = digest('9');
    let error = receipt
        .validate_for(&request)
        .expect_err("foreign content generation must not attach");
    assert_eq!(
        error,
        "search generation graph receipt content generation drift"
    );
}

#[test]
fn persisted_resident_stage_is_required_to_reopen_resident_graph() {
    let request = request();
    let receipt = receipt(&request);
    let stage = SearchGenerationStageReceipt {
        stage: SearchGenerationConstructionStage::ResidentGraph,
        identity: request.identity.clone(),
        artifact_digest: receipt.artifact_digest.clone(),
        worker_id: "rust-resident-graph-v1".to_owned(),
        complete: true,
    };
    let graph = open_resident_graph_generation(
        &content_generation(&request.identity),
        &request.source_snapshot,
        &request.workspace_generation,
        request.owner_paths.iter().cloned(),
        request.relations.iter().cloned(),
        &stage,
    )
    .unwrap();
    assert_eq!(graph.digest(), receipt.artifact_digest);
}

#[test]
fn persisted_non_resident_stage_cannot_open_resident_graph() {
    let request = request();
    let receipt = receipt(&request);
    let stage = SearchGenerationStageReceipt {
        stage: SearchGenerationConstructionStage::NativeSyntax,
        identity: request.identity.clone(),
        artifact_digest: receipt.artifact_digest,
        worker_id: "provider-native-syntax-playbook-v1".to_owned(),
        complete: true,
    };
    let error = open_resident_graph_generation(
        &content_generation(&request.identity),
        &request.source_snapshot,
        &request.workspace_generation,
        request.owner_paths.iter().cloned(),
        request.relations.iter().cloned(),
        &stage,
    )
    .unwrap_err();
    assert!(error.contains("stage is incomplete"));
}
