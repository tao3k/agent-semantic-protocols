//! Exact-segment query-key publication tests.

use super::{generation, owner, resident_pointer};
use agent_semantic_client_db::runtime_server_workspace::{
    RuntimeServerWorkspaceRegistry, WorkspaceExactProjectionDataPlaneClient,
};
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
async fn exact_projection_round_trip_preserves_parser_owned_selector_query_keys() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let mut published_owner = owner(
        "src/lib.rs",
        "rust://src/lib.rs#item/function/compare",
        b"fn compare() {}",
    );
    published_owner.selectors[0].query_keys = vec!["compare".to_owned(), "function".to_owned()];
    registry
        .publish(
            "selector-query-key-round-trip",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-selector-query-key-round-trip",
                1,
                published_owner.clone(),
            ),
        )
        .await
        .expect("publish selector query-key generation");

    let pointer = resident_pointer(temporary.path(), "workspace-selector-query-key-round-trip");
    let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
        .await
        .expect("open exact selector query-key index");
    let restored = client
        .owner_snapshot("src/lib.rs")
        .expect("read owner snapshot")
        .expect("owner exists");

    assert_eq!(restored, published_owner);
    let compare = client
        .owner_search_snapshot("src/lib.rs", &["compare".to_owned()], 100)
        .expect("read compare owner search")
        .expect("owner exists");
    assert_eq!(compare.candidate_count, 1);
    assert_eq!(compare.selectors.len(), 1);
    assert_eq!(
        compare.selectors[0].selector,
        "rust://src/lib.rs#item/function/compare"
    );

    let absent = client
        .owner_search_snapshot("src/lib.rs", &["absent".to_owned()], 100)
        .expect("read absent owner search")
        .expect("owner exists");
    assert_eq!(absent.candidate_count, 0);
    assert!(absent.selectors.is_empty());
}
