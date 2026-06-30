#![cfg(feature = "turso-overlay")]

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::{TursoClientDbSearchDocument, upsert_turso_search_document};
use agent_semantic_search::{
    TursoOverlaySearchDocument, bootstrap_turso_overlay_search_store,
    search_turso_overlay_documents, upsert_turso_overlay_search_document,
};

#[tokio::test(flavor = "current_thread")]
async fn turso_overlay_search_cold_functional_path_filters_to_overlay_hits() {
    let root = std::env::temp_dir().join(format!(
        "asp-turso-overlay-search-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    let db_path = root.join("client.sqlite3");
    bootstrap_turso_overlay_search_store(&db_path)
        .await
        .expect("bootstrap turso search schema");
    upsert_turso_search_document(
        &db_path,
        &TursoClientDbSearchDocument {
            namespace: "stable".to_string(),
            document_id: "stable-owner".to_string(),
            entity_id: "stable-owner".to_string(),
            selector: Some("rust://src/lib.rs#item/function/stable_owner".to_string()),
            document: "stable overlay_fixture_token".to_string(),
        },
    )
    .await
    .expect("upsert stable search document");
    upsert_turso_overlay_search_document(
        &db_path,
        &TursoOverlaySearchDocument {
            repo_id: "repo-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            session_id: "session-1".to_string(),
            base_generation: "dirty-1".to_string(),
            document_id: "overlay-owner".to_string(),
            selector: Some("rust://src/lib.rs#item/function/overlay_owner".to_string()),
            document: "dynamic overlay_fixture_token owner".to_string(),
        },
    )
    .await
    .expect("upsert overlay search document");

    let started = Instant::now();
    let hits = search_turso_overlay_documents(&db_path, "overlay_fixture_token", 8)
        .await
        .expect("search turso overlay documents");
    let elapsed = started.elapsed();

    assert_eq!(hits.len(), 1, "{hits:#?}");
    assert_eq!(hits[0].document_id, "overlay-owner");
    assert_eq!(
        hits[0].selector.as_deref(),
        Some("rust://src/lib.rs#item/function/overlay_owner")
    );
    assert!(
        elapsed.as_millis() <= 25,
        "overlay search should stay in the cold functional millisecond gate, elapsed={elapsed:?}"
    );
}
