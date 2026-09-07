// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleLookupKeyV1;
use agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleMissV1;
use agent_semantic_content_identity::exact_selector_cache::ExactSelectorProjectionRecordV1;
use agent_semantic_content_identity::exact_selector_cache::ExactSelectorWarmSideEffectsV1;
use agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1;
use agent_semantic_content_identity::exact_selector_merkle::EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM;
use agent_semantic_content_identity::exact_selector_merkle::EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID;
use agent_semantic_content_identity::exact_selector_merkle::EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION;
use agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1;
use agent_semantic_content_identity::exact_selector_merkle::ParserFactDigestInputV1;
use agent_semantic_content_identity::exact_selector_merkle::ParserLanguageIdV1;
use agent_semantic_content_identity::exact_selector_merkle::ProjectionDigestInputV1;
use agent_semantic_content_identity::exact_selector_merkle::derive_parser_fact_digest_v1;
use agent_semantic_content_identity::exact_selector_merkle::derive_projection_digest_v1;
use agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1;
use std::hint::black_box;
use std::time::Instant;

const SAMPLES: usize = 20;
const SINGLE_ITERATIONS: usize = 1_024;
const SINGLE_LOOKUP_P95_BUDGET_NS: u128 = 100_000;
const BATCH_LOOKUP_P95_BUDGET_NS: u128 = 10_000_000;

fn digest(character: char) -> ContentDigestV1 {
    ContentDigestV1::parse(character.to_string().repeat(64)).expect("valid digest")
}

fn record() -> ExactSelectorProjectionRecordV1 {
    let projection_payload = b"fn run() {}".to_vec();
    let language_id = ParserLanguageIdV1::from("rust");
    let owner_path = "crates/example/src/lib.rs".to_owned();
    let source_blob_digest = digest('d');
    let tree = WorkspacePathMerkleTreeV1::from_file_digests([
        (owner_path.clone(), source_blob_digest.clone()),
        ("crates/other/src/lib.rs".to_owned(), digest('2')),
    ])
    .expect("valid Merkle tree");
    let parser_fact_digest = derive_parser_fact_digest_v1(ParserFactDigestInputV1 {
        language_id: &language_id,
        parser_identity_digest: &digest('e'),
        query_pack_digest: &digest('f'),
        source_blob_digest: &digest('d'),
        normalized_parser_facts: b"normalized-parser-facts",
    });
    let structural_selector = "rust://crates/example/src/lib.rs#item/function/run".to_owned();
    let canonical_item_selector =
        agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelector::new(
            agent_semantic_content_identity::canonical_item_identity::CanonicalItemIdentity::new(
                "rust", "function", "run",
            ),
            structural_selector.clone(),
        );
    let projection_digest = derive_projection_digest_v1(ProjectionDigestInputV1 {
        canonical_item_selector: &canonical_item_selector,
        structural_selector: &structural_selector,
        projection_mode: ExactProjectionModeV1::Code,
        parser_fact_digest: &parser_fact_digest,
        projection_payload: &projection_payload,
    });
    ExactSelectorProjectionRecordV1 {
        source_byte_range: 0..projection_payload.len() as u64,
        proof: serde_json::from_value(serde_json::json!({
            "canonicalItemSelector": canonical_item_selector,
            "schemaId": EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID,
            "schemaVersion": EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION,
            "digestAlgorithm": EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM,
            "languageId": language_id,
            "workspaceRootDigest": tree.root_digest(),
            "ownerPath": owner_path,
            "ownerSubtreeDigest": tree
                .owner_subtree_digest(&owner_path)
                .expect("owner leaf"),
            "ownerInclusionProof": tree.inclusion_proof(&owner_path).expect("owner proof"),
            "sourceBlobDigest": source_blob_digest,
            "parserIdentityDigest": digest('e'),
            "queryPackDigest": digest('f'),
            "parserFactDigest": parser_fact_digest,
            "structuralSelector": structural_selector,
            "projectionMode": ExactProjectionModeV1::Code,
            "projectionDigest": projection_digest,
        }))
        .expect("valid exact-selector Merkle proof packet"),
        projection_payload,
    }
}

fn key<'a>(record: &'a ExactSelectorProjectionRecordV1) -> ExactSelectorMerkleLookupKeyV1<'a> {
    ExactSelectorMerkleLookupKeyV1 {
        language_id: record.proof.language_id(),
        workspace_root_digest: record.proof.workspace_root_digest(),
        owner_path: record.proof.owner_path(),
        owner_subtree_digest: record.proof.owner_subtree_digest(),
        source_blob_digest: record.proof.source_blob_digest(),
        parser_identity_digest: record.proof.parser_identity_digest(),
        query_pack_digest: record.proof.query_pack_digest(),
        structural_selector: record.proof.structural_selector(),
        projection_mode: record.proof.projection_mode().clone(),
    }
}

fn p95_ns(iterations: usize, mut operation: impl FnMut()) -> u128 {
    for _ in 0..iterations {
        operation();
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        for _ in 0..iterations {
            operation();
        }
        samples.push(started.elapsed().as_nanos() / iterations as u128);
    }
    samples.sort_unstable();
    samples[(SAMPLES * 95 / 100).min(SAMPLES - 1)]
}

fn main() {
    let record = record();
    let lookup_key = key(&record);
    let validated = agent_semantic_content_identity::exact_selector_cache::ValidatedExactSelectorProjectionV1::hydrate(
        record.clone(),
        &lookup_key,
    )
    .expect("hydrate validated warm projection");
    let single_p95_ns = p95_ns(SINGLE_ITERATIONS, || {
        let hit = black_box(&validated)
            .validate_warm_hit(black_box(&lookup_key))
            .expect("valid warm hit");
        assert_eq!(hit.side_effects, ExactSelectorWarmSideEffectsV1::ZERO);
    });
    println!(
        "[merkle-warm-bench] scenario=valid-hit p95Ns={} budgetNs={} parserProcessCount=0 contentStoreWriteCount=0 tursoWriteCount=0 manifestWriteCount=0 state=ok",
        single_p95_ns, SINGLE_LOOKUP_P95_BUDGET_NS
    );
    assert!(single_p95_ns <= SINGLE_LOOKUP_P95_BUDGET_NS);

    let mut invalid_record = record.clone();
    let mut invalid_proof =
        serde_json::to_value(&invalid_record.proof).expect("serializable proof");
    invalid_proof["workspaceRootDigest"] = serde_json::json!(digest('9'));
    invalid_record.proof = serde_json::from_value(invalid_proof).expect("deserializable proof");
    let invalid_key = key(&invalid_record);
    let miss_p95_ns = p95_ns(SINGLE_ITERATIONS, || {
        assert_eq!(
            black_box(&invalid_record).validate_warm_hit(black_box(&invalid_key)),
            Err(ExactSelectorMerkleMissV1::InvalidProofShape)
        );
    });
    println!(
        "[merkle-warm-bench] scenario=owner-proof-miss p95Ns={} budgetNs={} state=ok",
        miss_p95_ns, SINGLE_LOOKUP_P95_BUDGET_NS
    );
    assert!(miss_p95_ns <= SINGLE_LOOKUP_P95_BUDGET_NS);

    let batch_p95_ns = p95_ns(1, || {
        for _ in 0..1_024 {
            let hit = black_box(&validated)
                .validate_warm_hit(black_box(&lookup_key))
                .expect("valid warm hit");
            black_box(hit);
        }
    });
    println!(
        "[merkle-warm-bench] scenario=1024-valid-hits p95Ns={} budgetNs={} parserProcessCount=0 contentStoreWriteCount=0 tursoWriteCount=0 manifestWriteCount=0 state=ok",
        batch_p95_ns, BATCH_LOOKUP_P95_BUDGET_NS
    );
    assert!(batch_p95_ns <= BATCH_LOOKUP_P95_BUDGET_NS);
}
