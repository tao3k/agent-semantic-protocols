#[test]
fn native_exact_transport_materializes_dynamic_request_identity_once() {
    let args = super::native_exact_provider_args(
        &["query".to_owned(), "--asp-exact-request-stdin".to_owned()],
        "rust://src/lib.rs#item/function/run",
        "rs-harness",
    )
    .expect("materialize native exact provider args");

    assert_eq!(
        args,
        [
            "query",
            "--asp-exact-request-stdin",
            "--selector",
            "rust://src/lib.rs#item/function/run",
            "--projection",
            "callable-skeleton",
            "--asp-provider-id",
            "rs-harness",
        ]
    );
}

#[test]
fn native_exact_transport_rejects_registry_owned_dynamic_identity() {
    let error = super::native_exact_provider_args(
        &["query".to_owned(), "--projection".to_owned()],
        "rust://src/lib.rs#item/function/run",
        "rs-harness",
    )
    .expect_err("registry must not duplicate request identity");

    assert!(error.contains("must not encode request identity"));
}
