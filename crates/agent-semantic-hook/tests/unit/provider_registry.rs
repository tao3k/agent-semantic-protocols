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
        "[provider-registry-perf] scope=install-time-full-registry languages={} elapsedMicros={} budgetMicros=10000",
        manifests.len(),
        elapsed.as_micros()
    );
    assert!(
        elapsed < Duration::from_millis(10),
        "install-time registry and all provider routes must materialize within 10ms, elapsed={elapsed:?}"
    );
}

#[test]
fn selected_registry_method_lookup_is_lazy_and_sub_millisecond_warm() {
    let _ = schema_registry();
    let started = Instant::now();
    for _ in 0..100 {
        let invocation = crate::registered_provider_method_invocation_v1(
            "rust",
            "rs-harness",
            "search/owner-native",
        )
        .expect("resolve selected provider method")
        .expect("selected provider method");
        assert_eq!(invocation.argv[0], "rs-harness");
    }
    let elapsed = started.elapsed();
    println!(
        "[provider-registry-perf] scope=query-time-selected-method lookups=100 elapsedMicros={} budgetMicros=1000",
        elapsed.as_micros()
    );
    assert!(
        elapsed < Duration::from_millis(1),
        "100 warm selected-method lookups must remain sub-millisecond, elapsed={elapsed:?}"
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

#[test]
fn registry_method_inventory_is_explicit() {
    let rust_native_owner = crate::registered_provider_method_invocation_v1(
        "rust",
        "rs-harness",
        "search/owner-native",
    )
    .expect("resolve Rust native owner transport")
    .expect("Rust native owner transport must be registered");
    assert_eq!(
        rust_native_owner.argv,
        [
            "rs-harness",
            "owner-search-stdin",
            "--asp-provider-id",
            "rs-harness",
        ]
    );

    let rust_native_exact = crate::registered_provider_method_invocation_v1(
        "rust",
        "rs-harness",
        "query/exact-selector-native-v1",
    )
    .expect("resolve Rust native exact transport")
    .expect("Rust native exact transport must be registered");
    assert_eq!(
        rust_native_exact.argv,
        ["rs-harness", "query", "--asp-exact-request-stdin", "--json"]
    );

    let registry = super::schema_registry();
    for language in registry
        .languages
        .iter()
        .filter(|language| language.language_id != "rust")
    {
        assert!(
            crate::registered_provider_method_invocation_v1(
                language.language_id.as_str(),
                language.provider_id.as_str(),
                "search/owner-native",
            )
            .expect("resolve native owner transport")
            .is_none(),
            "{} must not inherit Rust's native owner transport",
            language.language_id
        );
    }

    let mut declared_native_exact_count = 0usize;
    for language in &registry.languages {
        let declared = language
            .method_descriptors
            .iter()
            .find(|descriptor| descriptor.method == "query/exact-selector-native-v1")
            .map(|descriptor| descriptor.invocation.clone());
        declared_native_exact_count += usize::from(declared.is_some());
        let resolved = crate::registered_provider_method_invocation_v1(
            language.language_id.as_str(),
            language.provider_id.as_str(),
            "query/exact-selector-native-v1",
        )
        .expect("resolve provider-declared native exact transport");
        assert_eq!(
            resolved, declared,
            "{} native exact admission must be derived from its registered method descriptor",
            language.language_id
        );
    }
    assert!(
        declared_native_exact_count > 0,
        "at least one registered language must exercise native exact admission"
    );

    assert!(
        crate::registered_provider_method_invocation_v1(
            "rust",
            "not-rs-harness",
            "search/owner-native",
        )
        .expect_err("provider identity drift must fail closed")
        .contains("provider drift")
    );
}

#[test]
fn every_registered_language_has_provider_owned_development_authority() {
    use crate::ProviderDevelopmentArtifactDomain::{Checkout, StateHomeProviderStaging};

    for (language, source_root, artifact_domain) in [
        ("rust", "languages/rust-lang-project-harness", Checkout),
        (
            "typescript",
            "languages/typescript-lang-project-harness",
            StateHomeProviderStaging,
        ),
        (
            "python",
            "languages/python-lang-project-harness",
            StateHomeProviderStaging,
        ),
        (
            "gerbil-scheme",
            "languages/gerbil-scheme-language-project-harness",
            StateHomeProviderStaging,
        ),
        ("julia", "languages/JuliaLangProjectHarness.jl", Checkout),
        ("org", "languages/orgize", Checkout),
        ("md", "languages/orgize", Checkout),
    ] {
        let registration =
            crate::registered_provider_development_v1(language).expect("development authority");
        assert_eq!(registration.development.schema_version, "1");
        assert_eq!(registration.development.source_root, source_root);
        assert_eq!(registration.development.artifact_domain, artifact_domain);
        assert_eq!(
            registration.development.build_binding,
            "root-development-installer-v1"
        );
    }
}
