use agent_semantic_search_projection::{
    RESIDENT_SEARCH_RESULT_SCHEMA_ID, RESIDENT_SEARCH_RESULT_SCHEMA_VERSION, ResidentSearchHit,
    ResidentSearchProjectionTier, ResidentSearchReadyResult, ResidentSearchReadyState,
    ResidentSearchWorkCounters,
};

use crate::{
    RUNTIME_SEARCH_SELECTOR_OWNER_LIMIT, ResidentGraphSearchStage,
    bounded_ranked_selector_owner_paths, bounded_runtime_search_source,
    build_runtime_provider_search_receipt, build_runtime_provider_search_receipt_with_graph,
};

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
        .send(Ok(std::sync::Arc::new(ready_result(
            &generation,
            "src/z.rs",
            "z",
        ))))
        .await
        .expect("resident source is admitted");
    source_b_tx
        .send(Ok(std::sync::Arc::new(ready_result(
            &generation,
            "src/a.rs",
            "a",
        ))))
        .await
        .expect("provider source is admitted");
    drop((source_a_tx, source_b_tx));

    let receipt = build_runtime_provider_search_receipt(
        "operation".to_owned(),
        agent_semantic_config::LanguageId::new("rust"),
        vec![source_a, source_b],
        7,
        Vec::new(),
    )
    .await
    .expect("bounded source fan-in succeeds");

    assert_eq!(receipt.candidate_count, 2);
    assert_eq!(
        receipt.selector_projection_budget,
        RUNTIME_SEARCH_SELECTOR_OWNER_LIMIT
    );
    assert_eq!(receipt.projected_owner_count, 0);
    assert_eq!(receipt.selectors, ["a", "z"]);
    assert_eq!(receipt.owner_paths, ["src/a.rs", "src/z.rs"]);
    assert_eq!(receipt.work_counters.scheduler_task_count, 0);
}

#[tokio::test]
async fn receipt_rejects_parser_projection_above_the_shared_owner_budget() {
    let generation = format!("blake3-256:{}", "a".repeat(64));
    let parser_owned_selector_pairs = (0..=RUNTIME_SEARCH_SELECTOR_OWNER_LIMIT)
        .map(|index| {
            (
                format!("rust://src/{index:03}.rs#item/function/owner_{index}"),
                format!("src/{index:03}.rs"),
            )
        })
        .collect();

    let error = build_runtime_provider_search_receipt(
        "operation".to_owned(),
        agent_semantic_config::LanguageId::new("rust"),
        vec![crate::RuntimeSearchSource::once(
            "resident",
            ready_result(&generation, "src/lib.rs", "lib"),
        )],
        0,
        parser_owned_selector_pairs,
    )
    .await
    .expect_err("projection above the shared owner budget must fail closed");

    assert!(error.contains("exceeded owner budget"));
}

#[test]
fn ranked_selector_projection_is_bounded_after_graph_ranking() {
    let generation = format!("blake3-256:{}", "a".repeat(64));
    let lexical_hits = (0..100)
        .map(|index| {
            ready_result(
                &generation,
                &format!("src/{index:03}.rs"),
                &format!("selector-{index:03}"),
            )
            .hits
            .into_iter()
            .next()
            .expect("one lexical hit")
        })
        .collect::<Vec<_>>();
    let graph_stage = ResidentGraphSearchStage {
        generation_digest: generation,
        result_digest: format!("blake3-256:{}", "b".repeat(64)),
        ranked_owner_paths: (0..100)
            .rev()
            .map(|index| format!("src/{index:03}.rs"))
            .collect(),
        elapsed_micros: 1,
        work: crate::ResidentGraphSearchWork::default(),
    };

    let projected = bounded_ranked_selector_owner_paths(&lexical_hits, Some(&graph_stage));

    assert_eq!(lexical_hits.len(), 100, "candidate evidence stays complete");
    assert_eq!(projected.len(), RUNTIME_SEARCH_SELECTOR_OWNER_LIMIT);
    assert_eq!(projected.first().map(String::as_str), Some("src/099.rs"));
    assert_eq!(projected.last().map(String::as_str), Some("src/084.rs"));
}

#[test]
fn lexical_selector_projection_fallback_is_bounded_and_deterministic() {
    let generation = format!("blake3-256:{}", "a".repeat(64));
    let mut lexical_hits = (0..32)
        .map(|index| {
            ready_result(
                &generation,
                &format!("src/{index:03}.rs"),
                &format!("selector-{index:03}"),
            )
            .hits
            .into_iter()
            .next()
            .expect("one lexical hit")
        })
        .collect::<Vec<_>>();
    lexical_hits.insert(1, lexical_hits[0].clone());

    let projected = bounded_ranked_selector_owner_paths(&lexical_hits, None);

    assert_eq!(projected.len(), RUNTIME_SEARCH_SELECTOR_OWNER_LIMIT);
    assert_eq!(projected.first().map(String::as_str), Some("src/000.rs"));
    assert_eq!(projected.last().map(String::as_str), Some("src/015.rs"));
}

#[tokio::test]
async fn fan_in_rejects_cross_generation_results() {
    let first = format!("blake3-256:{}", "a".repeat(64));
    let second = format!("blake3-256:{}", "b".repeat(64));
    let error = build_runtime_provider_search_receipt(
        "operation".to_owned(),
        agent_semantic_config::LanguageId::new("rust"),
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

#[tokio::test]
async fn graph_stage_reorders_only_the_same_generation_frontier() {
    let generation = format!("blake3-256:{}", "a".repeat(64));
    let receipt = build_runtime_provider_search_receipt_with_graph(
        "operation".to_owned(),
        agent_semantic_config::LanguageId::new("rust"),
        vec![
            crate::RuntimeSearchSource::once(
                "resident-a",
                ready_result(&generation, "src/a.rs", "a"),
            ),
            crate::RuntimeSearchSource::once(
                "resident-z",
                ready_result(&generation, "src/z.rs", "z"),
            ),
        ],
        7,
        Vec::new(),
        Some(ResidentGraphSearchStage {
            generation_digest: generation,
            result_digest: format!("blake3-256:{}", "b".repeat(64)),
            ranked_owner_paths: vec!["src/z.rs".to_owned(), "src/a.rs".to_owned()],
            elapsed_micros: 5,
            work: crate::ResidentGraphSearchWork::default(),
        }),
    )
    .await
    .expect("same-generation graph stage is admitted");

    assert_eq!(receipt.owner_paths, ["src/z.rs", "src/a.rs"]);
    assert!(receipt.service_elapsed_micros >= 5);
}
