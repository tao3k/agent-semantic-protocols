use std::time::{Duration, Instant};

use super::{materialize_provider_routes, schema_registry, schema_registry_provider_manifests};

#[test]
fn registry_is_singleton_and_all_language_routes_materialize_in_milliseconds() {
    let started = Instant::now();
    let first = schema_registry() as *const _;
    let second = schema_registry() as *const _;
    assert_eq!(
        first, second,
        "registry must be parsed exactly once per process"
    );

    let manifests = schema_registry_provider_manifests();
    for manifest in &manifests {
        materialize_provider_routes(manifest).expect("materialize provider routes");
    }
    let elapsed = started.elapsed();
    println!(
        "[provider-registry-perf] languages={} elapsedMicros={} budgetMicros=250000",
        manifests.len(),
        elapsed.as_micros()
    );
    assert!(
        elapsed < Duration::from_millis(250),
        "registry and all provider routes must materialize in milliseconds, elapsed={elapsed:?}"
    );
}
