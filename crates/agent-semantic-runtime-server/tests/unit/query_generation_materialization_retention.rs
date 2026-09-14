// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Generation-local completed-response retention tests.

use super::test_generation;
use crate::runtime_query_generation::{
    RuntimeQueryMaterializationState, RuntimeSearchMaterializationState,
};

#[tokio::test]
async fn completed_search_slot_is_bounded_and_evicted_waiter_keeps_terminal() {
    let generation = test_generation("blake3-256:bounded-search-generation");
    assert!(
        generation
            .begin_search_materialization("first".into())
            .unwrap()
    );
    let waiting_generation = std::sync::Arc::clone(&generation);
    let waiter = tokio::spawn(async move {
        waiting_generation
            .await_search_materialization("first")
            .await
    });
    tokio::task::yield_now().await;
    generation
        .publish_search_materialization("first".into(), Ok(serde_json::json!({"slot": 1})))
        .unwrap();
    assert!(
        generation
            .begin_search_materialization("second".into())
            .unwrap()
    );
    generation
        .publish_search_materialization("second".into(), Ok(serde_json::json!({"slot": 2})))
        .unwrap();

    assert!(
        generation
            .search_materialization("first")
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        generation.search_materialization("second").unwrap(),
        Some(RuntimeSearchMaterializationState::Ready(value)) if value["slot"] == 2
    ));
    assert!(matches!(
        waiter.await.unwrap().unwrap(),
        RuntimeSearchMaterializationState::Ready(value) if value["slot"] == 1
    ));
}

#[test]
fn completed_query_slot_retains_only_the_latest_terminal() {
    let generation = test_generation("blake3-256:bounded-query-generation");
    for (key, slot) in [("first", 1), ("second", 2)] {
        assert!(generation.begin_query_materialization(key.into()).unwrap());
        generation
            .publish_query_materialization(key.into(), Ok(serde_json::json!({"slot": slot})))
            .unwrap();
    }

    assert!(generation.query_materialization("first").unwrap().is_none());
    assert!(matches!(
        generation.query_materialization("second").unwrap(),
        Some(RuntimeQueryMaterializationState::Ready(value)) if value["slot"] == 2
    ));
}
