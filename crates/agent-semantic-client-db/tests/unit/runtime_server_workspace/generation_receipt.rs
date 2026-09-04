use agent_semantic_content_identity::{SourceSnapshotEvidence, project_resolution_schema_digest};
use agent_semantic_search::{
    ContentSearchGenerationReceipt, SearchGenerationConstructionStage, SearchGenerationIdentity,
    SearchGenerationStageReceipt, canonical_blake3_digest,
};

pub(crate) fn content_search_generation_receipt(
    project_id: &str,
    workspace_identity: &str,
    source_snapshot: &SourceSnapshotEvidence,
) -> ContentSearchGenerationReceipt {
    let identity = SearchGenerationIdentity {
        project_id: project_id.to_owned(),
        workspace_id: workspace_identity.to_owned(),
        source_root_digest: canonical_blake3_digest(&source_snapshot.root_digest)
            .expect("test source root digest"),
        provider_digest: canonical_blake3_digest(&source_snapshot.provider_digest)
            .expect("test provider digest"),
        schema_digest: canonical_blake3_digest(&project_resolution_schema_digest())
            .expect("test schema digest"),
        generation_candidate_digest: format!("blake3-256:{}", "c".repeat(64)),
    };
    let stage = |stage, worker_id: &str, byte: u8| SearchGenerationStageReceipt {
        stage,
        identity: identity.clone(),
        artifact_digest: format!("blake3-256:{}", char::from(byte).to_string().repeat(64)),
        worker_id: worker_id.to_owned(),
        complete: true,
    };
    ContentSearchGenerationReceipt::new(stage(
        SearchGenerationConstructionStage::SourceByteAcquisition,
        "test-source-byte-acquisition",
        b'd',
    ))
    .expect("test content search generation")
}
