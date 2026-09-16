// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Generation-local authority retention tests.

use super::test_generation;
use crate::runtime_query_generation::{
    RuntimeQueryMaterializationState, RuntimeSearchMaterializationState, RuntimeSearchTerminalState,
};

#[test]
fn same_content_generation_upgrade_shares_in_flight_completed_and_topology_authority() {
    let first = test_generation("blake3-256:shared-materialization-generation");
    assert!(first.begin_search_materialization("plan".into()).unwrap());
    assert!(
        first
            .begin_query_materialization("source\0selector".into())
            .unwrap()
    );
    first.project_topology_attachment.lock().unwrap().replace(
        crate::runtime_query_generation_model::RuntimeProjectTopologyCacheEntry::new(
            "blake3-256:topology-source".to_owned(),
            std::collections::BTreeSet::from(["src/lib.rs".to_owned()]),
            Err(std::sync::Arc::from("retained-fixture")),
            None,
        ),
    );

    let mut upgraded = std::sync::Arc::try_unwrap(test_generation(
        "blake3-256:shared-materialization-generation",
    ))
    .unwrap_or_else(|_| panic!("fresh test generation must have one owner"));
    upgraded
        .inherit_generation_local_authorities(&first)
        .unwrap();
    first
        .publish_search_materialization("plan".into(), Ok(serde_json::json!({"ready": true})))
        .unwrap();
    first
        .publish_query_materialization(
            "source\0selector".into(),
            Ok(serde_json::json!({"ready": true})),
        )
        .unwrap();

    assert!(matches!(
        upgraded.search_materialization("plan").unwrap(),
        Some(RuntimeSearchMaterializationState::Ready(value)) if value["ready"] == true
    ));
    assert!(matches!(
        upgraded.query_materialization("source\0selector").unwrap(),
        Some(RuntimeQueryMaterializationState::Ready(value)) if value["ready"] == true
    ));
    let retained_topology = upgraded.project_topology_attachment.lock().unwrap();
    let retained_topology = retained_topology
        .as_ref()
        .expect("same generation must retain its request topology attachment");
    assert!(retained_topology.matches(
        "blake3-256:topology-source",
        &std::collections::BTreeSet::from(["src/lib.rs".to_owned()]),
    ));
    assert!(matches!(
        retained_topology.attachment.as_ref(),
        Err(error) if error.as_ref() == "retained-fixture"
    ));
}

#[tokio::test]
async fn completed_search_materializations_are_isolated_by_normalized_plan_identity() {
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

    assert!(matches!(
        generation.search_materialization("first").unwrap(),
        Some(RuntimeSearchMaterializationState::Ready(value)) if value["slot"] == 1
    ));
    assert!(matches!(
        generation.search_materialization("second").unwrap(),
        Some(RuntimeSearchMaterializationState::Ready(value)) if value["slot"] == 2
    ));
    assert!(matches!(
        waiter.await.unwrap().unwrap(),
        RuntimeSearchTerminalState::Ready(value) if value["slot"] == 1
    ));

    for slot in 3..=5 {
        let key = format!("plan-{slot}");
        assert!(
            generation
                .begin_search_materialization(key.clone())
                .unwrap()
        );
        generation
            .publish_search_materialization(key, Ok(serde_json::json!({"slot": slot})))
            .unwrap();
    }
    let retained = ["first", "second", "plan-3", "plan-4", "plan-5"]
        .into_iter()
        .filter(|key| generation.search_materialization(key).unwrap().is_some())
        .count();
    assert_eq!(
        retained,
        generation.resource_supervisor.effective_cpu().max(2),
        "effective CPU bounds completed Search plans"
    );
}

#[test]
fn completed_query_cache_retains_one_terminal_per_v1_projection() {
    let generation = test_generation("blake3-256:bounded-query-generation");
    for (key, slot) in [("source\0first", 1), ("callable-skeleton\0first", 2)] {
        assert!(generation.begin_query_materialization(key.into()).unwrap());
        generation
            .publish_query_materialization(key.into(), Ok(serde_json::json!({"slot": slot})))
            .unwrap();
    }

    assert!(matches!(
        generation.query_materialization("source\0first").unwrap(),
        Some(RuntimeQueryMaterializationState::Ready(value)) if value["slot"] == 1
    ));
    assert!(matches!(
        generation.query_materialization("callable-skeleton\0first").unwrap(),
        Some(RuntimeQueryMaterializationState::Ready(value)) if value["slot"] == 2
    ));

    assert!(
        generation
            .begin_query_materialization("source\0second".into())
            .unwrap()
    );
    generation
        .publish_query_materialization("source\0second".into(), Ok(serde_json::json!({"slot": 3})))
        .unwrap();
    assert!(
        generation
            .query_materialization("source\0first")
            .unwrap()
            .is_none()
    );
    assert!(
        generation
            .query_materialization("callable-skeleton\0first")
            .unwrap()
            .is_some()
    );
}
