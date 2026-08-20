use super::{require_resident_sample_budget, resident_latency_distribution};

#[test]
fn resident_latency_distribution_reports_all_v1_quantiles_for_128_samples() {
    let distribution = resident_latency_distribution((0..128).collect())
        .expect("128 resident samples form a distribution");

    assert_eq!(distribution.sample_count, 128);
    assert_eq!(distribution.min_micros, 0);
    assert_eq!(distribution.p50_micros, 63);
    assert_eq!(distribution.p95_micros, 120);
    assert_eq!(distribution.p99_micros, 125);
    assert_eq!(distribution.max_micros, 127);
}

#[test]
fn any_resident_sample_above_one_millisecond_fails_the_case() {
    let error = require_resident_sample_budget("rust.case", "search-total", 73, 1_001, 1_000)
        .expect_err("one over-budget sample must fail the qualification");

    assert!(error.contains("case=rust.case"));
    assert!(error.contains("sampleIndex=73"));
    assert!(error.contains("elapsedMicros=1001"));
}
