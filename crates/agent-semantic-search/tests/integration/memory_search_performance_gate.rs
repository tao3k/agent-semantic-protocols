use agent_semantic_content_identity::{
    WorkspaceSnapshot, canonical_item_identity::CanonicalItemSelectorV1,
};
use agent_semantic_search::{
    MemorySearchItemV1, MemorySearchResidentV1,
    memory_search::{
        MemorySearchBackendV1, MemorySearchGenerationV1, MemorySearchIndexV1,
        MemorySearchRequestV1, MemorySearchResolutionStateV1, MemorySearchSourceLeafV1,
    },
    memory_search_fixture::{MemorySearchBenchmarkContractV1, MemorySearchFixtureV1},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

struct CountingBackend {
    index: MemorySearchIndexV1,
    loads: Arc<AtomicUsize>,
}

impl MemorySearchBackendV1 for CountingBackend {
    fn load_generation(&self) -> Result<MemorySearchIndexV1, String> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(self.index.clone())
    }
}

fn fixture() -> (MemorySearchIndexV1, MemorySearchRequestV1) {
    let owner_path = "src/lib.rs";
    let owner_content_digest = "1".repeat(64);
    let snapshot =
        WorkspaceSnapshot::from_file_hashes([(owner_path, owner_content_digest.as_str())]);
    let selector =
        CanonicalItemSelectorV1::parse("rust://src/lib.rs#item/function/run_language_command")
            .expect("fixture selector must be canonical");
    let generation_id = "memory-search-performance-gate".to_string();
    let generation = MemorySearchGenerationV1 {
        generation_id: generation_id.clone(),
        root_digest: snapshot.root_digest().to_string(),
        root_depth: 0,
        leaf_count: 1,
        owner_count: 1,
        selector_count: 1,
        language_id: "rust".to_string(),
        provider_id: "rs-harness".to_string(),
        parser_identity_digest: "2".repeat(64),
        query_pack_digest: "3".repeat(64),
        source_leaves: vec![MemorySearchSourceLeafV1 {
            owner_path: owner_path.to_string(),
            owner_content_digest: owner_content_digest.clone(),
        }],
    };
    let index = MemorySearchFixtureV1::new(generation)
        .with_item(MemorySearchItemV1 {
            owner_path: owner_path.to_string(),
            owner_content_digest,
            canonical_item_selector: selector.clone(),
        })
        .build()
        .expect("fixture generation must build");
    let request = MemorySearchRequestV1 {
        expected_generation_id: generation_id,
        requested_owner_path: owner_path.to_string(),
        canonical_item_selector: selector,
    };
    (index, request)
}

fn hard_contract() -> MemorySearchBenchmarkContractV1 {
    MemorySearchBenchmarkContractV1 {
        max_cold_lookup_micros: 1_000,
        max_warm_lookup_micros: 100,
        max_provider_process_count: 0,
        max_source_bytes_materialized: 0,
        max_db_opens: 0,
        max_db_queries: 0,
        max_cache_writes: 0,
    }
}

#[test]
fn exact_selector_memory_search_is_sub_millisecond_and_side_effect_free() {
    let (index, request) = fixture();
    let loads = Arc::new(AtomicUsize::new(0));
    let resident = MemorySearchResidentV1::new(CountingBackend {
        index,
        loads: Arc::clone(&loads),
    });

    let cold_resolution = resident
        .resolve(&request)
        .expect("cold resident lookup must load the active generation");
    let warm_resolution = resident
        .resolve(&request)
        .expect("warm resident lookup must stay in memory");

    assert!(resident.is_initialized());
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(
        cold_resolution.state,
        MemorySearchResolutionStateV1::LiveHit
    );
    assert_eq!(
        warm_resolution.state,
        MemorySearchResolutionStateV1::LiveHit
    );
    let contract = hard_contract();
    contract
        .validate_receipt(&cold_resolution.performance)
        .expect("cold Memory Search must satisfy the hard performance contract");
    contract
        .validate_receipt(&warm_resolution.performance)
        .expect("warm Memory Search must satisfy the hard performance contract");
}

#[test]
fn concurrent_memory_search_loads_generation_exactly_once() {
    let (index, request) = fixture();
    let loads = Arc::new(AtomicUsize::new(0));
    let resident = Arc::new(MemorySearchResidentV1::new(CountingBackend {
        index,
        loads: Arc::clone(&loads),
    }));
    let request = Arc::new(request);
    let workers = (0..32)
        .map(|_| {
            let resident = Arc::clone(&resident);
            let request = Arc::clone(&request);
            thread::spawn(move || {
                resident
                    .resolve(&request)
                    .expect("concurrent lookup must resolve")
            })
        })
        .collect::<Vec<_>>();

    let contract = hard_contract();
    for worker in workers {
        let resolution = worker.join().expect("concurrent lookup must not panic");
        assert_eq!(resolution.state, MemorySearchResolutionStateV1::LiveHit);
        contract
            .validate_receipt(&resolution.performance)
            .expect("concurrent Memory Search must remain side-effect free");
    }
    assert_eq!(loads.load(Ordering::SeqCst), 1);
}
