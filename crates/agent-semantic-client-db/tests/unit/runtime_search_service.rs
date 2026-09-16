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
async fn provider_owner_batch_preserves_one_request_and_one_response_boundary() {
    let (handle, mut requests) = super::runtime_search_service_channel();
    let actor = tokio::spawn(async move {
        let request = tokio_stream::StreamExt::next(&mut requests)
            .await
            .expect("owner batch request");
        let super::RuntimeSearchServiceRequest::ProviderOwners {
            workspace_identity,
            language_id,
            owners,
            auxiliary_owners,
            response,
            ..
        } = request
        else {
            panic!("expected one provider owner batch request");
        };
        assert_eq!(workspace_identity, "workspace-test");
        assert_eq!(language_id, "rust");
        assert_eq!(
            owners
                .iter()
                .map(|owner| owner.owner_path.as_str())
                .collect::<Vec<_>>(),
            ["src/a.rs", "src/b.rs"]
        );
        assert!(auxiliary_owners.is_empty());
        response.send(Ok(Vec::new())).expect("batch response");
    });

    let projected = handle
        .provider_owners(
            "workspace-test".to_owned(),
            std::path::PathBuf::from("/workspace"),
            std::path::PathBuf::from("/artifacts"),
            "rust".to_owned(),
            ["src/a.rs", "src/b.rs"]
                .into_iter()
                .map(
                    |owner_path| crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
                        owner_path: owner_path.to_owned(),
                        authority: None,
                        content_digest: format!("blake3-256:{}", blake3::hash(b"source").to_hex()),
                        native_syntax_diagnostic: None,
                        bytes: b"source".to_vec(),
                        selectors: Vec::new(),
                    },
                )
                .collect(),
            Vec::new(),
        )
        .await
        .expect("owner batch response");
    assert!(projected.is_empty());
    actor.await.expect("owner batch actor");
}
