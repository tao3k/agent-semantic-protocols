use super::build_runtime_provider_search_receipt;
use crate::source_index::{ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState};

#[test]
fn provider_search_missing_resident_index_is_typed() {
    let error = build_runtime_provider_search_receipt(
        "op-resident-index".to_owned(),
        agent_semantic_client_core::LanguageId::new("gerbil"),
        ClientDbSourceIndexLookupResult {
            db_path: std::path::PathBuf::new(),
            state: ClientDbSourceIndexLookupState::ColdRequired,
            candidates: Vec::new(),
            source_snapshot: None,
            index_artifact_digest: None,
        },
        0,
    )
    .expect_err("missing resident source index must fail closed");
    assert_eq!(
        error,
        "runtime-provider-search-source-index-resident-index-missing"
    );
}

#[test]
fn provider_search_receipt_exposes_runtime_timing_and_zero_external_work() {
    let receipt = build_runtime_provider_search_receipt(
        "op-resident-miss".to_owned(),
        agent_semantic_client_core::LanguageId::new("rust"),
        ClientDbSourceIndexLookupResult {
            db_path: std::path::PathBuf::new(),
            state: ClientDbSourceIndexLookupState::Miss,
            candidates: Vec::new(),
            source_snapshot: None,
            index_artifact_digest: None,
        },
        7,
    )
    .expect("resident miss is a successful terminal search");

    assert_eq!(receipt.schema_version, "1");
    assert_eq!(receipt.resident_read_elapsed_micros, 7);
    assert!(receipt.elapsed_micros >= receipt.resident_read_elapsed_micros);
    assert!(receipt.selectors.is_empty());
    assert!(receipt.owner_paths.is_empty());
    assert_eq!(receipt.work_counters.database_opens, 0);
    assert_eq!(receipt.work_counters.filesystem_reads, 0);
    assert_eq!(receipt.work_counters.provider_spawns, 0);
    assert_eq!(receipt.work_counters.control_socket_roundtrips, 0);
}

#[tokio::test]
async fn provider_owner_request_without_an_actor_response_reaches_a_typed_deadline() {
    let (handle, _receiver) =
        super::runtime_search_service_channel_with_deadline(std::time::Duration::from_millis(10));

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
        "runtime search service request deadline exceeded: operation=provider-owner deadlineMillis=10"
    );
}
