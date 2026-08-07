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
fn provider_native_argument_projections_are_closed_and_capability_truthful() {
    let lexical_values = crate::ProviderMethodArgumentValuesV1 {
        query: Some("alpha beta".to_string()),
        workspace: Some("/tmp/project".to_string()),
        presentation: Some("seeds".to_string()),
        owner: Some("must-not-leak.rs".to_string()),
    };
    for (language_id, provider_id) in [("rust", "rs-harness"), ("python", "py-harness")] {
        let argv = crate::registered_provider_method_projected_argv_v1(
            language_id,
            provider_id,
            "search/lexical",
            &lexical_values,
        )
        .unwrap_or_else(|error| panic!("project {language_id} lexical argv: {error}"));
        assert_eq!(
            argv,
            [
                "search",
                "lexical",
                "alpha beta",
                "owner",
                "tests",
                "--workspace",
                "/tmp/project",
                "--view",
                "seeds",
            ],
            "{language_id} projection must be deterministic and must not project the owner facade field"
        );
    }

    let gerbil_error = crate::registered_provider_method_projected_argv_v1(
        "gerbil-scheme",
        "gerbil-scheme-harness",
        "search/lexical",
        &lexical_values,
    )
    .expect_err("Gerbil whole-workspace lexical is not a provider-native capability");
    assert!(
        gerbil_error.contains("reasonKind=provider-native-argument-projection-unavailable")
            && gerbil_error.contains("detail=projection-not-declared"),
        "{gerbil_error}"
    );

    let owner_values = crate::ProviderMethodArgumentValuesV1 {
        owner: Some("src/main.ss".to_string()),
        workspace: Some("/tmp/project".to_string()),
        presentation: Some("seeds".to_string()),
        ..Default::default()
    };
    let gerbil_owner = crate::registered_provider_method_projected_argv_v1(
        "gerbil-scheme",
        "gerbil-scheme-harness",
        "search/owner",
        &owner_values,
    )
    .expect("Gerbil explicit-owner projection");
    assert_eq!(
        gerbil_owner,
        [
            "search",
            "owner",
            "src/main.ss",
            "items",
            "--workspace",
            "/tmp/project",
            "--view",
            "seeds",
        ]
    );
}

#[test]
fn provider_native_argument_projection_missing_slot_fails_typed() {
    let error = crate::registered_provider_method_projected_argv_v1(
        "rust",
        "rs-harness",
        "search/lexical",
        &crate::ProviderMethodArgumentValuesV1::default(),
    )
    .expect_err("missing projection values must fail closed");
    assert!(
        error.contains("reasonKind=provider-native-argument-projection-unavailable")
            && error.contains("detail=missing-slot-query"),
        "{error}"
    );
}

#[test]
fn every_registered_language_has_provider_owned_development_authority() {
    use crate::ProviderDevelopmentArtifactDomain::Checkout;

    for (language, source_root, artifact_domain, build_binding) in [
        (
            "rust",
            "languages/rust-lang-project-harness",
            Checkout,
            "provider-workspace-install-v1",
        ),
        (
            "typescript",
            "languages/typescript-lang-project-harness",
            Checkout,
            "provider-workspace-install-v1",
        ),
        (
            "python",
            "languages/python-lang-project-harness",
            Checkout,
            "provider-workspace-install-v1",
        ),
        (
            "gerbil-scheme",
            "languages/gerbil-scheme-language-project-harness",
            Checkout,
            "provider-workspace-install-v1",
        ),
        (
            "julia",
            "languages/JuliaLangProjectHarness.jl",
            Checkout,
            "provider-workspace-install-v1",
        ),
        (
            "org",
            "languages/orgize",
            Checkout,
            "root-development-installer-v1",
        ),
        (
            "md",
            "languages/orgize",
            Checkout,
            "root-development-installer-v1",
        ),
    ] {
        let registration =
            crate::registered_provider_development_v1(language).expect("development authority");
        assert_eq!(registration.development.schema_version, "1");
        assert_eq!(registration.development.source_root, source_root);
        assert_eq!(registration.development.artifact_domain, artifact_domain);
        assert_eq!(registration.development.build_binding, build_binding);
    }
}
