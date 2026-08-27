use crate::{ServerClientRoute, resolve_server_client_method, server_client_methods};

#[test]
fn server_catalog_exposes_northbound_search_query_and_schema_bundle_methods() {
    let methods = server_client_methods(["rust".to_owned()]).expect("Rust method catalog");
    let names = methods
        .iter()
        .map(|method| method.method.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "asp.schema.bundle",
            "rust.query",
            "rust.search",
            "rust.search.owner"
        ]
    );
    assert!(!names.contains(&"rust.projection-batch"));
    assert!(methods.iter().all(|method| {
        method
            .request_schema_id
            .starts_with("agent.semantic-protocols.asp-client-")
    }));
}

#[test]
fn schema_bundle_method_is_transport_neutral_and_profile_selected() {
    let methods = server_client_methods(["rust".to_owned()]).expect("Rust method catalog");
    let method = methods
        .iter()
        .find(|method| method.method == crate::SCHEMA_BUNDLE_METHOD)
        .expect("schema bundle method");
    assert_eq!(method.route_id, "asp.schema.bundle");
    assert_eq!(
        method.request_schema_id,
        crate::SCHEMA_BUNDLE_REQUEST_SCHEMA_ID
    );
    assert_eq!(
        method.response_schema_id,
        crate::SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID
    );
    assert_eq!(
        method
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        [
            "schemaId",
            "schemaVersion",
            "languageId",
            "rootSetIds",
            "knownBundleDigest"
        ]
    );
    assert!(!method.streaming);
    assert!(!method.cancellable);
}

#[test]
fn method_resolution_is_independent_of_provider_runtime_routes() {
    let languages = ["rust".to_owned(), "python".to_owned()];
    assert_eq!(
        resolve_server_client_method("rust.search.owner", languages.clone()),
        Ok(("rust".to_owned(), ServerClientRoute::OwnerSearch))
    );
    assert_eq!(
        resolve_server_client_method("python.query", languages),
        Ok(("python".to_owned(), ServerClientRoute::ExactQuery))
    );
}

#[test]
fn duplicate_language_identity_fails_closed() {
    let error = server_client_methods(["rust".to_owned(), "rust".to_owned()])
        .expect_err("duplicate language route ownership must be rejected");
    assert!(error.contains("multiple-installed-client-languages"));
}
