//! Typed lifecycle receipt for the Runtime-owned resident provider authority.

use serde::{Deserialize, Serialize};

use crate::{ProviderRuntimeActorState, ProviderRuntimeContractReceipt};

pub const PROVIDER_RUNTIME_AUTHORITY_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-runtime-authority-receipt";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRuntimeAuthorityState {
    Starting,
    Warming,
    Ready,
    Draining,
    Failed,
    Stopped,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRuntimeAuthorityReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub provider_id: String,
    pub language_id: String,
    pub state: ProviderRuntimeAuthorityState,
    pub contract_receipt: Option<ProviderRuntimeContractReceipt>,
    pub error: Option<String>,
}

impl ProviderRuntimeAuthorityReceipt {
    pub fn from_actor_state(
        expected: &ProviderRuntimeContractReceipt,
        state: ProviderRuntimeActorState,
    ) -> Result<Self, String> {
        let (state, contract_receipt, error) = match state {
            ProviderRuntimeActorState::Starting => {
                (ProviderRuntimeAuthorityState::Starting, None, None)
            }
            ProviderRuntimeActorState::Warming => {
                (ProviderRuntimeAuthorityState::Warming, None, None)
            }
            ProviderRuntimeActorState::Ready(receipt) => {
                if receipt != *expected {
                    return Err("provider-runtime-contract-drift".to_owned());
                }
                (ProviderRuntimeAuthorityState::Ready, Some(receipt), None)
            }
            ProviderRuntimeActorState::Draining => {
                (ProviderRuntimeAuthorityState::Draining, None, None)
            }
            ProviderRuntimeActorState::Failed(error) => {
                (ProviderRuntimeAuthorityState::Failed, None, Some(error))
            }
            ProviderRuntimeActorState::Stopped => {
                (ProviderRuntimeAuthorityState::Stopped, None, None)
            }
        };
        let receipt = Self {
            schema_id: PROVIDER_RUNTIME_AUTHORITY_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            provider_id: expected.provider_id.clone(),
            language_id: expected.language_id.clone(),
            state,
            contract_receipt,
            error,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != PROVIDER_RUNTIME_AUTHORITY_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
            || self.provider_id.trim().is_empty()
            || self.language_id.trim().is_empty()
        {
            return Err("provider runtime authority receipt identity is invalid".to_owned());
        }
        match self.state {
            ProviderRuntimeAuthorityState::Ready => {
                let contract = self.contract_receipt.as_ref().ok_or_else(|| {
                    "Ready provider runtime authority omitted contractReceipt".to_owned()
                })?;
                contract.validate()?;
                if contract.provider_id != self.provider_id
                    || contract.language_id != self.language_id
                    || self.error.is_some()
                {
                    return Err("Ready provider runtime authority identity drift".to_owned());
                }
            }
            ProviderRuntimeAuthorityState::Failed => {
                if self.contract_receipt.is_some()
                    || self.error.as_deref().is_none_or(str::is_empty)
                {
                    return Err("Failed provider runtime authority receipt is invalid".to_owned());
                }
            }
            ProviderRuntimeAuthorityState::Starting
            | ProviderRuntimeAuthorityState::Warming
            | ProviderRuntimeAuthorityState::Draining
            | ProviderRuntimeAuthorityState::Stopped => {
                if self.contract_receipt.is_some() || self.error.is_some() {
                    return Err(
                        "non-Ready provider runtime authority carried terminal data".to_owned()
                    );
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_authority.rs"]
mod tests;
