use super::require_runtime_exact_projection_budget;

#[test]
fn runtime_exact_projection_accepts_server_evidence_below_mmap_budget() {
    require_runtime_exact_projection_budget(999)
        .expect("Runtime-owned resident-read evidence below one millisecond must be accepted");
}

#[test]
fn runtime_exact_projection_rejects_server_evidence_at_mmap_budget() {
    let error = require_runtime_exact_projection_budget(1_000)
        .expect_err("the one-millisecond mmap boundary must fail closed");
    assert!(error.contains("elapsedMicros=1000"), "{error}");
    assert!(error.contains("budgetExclusiveMicros=1000"), "{error}");
}
