// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{ProviderProjectionTiming, RuntimeOwnerMaterializer, requires_external_parser};

fn provider(
    axis: agent_semantic_search::WorkspaceSearchProducerAxis,
) -> agent_semantic_search::WorkspaceSearchProvider {
    agent_semantic_search::WorkspaceSearchProvider {
        language_id: "org".to_owned(),
        provider_id: "asp-org".to_owned(),
        source_extensions: vec!["org".to_owned()],
        search_supported: true,
        producer_axes: vec![axis],
        enhanced_query_capability: None,
    }
}

#[test]
fn embedded_document_candidates_never_require_an_external_parser_process() {
    assert!(!requires_external_parser(&provider(
        agent_semantic_search::WorkspaceSearchProducerAxis::Document,
    )));
    assert!(requires_external_parser(&provider(
        agent_semantic_search::WorkspaceSearchProducerAxis::Language,
    )));
}

#[test]
fn provider_lifecycle_time_is_not_runtime_engine_work() {
    let timing = ProviderProjectionTiming {
        cache_probe_micros: 7,
        provider_start_micros: 11_000,
        provider_ready_micros: 23_000,
        projection_micros: 13,
        request_wall_micros: 34_020,
    };

    assert_eq!(timing.runtime_engine_work_micros(), 20);
    assert_eq!(timing.provider_lifecycle_work_micros(), 34_000);
    assert_eq!(timing.request_wall_micros, 34_020);
}

#[test]
fn last_waiter_reclaims_the_generation_scoped_claim() {
    let materializer = RuntimeOwnerMaterializer::default();
    let first = materializer
        .claim("workspace\0generation\0owner")
        .ok()
        .expect("first owner materialization claim");
    let second = materializer
        .claim("workspace\0generation\0owner")
        .ok()
        .expect("coalesced owner materialization claim");
    assert_eq!(materializer.claims.lock().unwrap().len(), 1);

    drop(first);
    assert_eq!(materializer.claims.lock().unwrap().len(), 1);
    drop(second);
    assert!(materializer.claims.lock().unwrap().is_empty());
}
