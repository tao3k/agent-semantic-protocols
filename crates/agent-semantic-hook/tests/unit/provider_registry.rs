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

#[test]
fn dependency_topology_routes_are_registered_for_capable_languages() {
    let registry = schema_registry();
    for (language_id, provider_id) in [
        ("rust", "rs-harness"),
        ("typescript", "ts-harness"),
        ("python", "py-harness"),
        ("julia", "julia-lang-project-harness"),
        ("gerbil-scheme", "gerbil-scheme-harness"),
    ] {
        let language = registry
            .languages
            .iter()
            .find(|language| {
                language.language_id == language_id && language.provider_id == provider_id
            })
            .expect("capable language registration");
        let descriptor = language
            .method_descriptors
            .iter()
            .find(|descriptor| descriptor.method == "search/dependency-topology")
            .unwrap_or_else(|| {
                panic!(
                    "missing search/dependency-topology descriptor for {language_id}/{provider_id}"
                )
            });
        assert_eq!(
            descriptor.invocation.argv[0], language.binary,
            "dependency topology invocation must use the registered binary"
        );
    }
}
