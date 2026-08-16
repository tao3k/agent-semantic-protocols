use super::*;
use crate::{ProviderRuntimeContractOperation, ProviderRuntimeContractTransport};

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn contract() -> ProviderRuntimeContractReceipt {
    ProviderRuntimeContractReceipt::new(
        "rs-harness",
        "rust",
        digest('a'),
        digest('b'),
        ProviderRuntimeContractTransport::RuntimeIpcV1,
        vec![ProviderRuntimeContractOperation {
            operation: "projection-batch-stdin".to_owned(),
            request_schema_id: "agent.semantic-protocols.language-projection-batch-request.v1"
                .to_owned(),
            response_schema_id: "agent.semantic-protocols.language-projection-batch-response.v1"
                .to_owned(),
        }],
    )
    .expect("provider runtime contract")
}

#[test]
fn authority_receipt_exposes_only_committed_ready_contracts() {
    let expected = contract();
    let building = ProviderRuntimeAuthorityReceipt::from_actor_state(
        &expected,
        ProviderRuntimeActorState::Starting,
    )
    .expect("Building authority receipt");
    assert_eq!(building.state, ProviderRuntimeAuthorityState::Starting);
    assert!(building.contract_receipt.is_none());

    let ready = ProviderRuntimeAuthorityReceipt::from_actor_state(
        &expected,
        ProviderRuntimeActorState::Ready(expected.clone()),
    )
    .expect("Ready authority receipt");
    assert_eq!(ready.state, ProviderRuntimeAuthorityState::Ready);
    assert_eq!(ready.contract_receipt, Some(expected));
}

#[test]
fn authority_receipt_fails_closed_on_contract_drift() {
    let expected = contract();
    let mut drifted = expected.clone();
    drifted.contract_digest = digest('c');
    let error = ProviderRuntimeAuthorityReceipt::from_actor_state(
        &expected,
        ProviderRuntimeActorState::Ready(drifted),
    )
    .expect_err("drifted Ready contract must fail closed");
    assert_eq!(error, "provider-runtime-contract-drift");
}

#[test]
fn authority_receipt_preserves_failed_and_stopped_terminal_states() {
    let expected = contract();
    let failed = ProviderRuntimeAuthorityReceipt::from_actor_state(
        &expected,
        ProviderRuntimeActorState::Failed("handshake failed".to_owned()),
    )
    .expect("Failed authority receipt");
    assert_eq!(failed.state, ProviderRuntimeAuthorityState::Failed);
    assert_eq!(failed.error.as_deref(), Some("handshake failed"));

    let stopped = ProviderRuntimeAuthorityReceipt::from_actor_state(
        &expected,
        ProviderRuntimeActorState::Stopped,
    )
    .expect("Stopped authority receipt");
    assert_eq!(stopped.state, ProviderRuntimeAuthorityState::Stopped);
    assert!(stopped.error.is_none());
}
