use super::provider_native_argument_values;
use std::path::Path;

#[test]
fn lexical_facade_fields_are_parsed_without_owner_leakage() {
    let args = [
        "search", "lexical", "--query", "alpha", "--query", "beta", "owner", "tests", "--view",
        "seeds",
    ]
    .map(str::to_string);
    let values = provider_native_argument_values(
        "search/lexical",
        &args,
        Path::new("/tmp/resolved-project"),
    )
    .expect("lexical facade argument values");
    assert_eq!(values.query.as_deref(), Some("alpha beta"));
    assert_eq!(values.workspace.as_deref(), Some("/tmp/resolved-project"));
    assert_eq!(values.presentation.as_deref(), Some("seeds"));
    assert_eq!(values.owner, None);
}

#[test]
fn unsupported_facade_option_fails_with_typed_projection_error() {
    let args = ["search", "lexical", "needle", "--json"].map(str::to_string);
    let error = provider_native_argument_values("search/lexical", &args, Path::new("/tmp/project"))
        .expect_err("unsupported facade option must fail closed");
    assert!(
        error.contains("reasonKind=provider-native-argument-projection-unavailable"),
        "{error}"
    );
}

#[test]
fn lexical_fallback_uses_registered_projection_and_fails_closed_for_gerbil() {
    let args = ["search", "lexical", "--query", "alpha", "--view", "seeds"].map(str::to_string);
    let values =
        provider_native_argument_values("search/lexical", &args, Path::new("/tmp/project"))
            .expect("lexical facade argument values");
    let rust = agent_semantic_hook::registered_provider_method_projected_argv_v1(
        "rust",
        "rs-harness",
        "search/lexical",
        &values,
    )
    .expect("Rust registered lexical projection");
    assert_eq!(
        rust,
        [
            "search",
            "lexical",
            "alpha",
            "owner",
            "tests",
            "--workspace",
            "/tmp/project",
            "--view",
            "seeds",
        ]
    );

    let gerbil = agent_semantic_hook::registered_provider_method_projected_argv_v1(
        "gerbil-scheme",
        "gerbil-scheme-harness",
        "search/lexical",
        &values,
    )
    .expect_err("Gerbil must fail before provider spawn");
    assert!(
        gerbil.contains("reasonKind=provider-native-argument-projection-unavailable")
            && gerbil.contains("detail=projection-not-declared"),
        "{gerbil}"
    );
}
