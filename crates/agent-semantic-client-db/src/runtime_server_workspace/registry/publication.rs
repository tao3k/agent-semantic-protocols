//! Typed publication commands submitted to a resident workspace writer.

use std::path::PathBuf;

use tokio::sync::oneshot;

use super::{RuntimeServerWorkspaceRegistry, WorkspaceWriteCommand};
use crate::runtime_server_workspace::{
    WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceRecoveryReceipt,
    WorkspaceRecoverySource, WorkspaceRuntimeSelectorOverlay,
    WorkspaceRuntimeSelectorOverlayReceipt,
};

impl RuntimeServerWorkspaceRegistry {
    pub async fn publish_selector_overlay(
        &self,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        overlay: WorkspaceRuntimeSelectorOverlay,
    ) -> Result<WorkspaceRuntimeSelectorOverlayReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::PublishSelectorOverlay {
                target: entry.write_target(),
                workspace_identity,
                overlay,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive.await.map_err(|_| {
            "runtime workspace writer lane dropped selector overlay receipt".to_owned()
        })?
    }

    pub async fn publish(
        &self,
        request_id: impl Into<String>,
        source: WorkspaceRecoverySource,
        generation: WorkspaceMemoryGeneration,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        generation.validate()?;
        let entry = self
            .entry(
                &generation.workspace_identity,
                std::path::Path::new(&generation.project_root),
            )
            .await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::Publish {
                target: entry.write_target(),
                request_id: request_id.into(),
                source,
                generation,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn publish_owner_overlay(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        owner: WorkspaceOwnerSnapshot,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::PublishOwnerOverlay {
                target: entry.write_target(),
                request_id: request_id.into(),
                workspace_identity,
                owner,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn tombstone_owner_overlay(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        owner_path: impl Into<String>,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::TombstoneOwnerOverlay {
                target: entry.write_target(),
                request_id: request_id.into(),
                workspace_identity,
                owner_path: owner_path.into(),
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn relocate_owner_overlay(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        previous_owner_path: impl Into<String>,
        owner: WorkspaceOwnerSnapshot,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::RelocateOwnerOverlay {
                target: entry.write_target(),
                request_id: request_id.into(),
                workspace_identity,
                previous_owner_path: previous_owner_path.into(),
                owner,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }

    pub async fn ensure_canonical_generation(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        const FOREGROUND_ACCEPTANCE_DEADLINE: std::time::Duration =
            std::time::Duration::from_millis(100);
        let request_id = request_id.into();
        let workspace_identity = workspace_identity.into();
        materialization.validate_persisted(&workspace_identity)?;
        let entry = self
            .entry(
                &workspace_identity,
                std::path::Path::new(&materialization.project_root),
            )
            .await?;
        let (accepted, acceptance) = oneshot::channel();
        let (reply, receive) = oneshot::channel();
        let acceptance_deadline = tokio::time::Instant::now() + FOREGROUND_ACCEPTANCE_DEADLINE;
        let send = entry
            .writer
            .send(WorkspaceWriteCommand::EnsureCanonicalGeneration {
                target: entry.write_target(),
                request_id,
                workspace_identity,
                materialization,
                accepted,
                reply,
            });
        match tokio::time::timeout_at(acceptance_deadline, send).await {
            Ok(result) => {
                result.map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?
            }
            Err(_) => {
                return Err(format!(
                    "runtime workspace writer lane exceeded its {} ms enqueue deadline",
                    FOREGROUND_ACCEPTANCE_DEADLINE.as_millis()
                ));
            }
        }
        let _receive = receive;
        Self::await_recovery_acceptance(acceptance, acceptance_deadline).await?;
        _receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its completion".to_owned())?
    }

    async fn await_recovery_acceptance(
        acceptance: oneshot::Receiver<Result<WorkspaceRecoveryReceipt, String>>,
        deadline: tokio::time::Instant,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        match tokio::time::timeout_at(deadline, acceptance).await {
            Ok(receipt) => receipt
                .map_err(|_| "runtime workspace writer lane dropped its acceptance".to_owned())?,
            Err(_) => {
                Err("runtime workspace writer lane exceeded its acceptance deadline".to_owned())
            }
        }
    }

    pub async fn restore_checkpoint(
        &self,
        request_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: &std::path::Path,
        path: PathBuf,
    ) -> Result<WorkspaceRecoveryReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let entry = self.entry(&workspace_identity, project_root).await?;
        let (reply, receive) = oneshot::channel();
        entry
            .writer
            .send(WorkspaceWriteCommand::RestoreCheckpoint {
                target: entry.write_target(),
                request_id: request_id.into(),
                workspace_identity,
                path,
                reply,
            })
            .await
            .map_err(|_| "runtime workspace writer lane is unavailable".to_owned())?;
        receive
            .await
            .map_err(|_| "runtime workspace writer lane dropped its receipt".to_owned())?
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/runtime_server_workspace_registry_publication.rs"]
mod acceptance_tests;
