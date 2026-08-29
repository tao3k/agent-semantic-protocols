use super::{
    DIGEST_LEN, ExactSelectorGenerationFixtureErrorV1, ExactSelectorGenerationFixtureViewV1,
    ExactSelectorGenerationIdentityV1, ExactSelectorGenerationRecordV1,
    ExactSelectorProjectionModeV1, build_exact_selector_generation_fixture_v1, fixture_digest_v1,
};
use crate::{
    CanonicalItemSelector, ExactSelectorMaterializationProofErrorV1,
    ExactSelectorMaterializationProofV1,
};

fn digest(byte: u8) -> [u8; DIGEST_LEN] {
    [byte; DIGEST_LEN]
}

fn materialization_proof() -> ExactSelectorMaterializationProofV1 {
    let projection = b"fn run() {}".to_vec();
    let structural_selector = "rust://src/lib.rs#item/function/run".to_string();
    ExactSelectorMaterializationProofV1 {
        language_id: "rust".to_string(),
        provider_id: "asp-rust".to_string(),
        canonical_item_selector: CanonicalItemSelector::parse(&structural_selector)
            .expect("canonical selector"),
        parser_identity_digest: digest(1),
        query_pack_digest: digest(2),
        workspace_root_digest: digest(3),
        owner_path: "src/lib.rs".to_string(),
        owner_subtree_digest: digest(3),
        owner_inclusion_proof: Vec::new(),
        source_blob_digest: digest(4),
        normalized_parser_facts_digest: digest(5),
        structural_selector,
        projection_mode: ExactSelectorProjectionModeV1::Source,
        source_byte_start: 0,
        source_byte_end: 11,
        projection_digest: *blake3::hash(&projection).as_bytes(),
        projection,
    }
}

#[test]
fn materialization_proof_converts_without_source_or_line_reconstruction() {
    let proof = materialization_proof();
    let record = ExactSelectorGenerationRecordV1::try_from(&proof).expect("proof");
    assert_eq!(record.structural_selector, proof.structural_selector);
    assert_eq!(record.owner_path, proof.owner_path);
    assert_eq!(
        record.source_byte_range,
        proof.source_byte_start..proof.source_byte_end
    );
    assert_eq!(record.projection, proof.projection);
}

#[test]
fn materialization_proof_rejects_selector_mismatch() {
    let mut proof = materialization_proof();
    proof.canonical_item_selector.structural_selector =
        "rust://src/lib.rs#item/function/other".to_string();
    assert_eq!(
        ExactSelectorGenerationRecordV1::try_from(&proof),
        Err(ExactSelectorMaterializationProofErrorV1::InvalidCanonicalSelector)
    );
}

#[test]
fn materialization_proof_rejects_zero_digest() {
    let mut proof = materialization_proof();
    proof.source_blob_digest = [0_u8; DIGEST_LEN];
    assert_eq!(
        ExactSelectorGenerationRecordV1::try_from(&proof),
        Err(ExactSelectorMaterializationProofErrorV1::InvalidDigest)
    );
}

#[test]
fn materialization_proof_rejects_invalid_source_byte_range() {
    let mut proof = materialization_proof();
    proof.source_byte_start = proof.source_byte_end;
    assert_eq!(
        ExactSelectorGenerationRecordV1::try_from(&proof),
        Err(ExactSelectorMaterializationProofErrorV1::InvalidSourceByteRange)
    );
}

#[test]
fn materialization_proof_rejects_projection_digest_mismatch() {
    let mut proof = materialization_proof();
    proof.projection_digest = digest(9);
    assert_eq!(
        ExactSelectorGenerationRecordV1::try_from(&proof),
        Err(ExactSelectorMaterializationProofErrorV1::ProjectionDigestMismatch)
    );
}

#[test]
fn materialization_proof_rejects_empty_projection() {
    let mut proof = materialization_proof();
    proof.projection.clear();
    proof.projection_digest = *blake3::hash(&proof.projection).as_bytes();
    assert_eq!(
        ExactSelectorGenerationRecordV1::try_from(&proof),
        Err(ExactSelectorMaterializationProofErrorV1::EmptyProjection)
    );
}

#[test]
fn materialization_proof_rejects_workspace_root_mismatch() {
    let mut proof = materialization_proof();
    proof.workspace_root_digest = digest(8);
    assert_eq!(
        ExactSelectorGenerationRecordV1::try_from(&proof),
        Err(ExactSelectorMaterializationProofErrorV1::WorkspaceRootDigestMismatch)
    );
}

fn fixture() -> (Vec<u8>, [u8; DIGEST_LEN]) {
    let generation_digest = digest(4);
    let bytes = build_exact_selector_generation_fixture_v1(
        &ExactSelectorGenerationIdentityV1 {
            workspace_identity_digest: [9_u8; DIGEST_LEN],
            language_id: "rust".to_string(),
            provider_id: "asp-rust".to_string(),
            workspace_root_digest: digest(1),
            parser_identity_digest: digest(2),
            query_pack_digest: digest(3),
            generation_digest,
            selector_count: 1,
            owner_count: 1,
            leaf_count: 1,
        },
        vec![ExactSelectorGenerationRecordV1 {
            structural_selector: "rust://src/lib.rs#item/function/run".to_string(),
            owner_path: "src/lib.rs".to_string(),
            owner_subtree_digest: digest(5),
            source_blob_digest: digest(6),
            normalized_parser_facts_digest: digest(7),
            projection_mode: ExactSelectorProjectionModeV1::Source,
            source_byte_range: 0..11,
            projection: b"fn run() {}".to_vec(),
        }],
    )
    .expect("fixture");
    (bytes, generation_digest)
}

#[test]
fn exact_lookup_reads_immutable_projection() {
    let (bytes, generation_digest) = fixture();
    let fixture_digest = *fixture_digest_v1(&bytes).expect("fixture digest");
    let view =
        ExactSelectorGenerationFixtureViewV1::attach(&bytes, &generation_digest, &fixture_digest)
            .expect("attach");
    let record = view
        .lookup("rust://src/lib.rs#item/function/run")
        .expect("lookup")
        .expect("hit");
    assert_eq!(record.projection, b"fn run() {}");
    assert_eq!(record.owner_path, "src/lib.rs");
}

#[test]
fn absent_selector_is_a_closed_miss() {
    let (bytes, generation_digest) = fixture();
    let fixture_digest = *fixture_digest_v1(&bytes).expect("fixture digest");
    let view =
        ExactSelectorGenerationFixtureViewV1::attach(&bytes, &generation_digest, &fixture_digest)
            .expect("attach");
    assert_eq!(
        view.lookup("rust://src/lib.rs#item/function/missing")
            .expect("lookup"),
        None
    );
}

#[test]
fn incomplete_generation_is_rejected() {
    let error = build_exact_selector_generation_fixture_v1(
        &ExactSelectorGenerationIdentityV1 {
            workspace_identity_digest: [9_u8; DIGEST_LEN],
            language_id: "rust".to_string(),
            provider_id: "asp-rust".to_string(),
            workspace_root_digest: digest(1),
            parser_identity_digest: digest(2),
            query_pack_digest: digest(3),
            generation_digest: digest(4),
            selector_count: 1,
            owner_count: 84,
            leaf_count: 85,
        },
        vec![ExactSelectorGenerationRecordV1 {
            structural_selector: "rust://src/lib.rs#item/function/run".to_string(),
            owner_path: "src/lib.rs".to_string(),
            owner_subtree_digest: digest(5),
            source_blob_digest: digest(6),
            normalized_parser_facts_digest: digest(7),
            projection_mode: ExactSelectorProjectionModeV1::Source,
            source_byte_range: 0..11,
            projection: b"fn run() {}".to_vec(),
        }],
    )
    .expect_err("incomplete generation");
    assert_eq!(
        error,
        ExactSelectorGenerationFixtureErrorV1::IncompleteGeneration {
            record_count: 1,
            selector_count: 1,
            owner_count: 84,
            leaf_count: 85,
        }
    );
}

#[test]
fn partial_selector_materialization_is_rejected() {
    let error = build_exact_selector_generation_fixture_v1(
        &ExactSelectorGenerationIdentityV1 {
            workspace_identity_digest: [9_u8; DIGEST_LEN],
            language_id: "rust".to_string(),
            provider_id: "asp-rust".to_string(),
            workspace_root_digest: digest(1),
            parser_identity_digest: digest(2),
            query_pack_digest: digest(3),
            generation_digest: digest(4),
            selector_count: 2,
            owner_count: 1,
            leaf_count: 1,
        },
        vec![ExactSelectorGenerationRecordV1 {
            structural_selector: "rust://src/lib.rs#item/function/run".to_string(),
            owner_path: "src/lib.rs".to_string(),
            owner_subtree_digest: digest(5),
            source_blob_digest: digest(6),
            normalized_parser_facts_digest: digest(7),
            projection_mode: ExactSelectorProjectionModeV1::Source,
            source_byte_range: 0..11,
            projection: b"fn run() {}".to_vec(),
        }],
    )
    .expect_err("partial selector generation");
    assert_eq!(
        error,
        ExactSelectorGenerationFixtureErrorV1::IncompleteGeneration {
            record_count: 1,
            selector_count: 2,
            owner_count: 1,
            leaf_count: 1,
        }
    );
}
