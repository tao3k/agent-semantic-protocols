//! Runtime-owned cold CompleteGeneration barrier established before Ready reads.

use std::path::PathBuf;
use std::sync::Arc;

use super::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionMode,
    WorkspaceGenerationAdmissionState, WorkspaceGenerationAdmissionTrigger,
    WorkspaceGenerationBuildMode, discover_workspace_generation_candidate,
};

impl WorkspaceGenerationAdmission {
    /// Establish the single complete-generation read barrier used by all
    /// language Search/Query routes on a client connection.
    ///
    /// A queued receipt is never a read barrier. Targeted provider admissions
    /// are also insufficient: this operation either joins their terminal and
    /// follows with one complete publication, or owns that complete publication
    /// itself. Replayed Ready receipts remain valid even though `accepted` is
    /// edge-triggered and therefore false.
    pub async fn ensure_runtime_generation_ready(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
    ) -> Result<super::WorkspaceGenerationAdmissionReceipt, String> {
        if workspace_identity.trim().is_empty() {
            return Err("workspace generation ensure-ready identity must be non-empty".to_owned());
        }
        if !project_root.is_absolute() {
            return Err("workspace generation ensure-ready root must be absolute".to_owned());
        }

        loop {
            if let Some(observed) = self.current(&workspace_identity, &project_root) {
                if matches!(
                    observed.state,
                    WorkspaceGenerationAdmissionState::Queued
                        | WorkspaceGenerationAdmissionState::Building
                ) {
                    self.wait_terminal_attempt(
                        &workspace_identity,
                        &project_root,
                        observed.attempt,
                    )
                    .await?;
                    continue;
                }
                if observed.state == WorkspaceGenerationAdmissionState::Ready
                    && observed.admission_mode == WorkspaceGenerationAdmissionMode::FullRecovery
                    && observed.commit.is_some()
                {
                    observed.validate()?;
                    return Ok(observed);
                }
            }

            let candidate = discover_workspace_generation_candidate(&project_root).await?;
            let build_mode = if self
                .current(&workspace_identity, &project_root)
                .is_some_and(|receipt| receipt.state == WorkspaceGenerationAdmissionState::Ready)
            {
                WorkspaceGenerationBuildMode::RebuildAfterMutation
            } else {
                WorkspaceGenerationBuildMode::RestoreOrBuild
            };
            let queued = self
                .admit_with_mode(
                    workspace_identity.clone(),
                    project_root.clone(),
                    candidate,
                    build_mode,
                    WorkspaceGenerationAdmissionTrigger::QueryDemand,
                    WorkspaceGenerationAdmissionMode::FullRecovery,
                    None,
                    Arc::default(),
                )
                .await?;
            let terminal = if matches!(
                queued.state,
                WorkspaceGenerationAdmissionState::Queued
                    | WorkspaceGenerationAdmissionState::Building
            ) {
                self.wait_terminal_attempt(&workspace_identity, &project_root, queued.attempt)
                    .await?
            } else {
                queued
            };
            terminal.validate()?;
            if terminal.state != WorkspaceGenerationAdmissionState::Ready
                || terminal.admission_mode != WorkspaceGenerationAdmissionMode::FullRecovery
                || terminal.commit.is_none()
            {
                return Err(serde_json::json!({
                    "schemaId": "agent.semantic-protocols.query-not-ready",
                    "schemaVersion": "1",
                    "reasonKind": "query-not-ready",
                    "workspaceIdentity": workspace_identity,
                    "accepted": terminal.accepted,
                    "attempt": terminal.attempt,
                    "state": terminal.state,
                    "admissionMode": terminal.admission_mode,
                    "commit": terminal.commit,
                    "error": terminal.error,
                })
                .to_string());
            }
            return Ok(terminal);
        }
    }
}
