// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::sync::Arc;

use super::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionKey,
    WorkspaceGenerationAdmissionReceipt, WorkspaceGenerationAdmissionState,
    WorkspaceGenerationCandidateIdentity,
};

impl WorkspaceGenerationAdmission {
    pub fn current(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Option<WorkspaceGenerationAdmissionReceipt> {
        self.entries
            .get(&WorkspaceGenerationAdmissionKey {
                workspace_identity: workspace_identity.to_owned(),
            })
            .filter(|entry| entry.matches_project_root(project_root))
            .map(|entry| entry.observed())
    }

    pub fn status(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Option<WorkspaceGenerationAdmissionReceipt> {
        self.entries
            .get(&WorkspaceGenerationAdmissionKey {
                workspace_identity: workspace_identity.to_owned(),
            })
            .filter(|entry| entry.matches_project_root(project_root))
            .map(|entry| entry.receipt.borrow().clone())
    }

    pub async fn ensure(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
        candidate: WorkspaceGenerationCandidateIdentity,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        if let Some(entry) = self.entries.get(&WorkspaceGenerationAdmissionKey {
            workspace_identity: workspace_identity.to_owned(),
        }) && !entry.matches_project_root(project_root)
        {
            return Err(format!(
                "workspace generation admission root drift: workspaceIdentity={workspace_identity} requestedRoot={}",
                project_root.display()
            ));
        }
        candidate.validate()?;
        self.record_catalog_resident(
            crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry::resolve(
                workspace_identity.to_owned(),
                project_root.to_path_buf(),
            )?,
        )?;
        match self.status(workspace_identity, project_root) {
            Some(receipt)
                if !matches!(
                    receipt.state,
                    WorkspaceGenerationAdmissionState::Failed
                        | WorkspaceGenerationAdmissionState::Cancelled
                ) && receipt.candidate_generation == candidate.candidate_generation
                    && receipt.policy_overlay_digest == candidate.policy_overlay_digest =>
            {
                Ok(receipt)
            }
            Some(receipt)
                if matches!(
                    receipt.state,
                    WorkspaceGenerationAdmissionState::Failed
                        | WorkspaceGenerationAdmissionState::Cancelled
                ) =>
            {
                self.admit(
                    workspace_identity.to_owned(),
                    project_root.to_path_buf(),
                    candidate,
                )
                .await
            }
            Some(_) => {
                let mutation_id = format!(
                    "candidate-generation:{}:{}",
                    candidate.candidate_generation.digest, candidate.policy_overlay_digest
                );
                self.admit_mutation_candidate(
                    mutation_id,
                    workspace_identity.to_owned(),
                    project_root.to_path_buf(),
                    Arc::new(std::collections::BTreeSet::new()),
                    candidate,
                )
                .await
            }
            None => {
                self.admit(
                    workspace_identity.to_owned(),
                    project_root.to_path_buf(),
                    candidate,
                )
                .await
            }
        }
    }

    pub async fn wait_terminal(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        loop {
            let changed = self.changes.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let receipt = self
                .status(workspace_identity, project_root)
                .ok_or_else(|| {
                    format!(
                        "workspace generation admission is unknown: workspaceIdentity={workspace_identity} projectRoot={}",
                        project_root.display()
                    )
                })?;
            if !matches!(
                receipt.state,
                WorkspaceGenerationAdmissionState::Queued
                    | WorkspaceGenerationAdmissionState::Building
            ) {
                if let Some(catalog) = &self.catalog {
                    catalog.wait_durable().await?;
                }
                return Ok(receipt);
            }
            changed.as_mut().await;
        }
    }

    pub async fn wait_terminal_attempt(
        &self,
        workspace_identity: &str,
        project_root: &std::path::Path,
        expected_attempt: u64,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        loop {
            let changed = self.changes.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let receipt = self
                .status(workspace_identity, project_root)
                .ok_or_else(|| {
                    format!(
                        "workspace generation admission is unknown: workspaceIdentity={workspace_identity} projectRoot={}",
                        project_root.display()
                    )
                })?;
            if receipt.attempt > expected_attempt {
                return Err(format!(
                    "workspace generation admission advanced past the requested attempt: workspaceIdentity={workspace_identity} expectedAttempt={expected_attempt} actualAttempt={}",
                    receipt.attempt
                ));
            }
            if receipt.attempt == expected_attempt
                && !matches!(
                    receipt.state,
                    WorkspaceGenerationAdmissionState::Queued
                        | WorkspaceGenerationAdmissionState::Building
                )
            {
                if let Some(catalog) = &self.catalog {
                    catalog.wait_durable().await?;
                }
                return Ok(receipt);
            }
            changed.as_mut().await;
        }
    }
}
