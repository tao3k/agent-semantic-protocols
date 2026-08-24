use agent_semantic_search_projection::{
    RESIDENT_SEARCH_RESULT_SCHEMA_ID, RESIDENT_SEARCH_RESULT_SCHEMA_VERSION, ResidentSearchHit,
    ResidentSearchProjectionTier, ResidentSearchReadyResult, ResidentSearchReadyState,
    ResidentSearchWorkCounters,
};

use crate::{bounded_runtime_search_source, build_runtime_provider_search_receipt};

fn ready_result(generation: &str, owner_path: &str, selector: &str) -> ResidentSearchReadyResult {
    ResidentSearchReadyResult {
        schema_id: RESIDENT_SEARCH_RESULT_SCHEMA_ID.to_owned(),
        schema_version: RESIDENT_SEARCH_RESULT_SCHEMA_VERSION.to_owned(),
        state: ResidentSearchReadyState::Ready,
        generation_digest: generation.to_owned(),
        root_digest: "1".repeat(64),
        provider_digest: "provider".to_owned(),
        index_artifact_digest: "index".to_owned(),
        hits: vec![ResidentSearchHit {
            owner_path: owner_path.to_owned(),
            owner_content_digest: "content".to_owned(),
            language_id: Some("rust".to_owned()),
            projection_tier: ResidentSearchProjectionTier::OwnerLocalDynamic,
            line_count: 1,
            query_keys: vec!["search".to_owned()],
            selector: Some(selector.to_owned()),
            score: Some(1),
        }],
        work_counters: ResidentSearchWorkCounters::default(),
    }
}

#[tokio::test]
async fn bounded_sources_fan_in_with_deterministic_identity_order() {
    let generation = format!("blake3-256:{}", "a".repeat(64));
    let (source_a_tx, source_a) = bounded_runtime_search_source("resident");
    let (source_b_tx, source_b) = bounded_runtime_search_source("provider");
    source_a_tx
        .send(Ok(ready_result(&generation, "src/z.rs", "z")))
        .await
        .expect("resident source is admitted");
    source_b_tx
        .send(Ok(ready_result(&generation, "src/a.rs", "a")))
        .await
        .expect("provider source is admitted");
    drop((source_a_tx, source_b_tx));

    let receipt = build_runtime_provider_search_receipt(
        "operation".to_owned(),
        agent_semantic_client_core::LanguageId::new("rust"),
        vec![source_a, source_b],
        7,
        Vec::new(),
    )
    .await
    .expect("bounded source fan-in succeeds");

    assert_eq!(receipt.candidate_count, 2);
    assert_eq!(receipt.selectors, ["a", "z"]);
    assert_eq!(receipt.owner_paths, ["src/a.rs", "src/z.rs"]);
    assert_eq!(receipt.work_counters.scheduler_task_count, 0);
}

#[tokio::test]
async fn fan_in_rejects_cross_generation_results() {
    let first = format!("blake3-256:{}", "a".repeat(64));
    let second = format!("blake3-256:{}", "b".repeat(64));
    let error = build_runtime_provider_search_receipt(
        "operation".to_owned(),
        agent_semantic_client_core::LanguageId::new("rust"),
        vec![
            crate::RuntimeSearchSource::once("resident", ready_result(&first, "a.rs", "a")),
            crate::RuntimeSearchSource::once("provider", ready_result(&second, "b.rs", "b")),
        ],
        0,
        Vec::new(),
    )
    .await
    .expect_err("cross-generation fan-in must fail closed");

    assert!(error.contains("crossed Runtime generation authority"));
}
