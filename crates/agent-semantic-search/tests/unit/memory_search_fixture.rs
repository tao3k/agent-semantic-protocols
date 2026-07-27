use super::{
    MemorySearchBackendV1, MemorySearchFixtureV1, MemorySearchGenerationV1, MemorySearchItemV1,
    MemorySearchRequestV1, MemorySearchResolutionStateV1,
};
use agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelectorV1;
use std::sync::Arc;

fn generation(owner_paths: &[&str]) -> MemorySearchGenerationV1 {
    let source_leaves = owner_paths
        .iter()
        .map(
            |owner_path| crate::memory_search::MemorySearchSourceLeafV1 {
                owner_path: (*owner_path).to_owned(),
                owner_content_digest: "d".repeat(64),
            },
        )
        .collect::<Vec<_>>();
    let item_count = owner_paths.len();
    let root_digest = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        source_leaves
            .iter()
            .map(|leaf| (leaf.owner_path.as_str(), leaf.owner_content_digest.as_str())),
    )
    .root_digest()
    .to_owned();
    MemorySearchGenerationV1 {
        generation_id: "generation-1".to_owned(),
        root_digest,
        root_depth: if item_count <= 1 {
            0
        } else {
            usize::BITS as usize - (item_count - 1).leading_zeros() as usize
        },
        leaf_count: source_leaves.len(),
        owner_count: source_leaves.len(),
        selector_count: item_count,
        language_id: "rust".to_owned(),
        provider_id: "rs-harness".to_owned(),
        parser_identity_digest: "b".repeat(64),
        query_pack_digest: "c".repeat(64),
        source_leaves,
    }
}

fn selector(owner: &str, kind: &str) -> CanonicalItemSelectorV1 {
    CanonicalItemSelectorV1::parse(&format!("rust://{owner}#item/{kind}/include_deps"))
        .expect("canonical selector")
}

fn benchmark_contract() -> crate::memory_search_fixture::MemorySearchBenchmarkContractV1 {
    let benchmark = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/unit/scenarios/memory_search_fixture_v1/benchmark.toml"),
    )
    .expect("benchmark");
    crate::memory_search_fixture::MemorySearchBenchmarkContractV1::from_toml(&benchmark)
        .expect("typed benchmark contract")
}

fn item(owner: &str, kind: &str) -> MemorySearchItemV1 {
    MemorySearchItemV1 {
        owner_path: owner.to_owned(),
        owner_content_digest: "d".repeat(64),
        canonical_item_selector: selector(owner, kind),
    }
}

fn request(owner: &str, kind: &str) -> MemorySearchRequestV1 {
    MemorySearchRequestV1 {
        expected_generation_id: "generation-1".to_owned(),
        requested_owner_path: owner.to_owned(),
        canonical_item_selector: selector(owner, kind),
    }
}

#[test]
fn moved_item_uses_zero_io_memory_resolution() {
    let fixture = MemorySearchFixtureV1::new(generation(&["src/search_pipe_surfaces.rs"]))
        .with_item(item("src/search_pipe_surfaces.rs", "function"));
    let index = fixture.load_generation().expect("fixture");
    let result = index.resolve(&request("src/search_pipe_graph_turbo.rs", "function"));
    assert_eq!(result.state, MemorySearchResolutionStateV1::LiveRelocated);
    assert_eq!(result.generation.generation_id, "generation-1");
    assert_eq!(result.generation.root_depth, 0);
    assert_eq!(result.generation.leaf_count, 1);
    assert_eq!(result.generation.owner_count, 1);
    assert_eq!(result.generation.selector_count, 1);
    assert_eq!(
        result.resolved.expect("resolved").owner_path,
        "src/search_pipe_surfaces.rs"
    );
    benchmark_contract()
        .validate_receipt(&result.performance)
        .expect("performance receipt");
}

#[test]
fn fixture_projects_all_typed_resolution_states() {
    let live = MemorySearchFixtureV1::new(generation(&["src/live.rs"]))
        .with_item(item("src/live.rs", "function"))
        .build()
        .unwrap();
    assert_eq!(
        live.resolve(&request("src/live.rs", "function")).state,
        MemorySearchResolutionStateV1::LiveHit
    );

    let ambiguous = MemorySearchFixtureV1::new(generation(&["src/a.rs", "src/b.rs"]))
        .with_item(item("src/a.rs", "function"))
        .with_item(item("src/b.rs", "function"))
        .build()
        .unwrap();
    assert_eq!(
        ambiguous
            .resolve(&request("src/stale.rs", "function"))
            .state,
        MemorySearchResolutionStateV1::Ambiguous
    );
    assert_eq!(
        live.resolve(&request("src/live.rs", "struct")).state,
        MemorySearchResolutionStateV1::KindMismatch
    );
    assert_eq!(
        live.resolve(&request("src/live.rs", "enum")).state,
        MemorySearchResolutionStateV1::KindMismatch
    );
    let mut drift = request("src/live.rs", "function");
    drift.expected_generation_id = "generation-2".to_owned();
    assert_eq!(
        live.resolve(&drift).state,
        MemorySearchResolutionStateV1::GenerationMismatch
    );
    let empty = MemorySearchFixtureV1::new(generation(&[])).build().unwrap();
    let empty_result = empty.resolve(&request("src/missing.rs", "function"));
    assert_eq!(empty_result.state, MemorySearchResolutionStateV1::Missing);
    assert_eq!(empty_result.generation.root_depth, 0);
    assert_eq!(empty_result.generation.leaf_count, 0);
    assert_eq!(empty_result.generation.owner_count, 0);
    assert_eq!(empty_result.generation.selector_count, 0);
}

#[test]
fn immutable_generation_supports_concurrent_readers() {
    let index = Arc::new(
        MemorySearchFixtureV1::new(generation(&["src/search_pipe_surfaces.rs"]))
            .with_item(item("src/search_pipe_surfaces.rs", "function"))
            .build()
            .unwrap(),
    );
    let readers = (0..16)
        .map(|_| {
            let index = Arc::clone(&index);
            std::thread::spawn(move || {
                index.resolve(&request("src/search_pipe_graph_turbo.rs", "function"))
            })
        })
        .collect::<Vec<_>>();
    for reader in readers {
        let result = reader.join().expect("reader");
        assert_eq!(result.state, MemorySearchResolutionStateV1::LiveRelocated);
        benchmark_contract()
            .validate_receipt(&result.performance)
            .expect("concurrent performance receipt");
    }
}

#[test]
fn incomplete_generation_evidence_is_rejected_before_lookup() {
    let mut count_drift = generation(&["src/live.rs"]);
    count_drift.owner_count = 0;
    assert_eq!(
        MemorySearchFixtureV1::new(count_drift)
            .with_item(item("src/live.rs", "function"))
            .build()
            .unwrap_err(),
        "memory-search generation evidence mismatch"
    );

    let mut root_drift = generation(&["src/live.rs"]);
    root_drift.root_digest = "e".repeat(64);
    assert_eq!(
        MemorySearchFixtureV1::new(root_drift)
            .with_item(item("src/live.rs", "function"))
            .build()
            .unwrap_err(),
        "memory-search generation root mismatch"
    );

    let mut item_drift = item("src/live.rs", "function");
    item_drift.owner_content_digest = "f".repeat(64);
    assert_eq!(
        MemorySearchFixtureV1::new(generation(&["src/live.rs"]))
            .with_item(item_drift)
            .build()
            .unwrap_err(),
        "memory-search item evidence mismatch"
    );
}

#[test]
fn serialized_fixture_matches_shared_v1_schema() {
    let scoped_item = MemorySearchItemV1 {
        owner_path: "src/live.rs".to_owned(),
        owner_content_digest: "d".repeat(64),
        canonical_item_selector: CanonicalItemSelectorV1::parse(
            "rust://src/live.rs#item/method/parse/scope/implementation-owner/type/CliOptions",
        )
        .expect("scoped canonical selector"),
    };
    let fixture = MemorySearchFixtureV1::new(generation(&["src/live.rs"])).with_item(scoped_item);
    let schema_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/memory-search-fixture.v1.schema.json"),
    )
    .expect("shared schema");
    let schema = serde_json::from_str::<serde_json::Value>(&schema_source).expect("schema json");
    let instance = serde_json::to_value(fixture).expect("fixture json");
    jsonschema::validator_for(&schema)
        .expect("schema validator")
        .validate(&instance)
        .expect("fixture validates");
}

#[test]
fn turso_snapshot_materializes_the_shared_zero_io_backend() {
    let file_hashes =
        std::collections::BTreeMap::from([("src/live.rs".to_owned(), "d".repeat(64))]);
    let source_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        file_hashes
            .iter()
            .map(|(owner_path, digest)| (owner_path.as_str(), digest.as_str())),
    )
    .evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "e".repeat(64),
    );
    let snapshot = agent_semantic_client_db::engine::ClientDbSourceIndexGenerationSnapshotV1 {
        generation_id: "generation-1".to_owned(),
        file_hashes,
        source_snapshot,
        owner_count: 1,
        selector_count: 1,
        owners: vec![
            agent_semantic_client_db::engine::ClientDbSourceIndexGenerationOwnerV1 {
                owner_path: "src/live.rs".to_owned(),
                owner_content_digest: "d".repeat(64),
                language_id: Some("rust".to_owned()),
                provider_id: Some("rs-harness".to_owned()),
                source_kind: "source".to_owned(),
                line_count: Some(1),
                selectors: vec![
                    agent_semantic_client_db::engine::ClientDbSourceIndexSelectorFactV1 {
                        selector_id: "selector-1".to_owned(),
                        symbol: Some("include_deps".to_owned()),
                        kind: Some("function".to_owned()),
                        start_line: 1,
                        end_line: 1,
                        structural_selector: "rust://src/live.rs#item/function/include_deps"
                            .to_owned(),
                        payload_kind: None,
                        payload_bounded: true,
                        query_keys: vec!["include_deps".to_owned()],
                    },
                ],
            },
        ],
    };
    let backend = crate::memory_search_turso::TursoMemorySearchBackendV1::from_snapshot(
        snapshot,
        crate::memory_search_turso::TursoMemorySearchBindingV1 {
            language_id: "rust".to_owned(),
            provider_id: "rs-harness".to_owned(),
            parser_identity_digest: "b".repeat(64),
            query_pack_digest: "c".repeat(64),
        },
    )
    .expect("Turso memory backend");
    let index = backend.load_generation().expect("generation");
    let result = index.resolve(&request("src/stale.rs", "function"));
    assert_eq!(result.state, MemorySearchResolutionStateV1::LiveRelocated);
    assert_eq!(result.performance.db_opens, 0);
    assert_eq!(result.performance.db_queries, 0);
    assert_eq!(result.performance.source_bytes_materialized, 0);
    assert_eq!(result.performance.provider_subprocesses, 0);
}

#[test]
fn scenario_manifest_fixes_the_performance_contract() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/unit/scenarios/memory_search_fixture_v1");
    let scenario_source = std::fs::read_to_string(root.join("scenario.toml")).expect("scenario");
    let scenario =
        crate::memory_search_fixture::MemorySearchScenarioContractV1::from_toml(&scenario_source)
            .expect("typed scenario contract");
    assert_eq!(scenario.schema_id, "memory-search-fixture-v1");
    assert_eq!(scenario.schema_version, "1");
    assert_eq!(scenario.backend, "memory");
    assert_eq!(scenario.reader_count, 16);
    let contract = benchmark_contract();
    assert_eq!(contract.max_cold_lookup_micros, 1_000);
    assert_eq!(contract.max_warm_lookup_micros, 250);
    assert_eq!(contract.max_provider_process_count, 0);
    assert_eq!(contract.max_source_bytes_materialized, 0);
    assert_eq!(contract.max_db_opens, 0);
    assert_eq!(contract.max_db_queries, 0);
    assert_eq!(contract.max_cache_writes, 0);
}
