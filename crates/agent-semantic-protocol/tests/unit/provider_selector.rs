#[path = "../../src/command/provider_selector.rs"]
mod implementation;

use implementation::root_structural_selector_language;

#[test]
fn registered_language_facades_are_registry_owned() {
    assert!(implementation::is_language_facade("rust"));
    assert!(!implementation::is_language_facade("effect"));
    let message = implementation::unsupported_language_facade_message("effect", None, None);
    assert!(message.contains("Known language facades:"));
    assert!(message.contains("asp providers"));
}

#[test]
fn exact_query_is_provider_owned() {
    assert!(implementation::is_provider_owned_structural_selector_query(
        "rust",
        &[
            "query".to_owned(),
            "--selector".to_owned(),
            "rust://src/lib.rs#item/function/run".to_owned(),
        ],
    ));
}

#[test]
fn python_selector_routes_to_python_facade() {
    let args = vec![
        "--selector".to_string(),
        "python://src/example.py#item/function/run".to_string(),
    ];
    assert_eq!(
        root_structural_selector_language(&args)
            .expect("valid Python selector")
            .as_deref(),
        Some("python")
    );
}

#[test]
fn gerbil_selector_routes_to_gerbil_facade() {
    let args = vec!["--selector=gerbil-scheme://src/example.ss#item/def/run".to_string()];
    assert_eq!(
        root_structural_selector_language(&args)
            .expect("valid Gerbil selector")
            .as_deref(),
        Some("gerbil-scheme")
    );
}

#[test]
fn query_without_selector_preserves_root_facade() {
    assert_eq!(
        root_structural_selector_language(&["--term".to_string(), "run".to_string()])
            .expect("query without selector"),
        None
    );
}

#[test]
fn malformed_selector_fails_closed() {
    let error = root_structural_selector_language(&[
        "--selector".to_string(),
        "not-a-structural-selector".to_string(),
    ])
    .expect_err("malformed selector must fail closed");
    assert_eq!(
        error,
        "invalid structural selector `not-a-structural-selector`"
    );
}
