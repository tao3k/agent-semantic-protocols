use super::{
    ProviderRuntimeContractOperation, ProviderRuntimeContractReceipt,
    ProviderRuntimeContractTransport,
};

fn operation() -> ProviderRuntimeContractOperation {
    ProviderRuntimeContractOperation {
        operation: "lexical-search".to_owned(),
        request_schema_id: "agent.semantic-protocols.semantic-query-packet".to_owned(),
        response_schema_id: "agent.semantic-protocols.semantic-search-packet".to_owned(),
    }
}

#[test]
fn contract_receipt_is_deterministic_and_valid() {
    let receipt = ProviderRuntimeContractReceipt::new(
        "rust-provider",
        "rust",
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111",
        "blake3-256:2222222222222222222222222222222222222222222222222222222222222222",
        ProviderRuntimeContractTransport::RuntimeIpcV1,
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
fn contract_receipt_rejects_duplicate_operations() {
    let mut receipt = ProviderRuntimeContractReceipt::new(
        "rust-provider",
        "rust",
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111",
        "blake3-256:2222222222222222222222222222222222222222222222222222222222222222",
        ProviderRuntimeContractTransport::RuntimeIpcV1,
        vec![operation()],
    )
    .expect("construct contract");
    receipt.operations.push(operation());
    assert!(receipt.validate().is_err());
}

#[test]
fn http_server_contract_is_a_first_class_runtime_transport() {
    let receipt = ProviderRuntimeContractReceipt::new(
        "julia-lang-project-harness",
        "julia",
        "blake3-256:3333333333333333333333333333333333333333333333333333333333333333",
        "blake3-256:4444444444444444444444444444444444444444444444444444444444444444",
        ProviderRuntimeContractTransport::HttpJsonV1,
        vec![operation()],
    )
    .expect("construct HTTP server contract");
    receipt.validate().expect("validate HTTP server contract");
    assert_eq!(
        receipt.transport,
        ProviderRuntimeContractTransport::HttpJsonV1
    );
}
