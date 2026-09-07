// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed lifecycle receipt for the Runtime-owned ASP Client Server authority.

use serde::Deserialize;
use serde::Serialize;

use crate::ProviderRuntimeActorState;
use crate::ProviderRuntimeContractReceipt;

pub const ASP_CLIENT_SERVER_LIFECYCLE_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-server-lifecycle-receipt";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AspClientServerLifecycleState {
    Starting,
    Warming,
    Ready,
    Draining,
    Failed,
    Stopped,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientServerLifecycleReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub provider_id: String,
    pub language_id: String,
    pub state: AspClientServerLifecycleState,
    pub attempt: u64,
    pub publication_epoch: u64,
    pub artifact_digest: String,
    pub registration_digest: String,
    pub runtime_contract_digest: String,
    pub endpoint: Option<String>,
    pub operations: Vec<String>,
    pub error: Option<String>,
}

impl AspClientServerLifecycleReceipt {
    pub fn from_actor_state(
        expected: &ProviderRuntimeContractReceipt,
        state: ProviderRuntimeActorState,
    ) -> Result<Self, String> {
        let (state, ready, error) = match state {
            ProviderRuntimeActorState::Starting => {
                (AspClientServerLifecycleState::Starting, false, None)
            }
            ProviderRuntimeActorState::Warming => {
                (AspClientServerLifecycleState::Warming, false, None)
            }
            ProviderRuntimeActorState::Ready(receipt) => {
                if receipt != *expected {
                    return Err("provider-runtime-contract-drift".to_owned());
                }
                (AspClientServerLifecycleState::Ready, true, None)
            }
            ProviderRuntimeActorState::Draining => {
                (AspClientServerLifecycleState::Draining, false, None)
            }
            ProviderRuntimeActorState::Failed(error) => {
                (AspClientServerLifecycleState::Failed, false, Some(error))
            }
            ProviderRuntimeActorState::Stopped => {
                (AspClientServerLifecycleState::Stopped, false, None)
            }
        };
        let receipt = Self {
            schema_id: ASP_CLIENT_SERVER_LIFECYCLE_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            provider_id: expected.provider_id.clone(),
            language_id: expected.language_id.clone(),
            state,
            attempt: 1,
            publication_epoch: u64::from(ready),
            artifact_digest: expected.artifact_digest.clone(),
            registration_digest: expected.registration_digest.clone(),
            runtime_contract_digest: expected.contract_digest.clone(),
            endpoint: None,
            operations: expected
                .operations
                .iter()
                .map(|operation| operation.operation.clone())
                .collect(),
            error,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != ASP_CLIENT_SERVER_LIFECYCLE_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
            || self.provider_id.trim().is_empty()
            || self.language_id.trim().is_empty()
            || self.attempt == 0
            || self.artifact_digest.trim().is_empty()
            || self.registration_digest.trim().is_empty()
            || self.runtime_contract_digest.trim().is_empty()
        {
            return Err("ASP Client Server lifecycle receipt identity is invalid".to_owned());
        }
        match self.state {
            AspClientServerLifecycleState::Ready => {
                if self.publication_epoch == 0 || self.error.is_some() {
                    return Err("Ready ASP Client Server lifecycle identity drift".to_owned());
                }
            }
            AspClientServerLifecycleState::Failed => {
                if self.error.as_deref().is_none_or(str::is_empty) {
                    return Err("Failed ASP Client Server lifecycle receipt is invalid".to_owned());
                }
            }
            AspClientServerLifecycleState::Starting
            | AspClientServerLifecycleState::Warming
            | AspClientServerLifecycleState::Draining
            | AspClientServerLifecycleState::Stopped => {
                if self.error.is_some() {
                    return Err(
                        "non-Ready ASP Client Server lifecycle carried terminal data".to_owned(),
                    );
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/unit/asp_client_server_lifecycle.rs"]
mod tests;
