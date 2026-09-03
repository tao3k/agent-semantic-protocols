use std::sync::{Arc, Barrier};

use agent_semantic_content_identity::exact_selector_generation_fixture::{
    ExactSelectorGenerationIdentityV1, ExactSelectorGenerationRecordV1,
    ExactSelectorProjectionModeV1,
};
use agent_semantic_search::active_exact_selector_fixture::ExactSelectorFixtureArtifactInput;
use agent_semantic_search::active_exact_selector_fixture::exact_selector_fixture_active_artifact_input_v1;
use agent_semantic_search::active_exact_selector_fixture::exact_selector_fixture_backend_from_active_artifact_v1;
use agent_semantic_search::exact_selector_fixture_memory::ExactSelectorFixtureResidentV1;
use agent_semantic_search::exact_selector_fixture_publication::publish_exact_selector_generation_records_v1;

fn required_activation_leaf()
-> agent_semantic_content_identity::active_artifact_merkle::ActiveArtifactLeaf {
    agent_semantic_content_identity::active_artifact_merkle::ActiveArtifactLeaf::new(
        agent_semantic_content_identity::active_artifact_merkle::ActiveArtifactLeafInput::new(
            "activation.json",
            "/tmp/activation.json",
            agent_semantic_content_identity::active_artifact_merkle::ActiveArtifactKind::Activation,
            agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1::parse(
                blake3::hash(b"activation").to_hex().to_string(),
            )
            .expect("activation content digest"),
        )
        .with_materialization_metadata(1, 1, None),
    )
    .expect("activation leaf")
}

macro_rules! binary_with_required_activation {
    ($binary:expr) => {
        vec![$binary, required_activation_leaf()]
    };
}

#[test]
fn active_receipt_without_exact_fixture_is_typed_generation_unavailable() {
    let receipt =
        agent_semantic_content_identity::active_artifact_merkle::ActiveAspArtifactReceipt::build(
            "empty-active-artifact-set",
            binary_with_required_activation![agent_semantic_content_identity::active_artifact_merkle::ActiveArtifactLeaf::new(
                agent_semantic_content_identity::active_artifact_merkle::ActiveArtifactLeafInput::new(
                    "bin/asp",
                    "/tmp/asp",
                    agent_semantic_content_identity::active_artifact_merkle::ActiveArtifactKind::AspBinary,
                    agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1::parse(
                        blake3::hash(b"asp-binary").to_hex().to_string(),
                    )
                    .expect("content digest"),
                )
                .with_materialization_metadata(1, 1, None),
            )
            .expect("asp binary leaf")],
        )
        .expect("build empty active artifact receipt");
    let error = exact_selector_fixture_active_artifact_input_v1(&receipt)
        .expect_err("receipt without exact fixture must fail closed");
    assert!(error.contains("state=generation-unavailable"));
    assert!(error.contains("reasonKind=active-fixture-missing"));
}

#[test]
fn concurrent_publication_commits_one_complete_generation() {
    let _performance_gate = super::performance_gate::lock();
    let started = std::time::Instant::now();
    let identity = Arc::new(ExactSelectorGenerationIdentityV1 {
        generation_digest: [9; 32],
        language_id: "rust".to_owned(),
        leaf_count: 1,
        owner_count: 1,
        parser_identity_digest: [8; 32],
        provider_id: "asp-rust".to_owned(),
        query_pack_digest: [7; 32],
        selector_count: 1,
        workspace_identity_digest: [6; 32],
        workspace_root_digest: [5; 32],
    });
    let record = ExactSelectorGenerationRecordV1 {
        structural_selector: "rust://src/lib.rs#item/function/run".to_owned(),
        owner_path: "src/lib.rs".to_owned(),
        source_blob_digest: [1; 32],
        owner_subtree_digest: [2; 32],
        normalized_parser_facts_digest: [3; 32],
        projection_mode: ExactSelectorProjectionModeV1::Source,
        source_byte_range: 0..32,
        projection: vec![0x5a; 32],
    };
    let test_root = std::env::temp_dir().join(format!(
        "asp-exact-selector-publication-gate-{}",
        std::process::id()
    ));
    let barrier = Arc::new(Barrier::new(32));

    let workers = (0..32)
        .map(|_| {
            let identity = Arc::clone(&identity);
            let record = record.clone();
            let test_root = test_root.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                publish_exact_selector_generation_records_v1(
                    &test_root,
                    identity.as_ref(),
                    vec![record],
                )
            })
        })
        .collect::<Vec<_>>();
    let receipts = workers
        .into_iter()
        .map(|worker| {
            worker
                .join()
                .expect("publication worker panicked")
                .expect("publication worker failed")
        })
        .collect::<Vec<_>>();

    assert!(receipts.windows(2).all(|pair| pair[0] == pair[1]));
    let active_artifact_input = ExactSelectorFixtureArtifactInput::from(&receipts[0]);
    assert_eq!(active_artifact_input.logical_path, receipts[0].logical_path);
    assert_eq!(
        active_artifact_input.materialized_path,
        receipts[0].materialized_path
    );
    assert_eq!(
        active_artifact_input.artifact_kind,
        agent_semantic_content_identity::active_artifact_merkle::ActiveArtifactKind::ExactSelectorGenerationFixture
    );
    assert_eq!(
        active_artifact_input.artifact_digest,
        receipts[0].artifact_digest
    );
    let backend = exact_selector_fixture_backend_from_active_artifact_v1(&active_artifact_input)
        .expect("active artifact should construct exact selector backend");
    assert_eq!(backend.fixture_path(), receipts[0].materialized_path);
    let resident = ExactSelectorFixtureResidentV1::new(backend);
    let cold_started = std::time::Instant::now();
    let cold_projection = resident
        .resolve("rust://src/lib.rs#item/function/run")
        .expect("cold active artifact lookup")
        .expect("cold active artifact selector hit");
    let cold_micros = cold_started.elapsed().as_micros();
    let warm_started = std::time::Instant::now();
    let warm_projection = resident
        .resolve("rust://src/lib.rs#item/function/run")
        .expect("warm active artifact lookup")
        .expect("warm active artifact selector hit");
    let warm_micros = warm_started.elapsed().as_micros();
    assert_eq!(cold_projection.as_bytes(), warm_projection.as_bytes());
    assert!(warm_projection.matches_generation_authority_v1(
        "rust",
        "asp-rust",
        &blake3::Hash::from_bytes([8; 32]).to_hex().to_string(),
        &blake3::Hash::from_bytes([7; 32]).to_hex().to_string(),
    ));
    assert!(!warm_projection.matches_generation_authority_v1(
        "rust",
        "asp-rust",
        &blake3::Hash::from_bytes([0; 32]).to_hex().to_string(),
        &blake3::Hash::from_bytes([7; 32]).to_hex().to_string(),
    ));
    assert_eq!(warm_projection.workspace_root_digest(), &[5; 32]);
    assert_eq!(warm_projection.workspace_identity_digest(), &[6; 32]);
    assert_eq!(warm_projection.generation_digest(), &[9; 32]);
    eprintln!("activeArtifactLookup coldMicros={cold_micros} warmMicros={warm_micros}");
    assert!(
        cold_micros <= 1_000,
        "active artifact cold lookup exceeded 1ms gate: {cold_micros}us"
    );
    assert!(
        warm_micros <= 100,
        "active artifact warm lookup exceeded 100us gate: {warm_micros}us"
    );
    let mut malformed_locator = ExactSelectorFixtureArtifactInput::from(&receipts[0]);
    malformed_locator.logical_path = "exact-selector-generation/missing.fixture".to_owned();
    assert!(
        exact_selector_fixture_backend_from_active_artifact_v1(&malformed_locator).is_err(),
        "non-canonical exact selector locator must fail closed"
    );
    let mut wrong_generation = ExactSelectorFixtureArtifactInput::from(&receipts[0]);
    wrong_generation.logical_path = format!(
        "exact-selector-generation/{}/{}.fixture",
        blake3::Hash::from_bytes([0; 32]).to_hex(),
        receipts[0].artifact_digest
    );
    let wrong_generation_backend =
        exact_selector_fixture_backend_from_active_artifact_v1(&wrong_generation)
            .expect("canonical locator should construct backend");
    let wrong_generation_resident = ExactSelectorFixtureResidentV1::new(wrong_generation_backend);
    assert!(
        wrong_generation_resident
            .resolve("rust://src/lib.rs#item/function/run")
            .is_err(),
        "generation digest mismatch must fail closed"
    );
    let artifact = std::fs::read(&receipts[0].materialized_path).expect("read committed fixture");
    assert_eq!(
        blake3::hash(&artifact).to_hex().to_string(),
        receipts[0].artifact_digest
    );
    let generation_root = receipts[0]
        .materialized_path
        .parent()
        .expect("generation artifact parent");
    let entries = std::fs::read_dir(&generation_root)
        .expect("read generation root")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect generation entries");
    assert_eq!(entries.len(), 1, "temporary publication artifact leaked");
    let elapsed_micros = started.elapsed().as_micros();
    eprintln!(
        "publicationPerformance callers=32 elapsedMicros={}",
        elapsed_micros
    );
    assert!(
        elapsed_micros <= 75_000,
        "32-caller durable publication exceeded 75ms gate: {elapsed_micros}us"
    );
    std::fs::remove_dir_all(test_root).expect("remove publication test root");
}
