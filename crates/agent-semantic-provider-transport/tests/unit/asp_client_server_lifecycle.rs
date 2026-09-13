// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::AspClientServerLifecycleReceipt;
use super::AspClientServerLifecycleState;
use super::ProviderRuntimeActorState;
use super::ProviderRuntimeContractReceipt;
use crate::ProviderRuntimeContractOperation;
use crate::ProviderRuntimeContractTransport;

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn contract() -> ProviderRuntimeContractReceipt {
    ProviderRuntimeContractReceipt::new(
        "asp-rust",
        "rust",
        digest('a'),
        digest('b'),
        ProviderRuntimeContractTransport::RuntimeIpc,
        vec![ProviderRuntimeContractOperation {
            operation: "projection-batch-stdin".to_owned(),
            request_schema: agent_semantic_provider_protocol::ProviderSchemaReference {
                schema_id: "agent.semantic-protocols.language-projection-batch-request.v1"
                    .to_owned(),
                schema_version: "1".to_owned(),
            },
            response_schema: agent_semantic_provider_protocol::ProviderSchemaReference {
                schema_id: "agent.semantic-protocols.language-projection-batch-response.v1"
                    .to_owned(),
                schema_version: "1".to_owned(),
            },
        }],
    )
    .expect("provider runtime contract")
}

#[test]
fn lifecycle_receipt_exposes_only_committed_ready_contracts() {
    let expected = contract();
    let building = AspClientServerLifecycleReceipt::from_actor_state(
        &expected,
        ProviderRuntimeActorState::Starting,
    )
    .expect("Building authority receipt");
    assert_eq!(building.state, AspClientServerLifecycleState::Starting);
    assert_eq!(building.publication_epoch, 0);

    let ready = AspClientServerLifecycleReceipt::from_actor_state(
        &expected,
        ProviderRuntimeActorState::Ready(expected.clone()),
    )
    .expect("Ready authority receipt");
    assert_eq!(ready.state, AspClientServerLifecycleState::Ready);
    assert_eq!(ready.publication_epoch, 1);
    assert_eq!(ready.runtime_contract_digest, expected.contract_digest);
    assert_eq!(ready.operations, vec!["projection-batch-stdin"]);
}

#[test]
fn lifecycle_receipt_fails_closed_on_contract_drift() {
    let expected = contract();
    let mut drifted = expected.clone();
    drifted.contract_digest = digest('c');
    let error = AspClientServerLifecycleReceipt::from_actor_state(
        &expected,
        ProviderRuntimeActorState::Ready(drifted),
    )
    .expect_err("drifted Ready contract must fail closed");
    assert_eq!(error, "provider-runtime-contract-drift");
}

#[test]
fn lifecycle_receipt_preserves_failed_and_stopped_terminal_states() {
    let expected = contract();
    let failed = AspClientServerLifecycleReceipt::from_actor_state(
        &expected,
        ProviderRuntimeActorState::Failed("handshake failed".to_owned()),
    )
    .expect("Failed authority receipt");
    assert_eq!(failed.state, AspClientServerLifecycleState::Failed);
    assert_eq!(failed.error.as_deref(), Some("handshake failed"));

    let stopped = AspClientServerLifecycleReceipt::from_actor_state(
        &expected,
        ProviderRuntimeActorState::Stopped,
    )
    .expect("Stopped authority receipt");
    assert_eq!(stopped.state, AspClientServerLifecycleState::Stopped);
    assert!(stopped.error.is_none());
}
