// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Validation for Runtime-owned workspace generation admission receipts.

use super::{
    WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID, WorkspaceGenerationAdmissionReceipt,
    WorkspaceGenerationAdmissionState, WorkspaceGenerationCandidateIdentity,
    WorkspaceGenerationCommitReceipt,
};

impl WorkspaceGenerationCommitReceipt {
    /// Validates that the committed generation and projection authority are one epoch.
    pub fn validate_projection_capability(&self) -> Result<(), String> {
        self.projection_capability.validate()?;
        if self.projection_capability.state
            != crate::active_generation_projection_capability::ActiveGenerationCapabilityState::Ready
        {
            return Err("committed generation projection capability is not ready".to_owned());
        }
        if self.projection_capability.publication_epoch != self.active_epoch {
            return Err("committed generation projection capability epoch drift".to_owned());
        }
        if self.projection_capability.generation_digest != self.generation_digest {
            return Err(
                "committed generation projection capability generation digest drift".to_owned(),
            );
        }
        if self.projection_capability.root_digest != self.source_root_digest {
            return Err("committed generation projection capability root digest drift".to_owned());
        }
        Ok(())
    }

    pub fn from_recovery(
        recovery: &crate::runtime_server_workspace::WorkspaceRecoveryReceipt,
    ) -> Result<Self, String> {
        recovery.validate()?;
        let receipt = Self {
            projection_capability: recovery.projection_capability.clone(),
            active_epoch: recovery.target_epoch,
            generation_digest: recovery.generation_digest.clone(),
            source_root_digest: recovery.source_root_digest.clone(),
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.active_epoch == 0
            || self.generation_digest.trim().is_empty()
            || self.source_root_digest.trim().is_empty()
        {
            return Err("workspace generation commit receipt is incomplete".to_owned());
        }
        Ok(())
    }
}

impl WorkspaceGenerationAdmissionReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err(format!(
                "workspace generation admission receipt schema mismatch: observed schemaId={} schemaVersion={} expected schemaId={} schemaVersion=1",
                self.schema_id,
                self.schema_version,
                WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID,
            ));
        }
        if !matches!(
            self.trigger.as_str(),
            "query-demand"
                | "workspace-change"
                | "operator-mutation"
                | "artifact-publication"
                | "runtime-recovery"
        ) {
            return Err(format!(
                "workspace generation admission receipt trigger is unsupported: {}",
                self.trigger
            ));
        }
        if !matches!(
            self.admission_mode.as_str(),
            "complete-generation" | "full-recovery"
        ) {
            return Err(format!(
                "workspace generation admission receipt mode is unsupported: {}",
                self.admission_mode
            ));
        }
        if self.build_owner != "runtime-server"
            || self.cancellation_authority != "runtime-server"
            || !self.request_lifetime_independent
        {
            return Err(
                "workspace generation admission receipt is not Runtime-owned and request-independent"
                    .to_owned(),
            );
        }
        if self.workspace_identity.trim().is_empty() || self.attempt == 0 {
            return Err("workspace generation admission receipt identity is incomplete".to_owned());
        }
        WorkspaceGenerationCandidateIdentity {
            candidate_generation: self.candidate_generation.clone(),
            policy_overlay_digest: self.policy_overlay_digest.clone(),
        }
        .validate()?;
        match (&self.state, &self.commit, &self.error, &self.failure_stage) {
            (
                WorkspaceGenerationAdmissionState::Queued
                | WorkspaceGenerationAdmissionState::Building,
                None,
                None,
                None,
            ) => Ok(()),
            (WorkspaceGenerationAdmissionState::Failed, None, Some(_), Some(_))
            | (WorkspaceGenerationAdmissionState::Cancelled, None, Some(_), Some(_)) => Ok(()),
            (WorkspaceGenerationAdmissionState::Ready, Some(commit), None, None) => {
                commit.validate()
            }
            _ => Err("workspace generation admission receipt state is inconsistent".to_owned()),
        }
    }
}
