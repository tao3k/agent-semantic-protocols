//! Memory-first ready admission for an already resident workspace generation.

use std::path::Path;

use super::model::WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID;
use super::{
    RuntimeDataPlaneCounters, RuntimeServerWorkspaceRegistry, WorkspaceGenerationState,
    WorkspaceRecoveryReceipt, WorkspaceRecoverySource,
};

impl RuntimeServerWorkspaceRegistry {
    pub async fn published_generation_state(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<super::PublishedWorkspaceGenerationState, String> {
        let pointer_path =
            super::workspace_generation_pointer_path(&self.root, workspace_identity, project_root)?;
        match super::WorkspaceGenerationDataPlaneClient::open_state(&pointer_path).await? {
            super::WorkspaceGenerationDataPlaneOpen::Ready(_client) => {
                // The lease was opened from a BLAKE3-authenticated committed segment. Full typed-digest
                // validation belongs to publication, not the resident readiness hot path.
                Ok(super::PublishedWorkspaceGenerationState::Ready)
            }
            super::WorkspaceGenerationDataPlaneOpen::Missing => {
                Ok(super::PublishedWorkspaceGenerationState::Missing)
            }
            super::WorkspaceGenerationDataPlaneOpen::RecoveryRequired { reason } => {
                Ok(super::PublishedWorkspaceGenerationState::RecoveryRequired { reason })
            }
        }
    }

    pub async fn restore_published_generation(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &Path,
    ) -> Result<super::WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let pointer_path = super::workspace_generation_pointer_path(
            &self.root,
            &workspace_identity,
            project_root,
        )?;
        let snapshot = super::WorkspaceGenerationPointerReader::open(&pointer_path)
            .await?
            .read()?;
        self.restore_checkpoint(
            request_id,
            workspace_identity,
            project_root,
            snapshot.mmap_segment_path.into(),
        )
        .await
    }

    /// Returns a typed ready receipt without reopening durable state when the generation is resident.
    pub fn ready_recovery_receipt(
        &self,
        request_id: impl Into<String>,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let lease = self.lease(workspace_identity, project_root)?;
        let generation = lease.generation();
        let target_epoch = generation.active_epoch;
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
            generation_digest: generation.generation_digest.clone(),
            source_root_digest: generation.source_snapshot.root_digest.clone(),
            old_generation_readable: active_epoch != 0,
            resident_publication_elapsed_micros: 0,
            counters: RuntimeDataPlaneCounters::default(),
        };
        receipt.validate()?;
        Ok(receipt)
    }
}
