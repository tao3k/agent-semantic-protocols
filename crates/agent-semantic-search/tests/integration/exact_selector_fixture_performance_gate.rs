use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_semantic_content_identity::exact_selector_generation_fixture::{
    ExactSelectorGenerationIdentityV1, ExactSelectorGenerationRecordV1,
    ExactSelectorProjectionModeV1, build_exact_selector_generation_fixture_v1, fixture_digest_v1,
};
use agent_semantic_search::{
    ExactSelectorFixtureArtifactV1, ExactSelectorFixtureBackendV1,
    ExactSelectorFixtureFileBackendV1, ExactSelectorFixtureResidentV1,
    exact_selector_fixture_projection_v1,
};

const COLD_BUDGET: Duration = Duration::from_micros(1_000);
const WARM_BUDGET: Duration = Duration::from_micros(100);

#[test]
fn active_artifact_receipt_has_exact_selector_fixture_kind() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/active-asp-artifact-receipt.v1.schema.json"
    ))
    .expect("active artifact receipt schema must be valid JSON");
    let kinds = schema["$defs"]["leaf"]["properties"]["artifactKind"]["enum"]
        .as_array()
        .expect("artifactKind must be a closed enum");
    assert!(
        kinds
            .iter()
            .any(|kind| kind == "exact-selector-generation-fixture"),
        "active receipt must carry the exact-selector fixture path and digest"
    );
}

fn identity(selector_count: u32) -> ExactSelectorGenerationIdentityV1 {
    ExactSelectorGenerationIdentityV1 {
        language_id: "rust".to_owned(),
        provider_id: "rs-harness".to_owned(),
        workspace_identity_digest: [1; 32],
        workspace_root_digest: [2; 32],
        parser_identity_digest: [3; 32],
        query_pack_digest: [4; 32],
        generation_digest: [5; 32],
        selector_count,
        owner_count: 1,
        leaf_count: 1,
    }
}

fn selector(index: usize) -> String {
    format!("rust://src/lib.rs#item/function/fixture_{index}")
}

fn record(index: usize) -> ExactSelectorGenerationRecordV1 {
    let projection = format!("fn fixture_{index}() {{}}").into_bytes();
    ExactSelectorGenerationRecordV1 {
        structural_selector: selector(index),
        owner_path: "src/lib.rs".to_owned(),
        owner_subtree_digest: [6; 32],
        source_blob_digest: [7; 32],
        normalized_parser_facts_digest: [8; 32],
        projection_mode: ExactSelectorProjectionModeV1::Source,
        source_byte_range: 0..projection.len() as u64,
        projection,
    }
}

#[test]
fn resident_projection_carries_live_owner_validation_proof() {
    let loads = Arc::new(AtomicUsize::new(0));
    let resident = ExactSelectorFixtureResidentV1::new(CountingBackend {
        artifact: Mutex::new(Some(fixture(1))),
        loads: Arc::clone(&loads),
    });
    let projection = resident
        .resolve(&selector(0))
        .expect("resident fixture")
        .expect("fixture hit");

    assert_eq!(projection.owner_path(), "src/lib.rs");
    assert_eq!(projection.source_blob_digest(), &[7; 32]);
    assert_eq!(projection.owner_subtree_digest(), &[6; 32]);
    assert_eq!(projection.normalized_parser_facts_digest(), &[8; 32]);
    assert_eq!(
        projection.projection_mode(),
        ExactSelectorProjectionModeV1::Source
    );
    assert_eq!(loads.load(Ordering::SeqCst), 1);
}

fn fixture(selector_count: usize) -> ExactSelectorFixtureArtifactV1 {
    let records = (0..selector_count).map(record).collect();
    let fixture_bytes =
        build_exact_selector_generation_fixture_v1(&identity(selector_count as u32), records)
            .expect("complete exact-selector fixture");
    let fixture_digest = *fixture_digest_v1(&fixture_bytes).expect("canonical fixture digest");
    ExactSelectorFixtureArtifactV1 {
        fixture_bytes: fixture_bytes.into(),
        generation_digest: [5; 32],
        fixture_digest,
    }
}

#[test]
fn ten_thousand_selector_memory_lookup_has_hard_cold_and_warm_gates() {
    let _performance_gate = super::performance_gate::lock();
    const SELECTOR_COUNT: usize = 10_000;
    let fixture = fixture(SELECTOR_COUNT);
    let target = selector(SELECTOR_COUNT - 1);

    let started = Instant::now();
    let cold_projection = exact_selector_fixture_projection_v1(
        &fixture.fixture_bytes,
        &fixture.generation_digest,
        &fixture.fixture_digest,
        &target,
    )
    .expect("validated fixture")
    .expect("cold fixture hit");
    let cold_elapsed = started.elapsed();
    assert!(!cold_projection.is_empty());
    assert!(
        cold_elapsed <= COLD_BUDGET,
        "10k selector cold lookup exceeded hard budget: elapsed={cold_elapsed:?} budget={COLD_BUDGET:?}"
    );

    let started = Instant::now();
    exact_selector_fixture_projection_v1(
        &fixture.fixture_bytes,
        &fixture.generation_digest,
        &fixture.fixture_digest,
        &target,
    )
    .expect("validated fixture")
    .expect("warm fixture hit");
    let warm_elapsed = started.elapsed();
    assert!(
        warm_elapsed <= WARM_BUDGET,
        "10k selector warm lookup exceeded hard budget: elapsed={warm_elapsed:?} budget={WARM_BUDGET:?}"
    );
    eprintln!(
        "fixturePerformance selectorCount={SELECTOR_COUNT} coldMicros={} warmMicros={}",
        cold_elapsed.as_micros(),
        warm_elapsed.as_micros()
    );
}

#[derive(Debug)]
struct CountingBackend {
    artifact: Mutex<Option<ExactSelectorFixtureArtifactV1>>,
    loads: Arc<AtomicUsize>,
}

impl ExactSelectorFixtureBackendV1 for CountingBackend {
    fn load_fixture(&self) -> Result<ExactSelectorFixtureArtifactV1, String> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        self.artifact
            .lock()
            .map_err(|_| "fixture backend lock poisoned".to_owned())?
            .take()
            .ok_or_else(|| "fixture generation was loaded more than once".to_owned())
    }
}

#[test]
fn thirty_two_concurrent_callers_load_one_fixture_generation() {
    let _performance_gate = super::performance_gate::lock();
    let loads = Arc::new(AtomicUsize::new(0));
    let resident = Arc::new(ExactSelectorFixtureResidentV1::new(CountingBackend {
        artifact: Mutex::new(Some(fixture(10_000))),
        loads: Arc::clone(&loads),
    }));
    let target = selector(9_999);
    let callers = (0..32)
        .map(|_| {
            let resident = Arc::clone(&resident);
            let target = target.clone();
            std::thread::spawn(move || {
                let started = Instant::now();
                resident
                    .resolve(&target)
                    .expect("resident fixture")
                    .expect("concurrent fixture hit");
                let elapsed = started.elapsed();
                assert!(
                    elapsed <= COLD_BUDGET,
                    "concurrent cold lookup exceeded hard budget: elapsed={elapsed:?} budget={COLD_BUDGET:?}"
                );
            })
        })
        .collect::<Vec<_>>();

    for caller in callers {
        caller.join().expect("fixture caller must finish");
    }
    assert_eq!(loads.load(Ordering::SeqCst), 1);
}

#[test]
fn digest_verified_file_backend_has_hard_cold_and_warm_gates() {
    let _performance_gate = super::performance_gate::lock();
    let fixture = fixture(1);
    let artifact_digest = blake3::hash(&fixture.fixture_bytes).to_hex().to_string();
    let path = std::env::temp_dir().join(format!(
        "asp-exact-selector-fixture-{}-{artifact_digest}.bin",
        std::process::id()
    ));
    std::fs::write(&path, &fixture.fixture_bytes).expect("publish fixture artifact");
    let resident = ExactSelectorFixtureResidentV1::new(ExactSelectorFixtureFileBackendV1::new(
        &path,
        artifact_digest,
        fixture.generation_digest,
    ));

    let started = Instant::now();
    resident
        .resolve(&selector(0))
        .expect("verified artifact")
        .expect("cold file fixture hit");
    let cold_elapsed = started.elapsed();
    assert!(
        cold_elapsed <= COLD_BUDGET,
        "file fixture cold path exceeded hard budget: elapsed={cold_elapsed:?} budget={COLD_BUDGET:?}"
    );

    let started = Instant::now();
    resident
        .resolve(&selector(0))
        .expect("resident artifact")
        .expect("warm file fixture hit");
    let warm_elapsed = started.elapsed();
    assert!(
        warm_elapsed <= WARM_BUDGET,
        "file fixture warm path exceeded hard budget: elapsed={warm_elapsed:?} budget={WARM_BUDGET:?}"
    );
    eprintln!(
        "fileFixturePerformance coldMicros={} warmMicros={}",
        cold_elapsed.as_micros(),
        warm_elapsed.as_micros()
    );
    std::fs::remove_file(path).expect("remove fixture artifact");
}
