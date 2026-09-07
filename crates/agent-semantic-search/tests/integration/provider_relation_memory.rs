// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
                kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
                id: "item:caller".to_owned(),
            },
            kind: "calls".into(),
            to: ProviderProjectedRelationEndpoint {
                kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
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
    let relations = search.relations_from(
        agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
        "item:caller",
    );
    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].kind, "calls");
    assert_eq!(relations[0].to.id, "item:callee");
}

#[test]
fn hot_relation_lookup_is_sub_millisecond_without_runtime_io() {
    let (bytes, generation_digest, artifact_digest) = fixture();
    let search =
        ProviderRelationMemorySearch::attach_owned(bytes, generation_digest, artifact_digest)
            .expect("attach relation fixture");

    for _ in 0..256 {
        let started = std::time::Instant::now();
        let relations = search.relations_from(
            agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
            "item:caller",
        );
        let elapsed = started.elapsed();
        assert_eq!(relations.len(), 1);
        assert!(
            elapsed < std::time::Duration::from_millis(1),
            "resident relation lookup exceeded the sub-millisecond gate: {elapsed:?}"
        );
    }
}

#[test]
fn attached_relation_fixture_rejects_digest_drift() {
    let (bytes, generation_digest, _) = fixture();
    let error = ProviderRelationMemorySearch::attach_owned(bytes, generation_digest, [0; 32])
        .expect_err("digest drift must fail closed");
    assert_eq!(error, ProviderRelationMemoryError::ArtifactDigestMismatch);
}
