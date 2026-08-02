use std::sync::Arc;

use agent_semantic_content_identity::provider_projection_relation::{
    PROVIDER_RELATION_GENERATION_SCHEMA_ID, ProviderProjectedRelation,
    ProviderProjectedRelationEndpoint, ProviderRelationGeneration,
};
use agent_semantic_search::provider_relation_memory::{
    ProviderRelationMemoryError, ProviderRelationMemorySearch,
};

fn fixture() -> (Arc<[u8]>, [u8; 32], [u8; 32]) {
    let generation_digest = [7_u8; 32];
    let envelope = ProviderRelationGeneration {
        schema_id: PROVIDER_RELATION_GENERATION_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        generation_digest: format!("blake3-256:{}", "07".repeat(32)),
        relations: vec![ProviderProjectedRelation {
            from: ProviderProjectedRelationEndpoint {
                kind: "item".to_owned(),
                id: "item:caller".to_owned(),
            },
            kind: "calls".to_owned(),
            to: ProviderProjectedRelationEndpoint {
                kind: "item".to_owned(),
                id: "item:callee".to_owned(),
            },
        }],
    };
    let bytes: Arc<[u8]> = Arc::from(serde_json::to_vec(&envelope).expect("encode fixture"));
    let artifact_digest = *blake3::hash(&bytes).as_bytes();
    (bytes, generation_digest, artifact_digest)
}

#[test]
fn attached_relation_fixture_resolves_from_memory() {
    let (bytes, generation_digest, artifact_digest) = fixture();
    let search =
        ProviderRelationMemorySearch::attach_owned(bytes, generation_digest, artifact_digest)
            .expect("attach relation fixture");
    assert_eq!(search.relation_count(), 1);
    let relations = search.relations_from("item", "item:caller");
    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].kind, "calls");
    assert_eq!(relations[0].to.id, "item:callee");
}

#[test]
fn attached_relation_fixture_rejects_digest_drift() {
    let (bytes, generation_digest, _) = fixture();
    let error = ProviderRelationMemorySearch::attach_owned(bytes, generation_digest, [0; 32])
        .expect_err("digest drift must fail closed");
    assert_eq!(error, ProviderRelationMemoryError::ArtifactDigestMismatch);
}
