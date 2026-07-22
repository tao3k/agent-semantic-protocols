use super::exact_selector_cache_contract::{key, record};
use agent_semantic_content_identity::exact_selector_cache::ExactSelectorWarmSideEffectsV1;
use std::hint::black_box;
use std::time::Instant;

const SAMPLES: usize = 64;
const ITERATIONS: usize = 1_024;
const WARM_HIT_P95_BUDGET_NS: u128 = 10_000_000;

#[test]
fn warm_hit_p95_is_bounded_and_side_effect_free() {
    let record = record();
    let key = key(&record);
    for _ in 0..ITERATIONS {
        let hit = record.validate_warm_hit(&key).expect("valid warm hit");
        assert_eq!(hit.side_effects, ExactSelectorWarmSideEffectsV1::ZERO);
    }

    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        for _ in 0..ITERATIONS {
            let hit = black_box(&record)
                .validate_warm_hit(black_box(&key))
                .expect("valid warm hit");
            black_box(hit);
        }
        samples.push(started.elapsed().as_nanos() / ITERATIONS as u128);
    }
    samples.sort_unstable();
    let p95_ns = samples[(SAMPLES * 95 / 100).min(SAMPLES - 1)];
    println!(
        "[merkle-warm-test] scenario=valid-hit p95Ns={} budgetNs={} parserProcessCount=0 contentStoreWriteCount=0 tursoWriteCount=0 manifestWriteCount=0 state=ok",
        p95_ns, WARM_HIT_P95_BUDGET_NS
    );
    assert!(
        p95_ns <= WARM_HIT_P95_BUDGET_NS,
        "p95Ns={p95_ns} exceeds budgetNs={WARM_HIT_P95_BUDGET_NS}"
    );
}
