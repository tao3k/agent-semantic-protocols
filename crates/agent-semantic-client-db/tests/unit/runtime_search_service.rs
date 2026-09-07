// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search::RuntimeSearchSource;
use agent_semantic_search::build_runtime_provider_search_receipt;
use agent_semantic_search_projection::RESIDENT_SEARCH_RESULT_SCHEMA_ID;
use agent_semantic_search_projection::RESIDENT_SEARCH_RESULT_SCHEMA_VERSION;
use agent_semantic_search_projection::ResidentSearchReadyResult;
use agent_semantic_search_projection::ResidentSearchReadyState;
use agent_semantic_search_projection::ResidentSearchWorkCounters;

fn resident_result(
    generation_digest: &str,
    root_digest: &str,
    provider_digest: &str,
    index_artifact_digest: &str,
) -> ResidentSearchReadyResult {
    ResidentSearchReadyResult {
        schema_id: RESIDENT_SEARCH_RESULT_SCHEMA_ID.to_owned(),
        schema_version: RESIDENT_SEARCH_RESULT_SCHEMA_VERSION.to_owned(),
        state: ResidentSearchReadyState::Ready,
        generation_digest: generation_digest.to_owned(),
        root_digest: root_digest.to_owned(),
        provider_digest: provider_digest.to_owned(),
        index_artifact_digest: index_artifact_digest.to_owned(),
        hits: Vec::new(),
        work_counters: ResidentSearchWorkCounters::default(),
    }
}

#[tokio::test]
async fn provider_search_invalid_resident_generation_is_typed() {
    let error = build_runtime_provider_search_receipt(
        "op-resident-index".to_owned(),
        agent_semantic_client_core::LanguageId::new("gerbil"),
        vec![RuntimeSearchSource::once(
            "resident",
            resident_result("", "root", "provider", "index"),
        )],
        0,
        Vec::new(),
    )
    .await
    .expect_err("missing resident source index must fail closed");
    assert_eq!(error, "resident search result identity is invalid");
}

#[tokio::test]
async fn provider_search_receipt_exposes_runtime_timing_and_zero_external_work() {
    let receipt = build_runtime_provider_search_receipt(
        "op-resident-miss".to_owned(),
        agent_semantic_client_core::LanguageId::new("rust"),
        vec![RuntimeSearchSource::once(
            "resident",
            resident_result("blake3-256:generation", "root", "provider", "index"),
        )],
        7,
        Vec::new(),
    )
    .await
    .expect("resident miss is a successful terminal search");

    assert_eq!(receipt.schema_version, "1");
    assert_eq!(receipt.resident_read_elapsed_micros, 7);
    assert!(receipt.elapsed_micros >= receipt.resident_read_elapsed_micros);
    assert!(receipt.selectors.is_empty());
    assert!(receipt.owner_paths.is_empty());
    assert_eq!(receipt.work_counters.database_read_count, 0);
    assert_eq!(receipt.work_counters.filesystem_read_count, 0);
    assert_eq!(receipt.work_counters.provider_process_count, 0);
    assert_eq!(receipt.work_counters.socket_operation_count, 0);
    assert_eq!(receipt.work_counters.scheduler_task_count, 0);
}

#[tokio::test]
async fn provider_owner_request_without_an_actor_response_returns_when_receiver_closes() {
    let (handle, receiver) = super::runtime_search_service_channel();
    drop(receiver);

    let result = handle
        .provider_owner(
            "workspace-test".to_owned(),
            std::path::PathBuf::from("/workspace"),
            "rust".to_owned(),
            "src/lib.rs".to_owned(),
        )
        .await;
    let error = match result {
        Ok(_) => panic!("missing actor response must not leave provider-owner pending"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        "Runtime search service is not accepting owner requests"
    );
}
