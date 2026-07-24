use std::path::Path;

use agent_semantic_client_core::{
    CacheExportMethod, ClientMethod, ClientRequest, ResolvedProvider,
};

use super::{exact_request_fingerprint, request_export_method};

#[test]
fn dependency_search_alias_uses_deps_cache_method_and_fingerprint() {
    let root = Path::new(".");
    let deps_args = vec![
        "deps".to_string(),
        "serde".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ];
    let dependency_args = vec![
        "dependency".to_string(),
        "serde".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ];
    let deps_request =
        ClientRequest::new(ClientMethod::Search, root).with_forwarded_args(deps_args.clone());
    let dependency_request =
        ClientRequest::new(ClientMethod::Search, root).with_forwarded_args(dependency_args.clone());
    let export_method = CacheExportMethod::from("search/deps");
    let provider = rust_provider();

    assert_eq!(
        request_export_method(&deps_request)
            .expect("deps export method")
            .as_str(),
        "search/deps"
    );
    assert_eq!(
        request_export_method(&dependency_request)
            .expect("dependency export method")
            .as_str(),
        "search/deps"
    );
    assert_eq!(
        exact_request_fingerprint(&provider, root, &export_method, &deps_args),
        exact_request_fingerprint(&provider, root, &export_method, &dependency_args),
    );
}

fn rust_provider() -> ResolvedProvider {
    crate::test_support::resolved_provider("rust")
}
