// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Separates durable restart restoration from validated resident replacement.

use std::{path::Path, sync::Arc};

use super::core::WorkspaceEntryBootstrap;
use crate::runtime_server_workspace::{
    WorkspaceGenerationDataPlaneClient, WorkspaceGenerationDataPlaneOpen,
    WorkspaceGenerationPointerReader, WorkspaceGenerationPublisher, WorkspaceMemoryBackend,
};

pub(super) struct InitializedWorkspaceEntry {
    pub(super) restored: Option<Arc<WorkspaceMemoryBackend>>,
    pub(super) persisted_epoch: u64,
    pub(super) publisher: Arc<WorkspaceGenerationPublisher>,
}

pub(super) async fn initialize(
    directory: std::path::PathBuf,
    workspace_identity: &str,
    project_root: &Path,
    bootstrap: WorkspaceEntryBootstrap,
) -> Result<InitializedWorkspaceEntry, String> {
    let pointer_path = directory.join("active-generation.pointer");
    let (restored, persisted_epoch) = match bootstrap {
        WorkspaceEntryBootstrap::RestoreDurable => {
            let restored =
                match WorkspaceGenerationDataPlaneClient::open_state(&pointer_path).await? {
                    WorkspaceGenerationDataPlaneOpen::Ready(client) => {
                        Some(Arc::clone(&client.lease().backend))
                    }
                    WorkspaceGenerationDataPlaneOpen::Missing
                    | WorkspaceGenerationDataPlaneOpen::RecoveryRequired { .. } => None,
                };
            let persisted_epoch = if let Some(backend) = restored.as_ref() {
                backend.generation().active_epoch
            } else {
                persisted_generation_epoch(&pointer_path).await
            };
            (restored, persisted_epoch)
        }
        WorkspaceEntryBootstrap::ReplaceWithValidatedResident => {
            // The new bytes are already validated. Reading only fixed-size
            // pointer metadata preserves epoch monotonicity without putting
            // old generation decoding in front of `ResidentReady`.
            (None, persisted_generation_epoch(&pointer_path).await)
        }
    };
    let publisher = Arc::new(WorkspaceGenerationPublisher::new(directory).await?);
    if let Some(backend) = restored.as_ref() {
        // Projection segments are reconstructible. Preserve the generic
        // backend if projection authority is incompatible so the writer can
        // atomically republish the complete generation.
        let _projection_restore = publisher
            .restore_search_generation_authority(
                workspace_identity,
                project_root.to_string_lossy().as_ref(),
                backend.generation().active_epoch,
            )
            .await;
    }
    Ok(InitializedWorkspaceEntry {
        restored,
        persisted_epoch,
        publisher,
    })
}

async fn persisted_generation_epoch(pointer_path: &Path) -> u64 {
    let Ok(Some(reader)) = WorkspaceGenerationPointerReader::open_optional(pointer_path).await
    else {
        return 0;
    };
    let Ok(snapshot) = reader.read() else {
        return 0;
    };
    if snapshot.validate().is_err() {
        return 0;
    }
    snapshot.active_epoch
}
