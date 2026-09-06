// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Runtime-owned full generation admission after immutable artifact publication.

use std::path::PathBuf;
use std::sync::Arc;

use super::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionMode,
    WorkspaceGenerationAdmissionReceipt, WorkspaceGenerationAdmissionState,
    WorkspaceGenerationAdmissionTrigger, WorkspaceGenerationBuildMode,
    discover_workspace_generation_candidate,
};

impl WorkspaceGenerationAdmission {
    /// Rebuild an admitted generation whose resident mmap layout cannot be opened.
    ///
    /// The admission authority remains the sole build and publication owner. The
    /// caller supplies no paths, deletes no artifacts, and cannot restore the
    /// incompatible generation.
    pub async fn admit_runtime_recovery_and_wait(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        let candidate = discover_workspace_generation_candidate(&project_root).await?;
        let receipt = self
            .admit_with_mode(
                workspace_identity.clone(),
                project_root.clone(),
                candidate,
                WorkspaceGenerationBuildMode::RebuildAfterMutation,
                WorkspaceGenerationAdmissionTrigger::RuntimeRecovery,
                WorkspaceGenerationAdmissionMode::FullRecovery,
                None,
                Arc::default(),
            )
            .await?;
        if matches!(
            receipt.state,
            WorkspaceGenerationAdmissionState::Queued | WorkspaceGenerationAdmissionState::Building
        ) {
            self.wait_terminal(&workspace_identity, &project_root).await
        } else {
            Ok(receipt)
        }
    }

    /// Builds or restores one complete generation and waits for its terminal receipt.
    ///
    /// This is a cold lifecycle operation. The Runtime remains the only candidate,
    /// provider, generation, and publication owner; the caller never polls or builds.
    pub async fn admit_artifact_publication_and_wait(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        language_id: String,
        provider_id: String,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        self.admit_artifact_publication_with_mode_and_wait(
            workspace_identity,
            project_root,
            language_id,
            provider_id,
            WorkspaceGenerationBuildMode::RestoreOrBuild,
        )
        .await
    }

    pub(crate) async fn admit_artifact_publication_with_mode_and_wait(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        language_id: String,
        provider_id: String,
        build_mode: WorkspaceGenerationBuildMode,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        if workspace_identity.trim().is_empty() {
            return Err("artifact publication admission identity must be non-empty".to_owned());
        }
        if !project_root.is_absolute() {
            return Err("artifact publication admission root must be absolute".to_owned());
        }
        if language_id.trim().is_empty() || provider_id.trim().is_empty() {
            return Err(
                "artifact publication admission provider target must be non-empty".to_owned(),
            );
        }

        if self
            .current(&workspace_identity, &project_root)
            .is_some_and(|receipt| {
                matches!(
                    receipt.state,
                    WorkspaceGenerationAdmissionState::Queued
                        | WorkspaceGenerationAdmissionState::Building
                )
            })
        {
            let _ = self
                .wait_terminal(&workspace_identity, &project_root)
                .await?;
        }

        let candidate = discover_workspace_generation_candidate(&project_root).await?;
        let receipt = self
            .admit_with_mode(
                workspace_identity.clone(),
                project_root.clone(),
                candidate,
                build_mode,
                WorkspaceGenerationAdmissionTrigger::ArtifactPublication,
                WorkspaceGenerationAdmissionMode::FullRecovery,
                Some(
                    crate::runtime_server_admission::WorkspaceGenerationProviderTarget {
                        language_id,
                        provider_id: Some(provider_id),
                    },
                ),
                Arc::default(),
            )
            .await?;

        if matches!(
            receipt.state,
            WorkspaceGenerationAdmissionState::Queued | WorkspaceGenerationAdmissionState::Building
        ) {
            self.wait_terminal(&workspace_identity, &project_root).await
        } else {
            Ok(receipt)
        }
    }
}
