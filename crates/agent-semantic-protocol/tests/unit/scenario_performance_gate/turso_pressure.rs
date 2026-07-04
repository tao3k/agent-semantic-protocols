use std::path::Path;

use super::{SharedBenchmarkToml, duration_millis_from_manifest, read_toml};

pub(super) fn asp_turso_db_engine_concurrent_process_pressure_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_turso_db_engine_concurrent_process_pressure");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_eq!(benchmark.harness, "libtest");
    assert_eq!(
        benchmark.test.as_deref(),
        Some("db_engine_cache_status_survives_concurrent_process_read_write_pressure")
    );
    assert_eq!(benchmark.route_source.as_deref(), Some("db-engine"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(16384));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
    assert!(
        duration_millis_from_manifest(&benchmark.max_total) <= 750,
        "Turso DB Engine pressure gate must remain a subsecond DB-operation budget: max_total={}",
        benchmark.max_total
    );
    assert!(
        duration_millis_from_manifest(&benchmark.observed_total) <= 750,
        "Turso DB Engine pressure gate observed total must be DB-operation latency, not process startup: observed_total={}",
        benchmark.observed_total
    );
    assert!(
        benchmark
            .observed_timings
            .contains_key("process_db_operation"),
        "Turso DB Engine pressure gate must report child-internal DB operation timing"
    );
}

pub(super) fn asp_turso_agent_session_registry_shared_route_pressure_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_turso_agent_session_registry_shared_route_pressure");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_eq!(benchmark.harness, "libtest");
    assert_eq!(
        benchmark.test.as_deref(),
        Some(
            "agent_session_registry_concurrent_process_register_shared_route_does_not_unique_fail"
        )
    );
    assert_eq!(benchmark.route_source.as_deref(), Some("db-engine"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(16384));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
    assert!(
        duration_millis_from_manifest(&benchmark.max_total) <= 500,
        "Turso session registry pressure gate must remain a subsecond DB-operation budget: max_total={}",
        benchmark.max_total
    );
    assert!(
        duration_millis_from_manifest(&benchmark.observed_total) <= 500,
        "Turso session registry pressure observed total must be DB-operation latency, not process startup: observed_total={}",
        benchmark.observed_total
    );
    assert!(
        benchmark
            .observed_timings
            .contains_key("registry_db_operation"),
        "Turso session registry pressure gate must report child-internal registry operation timing"
    );
}

pub(super) fn asp_turso_source_index_refresh_lookup_pressure_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_turso_source_index_refresh_lookup_pressure");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_eq!(benchmark.harness, "libtest");
    assert_eq!(
        benchmark.test.as_deref(),
        Some("db_engine_source_index_refresh_lookup_pressure_returns_busy_instead_of_lock_errors")
    );
    assert_eq!(benchmark.route_source.as_deref(), Some("db-engine"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(16384));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
    assert!(
        duration_millis_from_manifest(&benchmark.max_total) <= 750,
        "Turso source-index pressure gate must remain a subsecond DB-operation budget: max_total={}",
        benchmark.max_total
    );
    assert!(
        duration_millis_from_manifest(&benchmark.observed_total) <= 750,
        "Turso source-index pressure observed total must be DB-operation latency, not command startup: observed_total={}",
        benchmark.observed_total
    );
    assert!(
        benchmark
            .observed_timings
            .contains_key("source_index_pressure"),
        "Turso source-index pressure gate must report source-index operation timing"
    );
}
