use agent_semantic_content_identity::exact_selector_cache::{
    ExactSelectorMerkleLookupKeyV1, ExactSelectorMerkleMissV1, ExactSelectorProjectionRecordV1,
    ExactSelectorWarmSideEffectsV1,
};
use agent_semantic_content_identity::exact_selector_merkle::{
    ContentDigestV1, EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM, EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID,
    EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION, ExactProjectionModeV1, ExactSelectorMerkleProofV1,
    derive_parser_fact_digest_v1, derive_projection_digest_v1,
};
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
    let owner_path = "crates/example/src/lib.rs".to_owned();
    let source_blob_digest = digest('d');
    let tree = WorkspacePathMerkleTreeV1::from_file_digests([
        (owner_path.clone(), source_blob_digest.clone()),
        ("crates/other/src/lib.rs".to_owned(), digest('2')),
    ])
    .expect("valid Merkle tree");
    let parser_fact_digest = derive_parser_fact_digest_v1(
        "rust",
        &digest('e'),
        &digest('f'),
        &digest('d'),
        b"normalized-parser-facts",
    );
    let structural_selector = "rust://crates/example/src/lib.rs#item/function/run".to_owned();
    let canonical_item_selector =
        agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelectorV1::new(
            agent_semantic_content_identity::canonical_item_identity::CanonicalItemIdentityV1::new(
                "rust", "function", "run",
            ),
            structural_selector.as_str(),
        );
    let projection_digest = derive_projection_digest_v1(
        &canonical_item_selector,
        &structural_selector,
        ExactProjectionModeV1::Code,
        &parser_fact_digest,
        &projection_payload,
    );
    ExactSelectorProjectionRecordV1 {
        proof: ExactSelectorMerkleProofV1 {
            schema_id: EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_ID.to_owned(),
            schema_version: EXACT_SELECTOR_MERKLE_PROOF_SCHEMA_VERSION.to_owned(),
            digest_algorithm: EXACT_SELECTOR_MERKLE_DIGEST_ALGORITHM.to_owned(),
            language_id: "rust".to_owned(),
            workspace_root_digest: tree.root_digest().clone(),
            owner_path: owner_path.clone(),
            owner_subtree_digest: tree
                .owner_subtree_digest(&owner_path)
                .expect("owner leaf")
                .clone(),
            owner_inclusion_proof: tree.inclusion_proof(&owner_path).expect("owner proof"),
            source_blob_digest,
            parser_identity_digest: digest('e'),
            query_pack_digest: digest('f'),
            parser_fact_digest,
            canonical_item_selector,
            structural_selector,
            projection_mode: ExactProjectionModeV1::Code,
            projection_digest,
        },
        projection_payload,
    }
}

fn key<'a>(record: &'a ExactSelectorProjectionRecordV1) -> ExactSelectorMerkleLookupKeyV1<'a> {
    ExactSelectorMerkleLookupKeyV1 {
        language_id: &record.proof.language_id,
        workspace_root_digest: &record.proof.workspace_root_digest,
        owner_path: &record.proof.owner_path,
        owner_subtree_digest: &record.proof.owner_subtree_digest,
        source_blob_digest: &record.proof.source_blob_digest,
        parser_identity_digest: &record.proof.parser_identity_digest,
        query_pack_digest: &record.proof.query_pack_digest,
        structural_selector: &record.proof.structural_selector,
        projection_mode: record.proof.projection_mode,
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
    invalid_record.proof.workspace_root_digest = digest('9');
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
