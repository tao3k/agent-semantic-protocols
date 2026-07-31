//! Memory-first ready admission for an already resident workspace generation.

use std::path::Path;

use super::model::WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID;
use super::{
    RuntimeDataPlaneCounters, RuntimeServerWorkspaceRegistry, WorkspaceGenerationState,
    WorkspaceRecoveryReceipt, WorkspaceRecoverySource,
};

impl RuntimeServerWorkspaceRegistry {
    /// Returns a typed ready receipt without reopening durable state when the generation is resident.
    pub fn ready_recovery_receipt(
        &self,
        request_id: impl Into<String>,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let lease = self.lease(workspace_identity, project_root)?;
        let target_epoch = lease.generation().active_epoch;
        let active_epoch = target_epoch
            .checked_sub(1)
            .ok_or_else(|| "runtime workspace generation epoch must be positive".to_owned())?;
        let receipt = WorkspaceRecoveryReceipt {
            schema_id: WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            request_id: request_id.into(),
            workspace_identity: workspace_identity.to_owned(),
            source: WorkspaceRecoverySource::MmapCheckpoint,
            state: WorkspaceGenerationState::Ready,
            active_epoch,
            target_epoch,
            old_generation_readable: active_epoch != 0,
            counters: RuntimeDataPlaneCounters::default(),
        };
        receipt.validate()?;
        Ok(receipt)
    }
}
