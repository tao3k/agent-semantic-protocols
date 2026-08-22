//! Durable workspace-generation restore and background live reconciliation.

use super::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionMode,
    WorkspaceGenerationAdmissionState, WorkspaceGenerationAdmissionTrigger,
    WorkspaceGenerationBuildMode, WorkspaceGenerationCandidateIdentity,
    WorkspaceGenerationRestoreFailure, WorkspaceGenerationRestoreReport,
};

fn durable_restore_candidate_identity() -> WorkspaceGenerationCandidateIdentity {
    let digest = format!(
        "blake3:{}",
        blake3::hash(b"agent.semantic-protocols.durable-restore-candidate.v1").to_hex()
    );
    WorkspaceGenerationCandidateIdentity {
        candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
            algorithm: "blake3-worktree-state-v1".to_owned(),
            digest: digest.clone(),
            authorities: vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex],
        },
        policy_overlay_digest: digest,
    }
}

impl WorkspaceGenerationAdmission {
    pub async fn restore_registered(&self) -> Result<WorkspaceGenerationRestoreReport, String> {
        let Some(catalog) = &self.catalog else {
            return Ok(WorkspaceGenerationRestoreReport::default());
        };
        let entries = catalog.snapshot();
        let mut tasks = tokio::task::JoinSet::new();
        let restore_permits = std::sync::Arc::new(tokio::sync::Semaphore::new(
            super::registered_workspace_restore_concurrency(),
        ));
        for entry in entries.iter().cloned() {
            let admission = self.clone();
            let restore_permits = std::sync::Arc::clone(&restore_permits);
            tasks.spawn(async move {
                let workspace_identity = entry.workspace_identity.clone();
                let restored = async {
                    let _restore_permit = restore_permits
                        .acquire_owned()
                        .await
                        .map_err(|_| "registered workspace restore lane closed".to_owned())?;
                    let candidate = durable_restore_candidate_identity();
                    let receipt = admission
                        .admit_with_mode(
                            entry.workspace_identity.clone(),
                            entry.project_root.clone(),
                            candidate,
                            // Startup owns registered workspace freshness:
                            // restore when possible, then rebuild stale or
                            // missing canonical materialization before Healthy.
                            WorkspaceGenerationBuildMode::RestoreOnly,
                            WorkspaceGenerationAdmissionTrigger::RuntimeRecovery,
                            WorkspaceGenerationAdmissionMode::FullRecovery,
                            None,
                            std::sync::Arc::default(),
                        )
                        .await?;
                    let receipt = if receipt.state == WorkspaceGenerationAdmissionState::Building {
                        admission
                            .wait_terminal(&entry.workspace_identity, &entry.project_root)
                            .await
                    } else {
                        Ok(receipt)
                    }?;
                    Ok(receipt)
                }
                .await;
                (workspace_identity, restored)
            });
        }
        let mut report = WorkspaceGenerationRestoreReport::default();
        while let Some(result) = tasks.join_next().await {
            let (workspace_identity, restored) = result
                .map_err(|error| format!("workspace admission restore task failed: {error}"))?;
            let receipt = match restored {
                Ok(receipt) => receipt,
                Err(error) => {
                    report.failed.push(WorkspaceGenerationRestoreFailure {
                        workspace_identity,
                        error,
                    });
                    continue;
                }
            };
            match receipt.state {
                WorkspaceGenerationAdmissionState::Ready => report.ready.push(receipt),
                WorkspaceGenerationAdmissionState::Failed
                | WorkspaceGenerationAdmissionState::Cancelled => {
                    report.failed.push(WorkspaceGenerationRestoreFailure {
                        workspace_identity: receipt.workspace_identity,
                        error: receipt
                            .error
                            .unwrap_or_else(|| "workspace generation restore failed".to_owned()),
                    });
                }
                WorkspaceGenerationAdmissionState::Building => {
                    report.failed.push(WorkspaceGenerationRestoreFailure {
                        workspace_identity,
                        error: "workspace admission restore returned a non-terminal receipt"
                            .to_owned(),
                    });
                }
            }
        }
        report
            .ready
            .sort_by(|left, right| left.workspace_identity.cmp(&right.workspace_identity));
        report
            .failed
            .sort_by(|left, right| left.workspace_identity.cmp(&right.workspace_identity));
        // Only after every registered workspace has a resident generation do
        // we begin live Git reconciliation. Healthy status therefore describes
        // the restored Server registry, not a partially restored catalog.
        Ok(report)
    }
}
