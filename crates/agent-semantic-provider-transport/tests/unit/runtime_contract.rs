use super::ProviderRuntimeContractOperation;
use super::ProviderRuntimeContractReceipt;
use super::ProviderRuntimeContractTransport;

fn operation() -> ProviderRuntimeContractOperation {
    ProviderRuntimeContractOperation {
        operation: "lexical-search".to_owned(),
        request_schema: agent_semantic_provider_protocol::ProviderSchemaReference {
            schema_id: "agent.semantic-protocols.semantic-query-packet".to_owned(),
            schema_version: "1".to_owned(),
        },
        response_schema: agent_semantic_provider_protocol::ProviderSchemaReference {
            schema_id: "agent.semantic-protocols.semantic-search-packet".to_owned(),
            schema_version: "1".to_owned(),
        },
    }
}

#[test]
fn contract_receipt_is_deterministic_and_valid() {
    let receipt = ProviderRuntimeContractReceipt::new(
        "rust-provider",
        "rust",
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111",
        "blake3-256:2222222222222222222222222222222222222222222222222222222222222222",
        ProviderRuntimeContractTransport::RuntimeIpc,
        vec![operation()],
    )
    .expect("construct contract");
    assert_eq!(
        receipt.contract_digest,
        receipt.expected_contract_digest().unwrap()
    );
    receipt.validate().expect("validate contract");
}

#[test]
fn runtime_ipc_transport_uses_the_canonical_wire_literal() {
    let encoded = serde_json::to_value(ProviderRuntimeContractTransport::RuntimeIpc)
        .expect("encode runtime transport");
    assert_eq!(encoded, serde_json::json!("runtime-ipc"));

    let decoded: ProviderRuntimeContractTransport =
        serde_json::from_value(serde_json::json!("runtime-ipc"))
            .expect("decode stable v1 runtime transport");
    assert_eq!(decoded, ProviderRuntimeContractTransport::RuntimeIpc);
}

#[test]
fn contract_receipt_rejects_duplicate_operations() {
    let mut receipt = ProviderRuntimeContractReceipt::new(
        "rust-provider",
        "rust",
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111",
        "blake3-256:2222222222222222222222222222222222222222222222222222222222222222",
        ProviderRuntimeContractTransport::RuntimeIpc,
        vec![operation()],
    )
    .expect("construct contract");
    receipt.operations.push(operation());
    assert!(receipt.validate().is_err());
}

#[test]
fn http_server_contract_is_a_first_class_runtime_transport() {
    let receipt = ProviderRuntimeContractReceipt::new(
        "asp-julia",
        "julia",
        "blake3-256:3333333333333333333333333333333333333333333333333333333333333333",
        "blake3-256:4444444444444444444444444444444444444444444444444444444444444444",
        ProviderRuntimeContractTransport::HttpJson,
        vec![operation()],
    )
    .expect("construct HTTP server contract");
    receipt.validate().expect("validate HTTP server contract");
    assert_eq!(
        receipt.transport,
        ProviderRuntimeContractTransport::HttpJson
    );
}
