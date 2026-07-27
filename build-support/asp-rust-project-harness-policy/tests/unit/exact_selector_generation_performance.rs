use std::sync::Arc;
use std::thread;

use rust_lang_project_harness::exact_selector_generation_fixture::{
    ExactSelectorGenerationFixtureViewV1, ExactSelectorGenerationIdentityV1,
    ExactSelectorGenerationRecordV1, ExactSelectorProjectionModeV1,
    build_registered_rust_exact_selector_fixture_v1, fixture_digest_v1,
    measure_registered_rust_exact_selector_fixture_v1,
};
use rust_lang_project_harness::exact_selector_generation_fixture::{
    WorkspaceSearchIdentityInputV1, WorkspaceSearchIdentityV1, WorkspaceSearchScopeKindV1,
};

const RECORD_COUNT: usize = 4_096;
const HIT_INDEX: usize = RECORD_COUNT / 2;

fn digest(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn selector(index: usize) -> String {
    format!("rust://src/lib.rs#item/function/item_{index:04}")
}

fn fixture(generation_byte: u8) -> (Vec<u8>, [u8; 32], WorkspaceSearchIdentityV1) {
    let generation_digest = digest(generation_byte);
    let workspace_identity = WorkspaceSearchIdentityV1::admit(WorkspaceSearchIdentityInputV1 {
        language_id: "rust".to_string(),
        provider_id: "rs-harness".to_string(),
        scope_kind: WorkspaceSearchScopeKindV1::Package,
        requested_discovery_root: "/repo/crates/client-db".into(),
        cargo_workspace_root: "/repo".into(),
        selected_package_root: Some("/repo/crates/client-db".into()),
        provider_tool_root: "/providers/rs-harness".into(),
        envelope_root: "/repo/crates/client-db".into(),
        root_count: 1,
        owner_count: 1,
        leaf_count: 1,
        selector_count: u32::try_from(RECORD_COUNT).expect("selector count"),
        source_snapshot_root_digest: digest(1),
    })
    .expect("workspace search identity");
    let records = (0..RECORD_COUNT)
        .map(|index| {
            let projection = format!("pub fn item_{index:04}() {{}}\n").into_bytes();
            ExactSelectorGenerationRecordV1 {
                structural_selector: selector(index),
                owner_path: "src/lib.rs".to_string(),
                owner_subtree_digest: digest(5),
                source_blob_digest: digest(6),
                normalized_parser_facts_digest: digest(7),
                projection_mode: ExactSelectorProjectionModeV1::Code,
                source_byte_range: 0..u64::try_from(projection.len()).expect("projection length"),
                projection,
            }
        })
        .collect();
    let bytes = build_registered_rust_exact_selector_fixture_v1(
        &workspace_identity,
        ExactSelectorGenerationIdentityV1 {
            language_id: "rust".to_string(),
            provider_id: "rs-harness".to_string(),
            workspace_root_digest: digest(1),
            parser_identity_digest: digest(2),
            query_pack_digest: digest(3),
            generation_digest,
            selector_count: u32::try_from(RECORD_COUNT).expect("selector count"),
            owner_count: 1,
            leaf_count: 1,
        },
        records,
    )
    .expect("registered Rust generation fixture");
    (bytes, generation_digest, workspace_identity)
}

#[test]
fn fixtool_gates_cold_warm_and_absent_exact_selector_latency() {
    let (fixture, generation_digest, workspace_identity) = fixture(4);
    let receipt = measure_registered_rust_exact_selector_fixture_v1(
        &fixture,
        &workspace_identity,
        generation_digest,
        &selector(HIT_INDEX),
        "rust://src/lib.rs#item/function/absent",
        1_024,
    )
    .expect("performance receipt");
    eprintln!(
        "[exact-selector-performance] coldMicros={} warmP95Micros={} missMicros={} subprocesses={} dbOpens={} sourceFilesRead={} sourceBytesRead={} manifestWrites={}",
        receipt.cold_micros,
        receipt.warm_p95_micros,
        receipt.miss_micros,
        receipt.subprocesses,
        receipt.db_opens,
        receipt.source_files_read,
        receipt.source_bytes_read,
        receipt.manifest_writes,
    );
    receipt.enforce_v1().expect("V1 performance budget");
}

#[test]
fn fixtool_gates_parallel_readers_without_database_or_lock_authority() {
    let (fixture, generation_digest, _) = fixture(4);
    let fixture_digest = *fixture_digest_v1(&fixture).expect("fixture digest");
    let fixture = Arc::new(fixture);
    let selector = Arc::new(selector(HIT_INDEX));
    let readers = (0..16)
        .map(|_| {
            let fixture = Arc::clone(&fixture);
            let selector = Arc::clone(&selector);
            thread::spawn(move || {
                let view = ExactSelectorGenerationFixtureViewV1::attach(
                    fixture.as_slice(),
                    &generation_digest,
                    &fixture_digest,
                )
                .expect("parallel attach");
                for _ in 0..1_000 {
                    let hit = view
                        .lookup(selector.as_str())
                        .expect("parallel lookup")
                        .expect("parallel hit");
                    assert_eq!(hit.owner_path, "src/lib.rs");
                }
            })
        })
        .collect::<Vec<_>>();
    for reader in readers {
        reader.join().expect("parallel reader");
    }
}

#[test]
fn fixtool_gates_generation_replacement_with_pinned_readers() {
    let (first_fixture, first_generation, _) = fixture(4);
    let first_fixture_digest = *fixture_digest_v1(&first_fixture).expect("first fixture digest");
    let first_view = ExactSelectorGenerationFixtureViewV1::attach(
        &first_fixture,
        &first_generation,
        &first_fixture_digest,
    )
    .expect("first generation");

    let (next_fixture, next_generation, _) = fixture(8);
    let next_fixture_digest = *fixture_digest_v1(&next_fixture).expect("next fixture digest");
    let next_view = ExactSelectorGenerationFixtureViewV1::attach(
        &next_fixture,
        &next_generation,
        &next_fixture_digest,
    )
    .expect("next generation");

    assert_eq!(first_view.generation_digest(), &first_generation);
    assert_eq!(next_view.generation_digest(), &next_generation);
    assert_ne!(
        first_view.generation_digest(),
        next_view.generation_digest()
    );
    assert!(
        first_view
            .lookup(&selector(HIT_INDEX))
            .expect("old lookup")
            .is_some()
    );
    assert!(
        next_view
            .lookup(&selector(HIT_INDEX))
            .expect("new lookup")
            .is_some()
    );
}
