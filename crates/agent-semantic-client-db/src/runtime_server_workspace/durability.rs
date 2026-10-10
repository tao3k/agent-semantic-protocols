// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use serde::{Deserialize, Serialize};

pub const WORKSPACE_GENERATION_DURABILITY_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-generation-durability.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationDurabilityState {
    ResidentReady,
    DurableReady,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationDurabilityReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub generation_digest: String,
    pub target_epoch: u64,
    pub state: WorkspaceGenerationDurabilityState,
    pub failure: Option<String>,
}

impl WorkspaceGenerationDurabilityReceipt {
    pub fn new(
        workspace_identity: impl Into<String>,
        generation_digest: impl Into<String>,
        target_epoch: u64,
        state: WorkspaceGenerationDurabilityState,
        failure: Option<String>,
    ) -> Result<Self, String> {
        let receipt = Self {
            schema_id: WORKSPACE_GENERATION_DURABILITY_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.into(),
            generation_digest: generation_digest.into(),
            target_epoch,
            state,
            failure,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_GENERATION_DURABILITY_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err(
                "workspace generation durability receipt schema identity mismatch".to_owned(),
            );
        }
        if self.workspace_identity.trim().is_empty()
            || self.generation_digest.trim().is_empty()
            || self.target_epoch == 0
        {
            return Err(
                "workspace generation durability receipt identity is incomplete".to_owned(),
            );
        }
        match (self.state, self.failure.as_deref()) {
            (WorkspaceGenerationDurabilityState::Failed, Some(failure))
                if !failure.trim().is_empty() => {}
            (WorkspaceGenerationDurabilityState::Failed, _) => {
                return Err("failed workspace durability receipt requires a failure".to_owned());
            }
            (_, None) => {}
            (_, Some(_)) => {
                return Err(
                    "successful workspace durability receipt cannot contain a failure".to_owned(),
                );
            }
        }
        Ok(())
    }
}
