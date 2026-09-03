//! Runtime-owned cold CompleteGeneration barrier established before Ready reads.

use std::path::PathBuf;
use std::sync::Arc;

use super::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionKey,
    WorkspaceGenerationAdmissionMode, WorkspaceGenerationAdmissionState,
    WorkspaceGenerationAdmissionTrigger, WorkspaceGenerationBuildMode,
};

/// Result of submitting cold generation readiness to the Runtime-owned dispatcher.
///
/// This is an in-process control receipt. Search/Query keeps its public typed
/// `query-not-ready` terminal while the accepted request runs independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceGenerationReadinessRequestState {
    Accepted,
    Coalesced,
    Ready,
}

struct PendingReadinessRequestGuard {
    key: WorkspaceGenerationAdmissionKey,
    requests: std::sync::Arc<
        std::sync::Mutex<
            std::collections::BTreeMap<WorkspaceGenerationAdmissionKey, std::path::PathBuf>,
        >,
    >,
}

impl Drop for PendingReadinessRequestGuard {
    fn drop(&mut self) {
        if let Ok(mut requests) = self.requests.lock() {
            requests.remove(&self.key);
        }
    }
}

impl WorkspaceGenerationAdmission {
    /// Submit one request-independent cold CompleteGeneration admission.
    ///
    /// Candidate discovery and all filesystem/provider work run behind the
    /// Runtime admission dispatcher. The Search/Query request path therefore
    /// never waits for cold construction. Concurrent requests for the same
    /// workspace coalesce before candidate discovery, and completion or
    /// failure releases the key for a later retry.
    pub fn request_runtime_generation_ready(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
    ) -> Result<WorkspaceGenerationReadinessRequestState, String> {
        self.request_runtime_generation_ready_with_terminal(
            workspace_identity,
            project_root,
            |_| async {},
        )
    }

    /// Submit the complete-generation barrier and keep its terminal consumer
    /// inside the same single-flight request.
    ///
    /// A durable Ready admission is not, by itself, proof that the Runtime's
    /// resident read generation is open. The terminal callback therefore runs
    /// for both a newly built generation and a replayed Ready generation. This
    /// lets the ASP Server install the exact commit into its resident authority
    /// without creating another publication authority or request-local retry.
    pub fn request_runtime_generation_ready_with_terminal<Terminal, TerminalFuture>(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        terminal: Terminal,
    ) -> Result<WorkspaceGenerationReadinessRequestState, String>
    where
        Terminal: FnOnce(Result<super::WorkspaceGenerationAdmissionReceipt, String>) -> TerminalFuture
            + Send
            + 'static,
        TerminalFuture: std::future::Future<Output = ()> + Send + 'static,
    {
        if workspace_identity.trim().is_empty() {
            return Err(
                "workspace generation readiness request identity must be non-empty".to_owned(),
            );
        }
        if !project_root.is_absolute() {
            return Err("workspace generation readiness request root must be absolute".to_owned());
        }

        let key = WorkspaceGenerationAdmissionKey {
            workspace_identity: workspace_identity.clone(),
        };
        let ready = if let Some(entry) = self.entries.get(&key) {
            if !entry.matches_project_root(&project_root) {
                return Err(format!(
                    "workspace generation admission root drift: workspaceIdentity={workspace_identity} requestedRoot={} admittedRoot={}",
                    project_root.display(),
                    entry.project_root.display(),
                ));
            }
            let observed = entry.observed();
            if observed.state == WorkspaceGenerationAdmissionState::Ready
                && observed.admission_mode.is_complete_generation()
                && observed.commit.is_some()
                && self
                    .ready_validator
                    .as_ref()
                    .is_none_or(|validator| validator(&workspace_identity, &project_root).is_ok())
            {
                observed.validate()?;
                Some(observed)
            } else {
                None
            }
        } else {
            None
        };

        let guard = {
            let mut requests = self.pending_readiness_requests.lock().map_err(|_| {
                "workspace generation readiness request registry poisoned".to_owned()
            })?;
            if let Some(admitted_root) = requests.get(&key) {
                if admitted_root != &project_root {
                    return Err(format!(
                        "workspace generation readiness request root drift: workspaceIdentity={workspace_identity} requestedRoot={} admittedRoot={}",
                        project_root.display(),
                        admitted_root.display(),
                    ));
                }
                return Ok(WorkspaceGenerationReadinessRequestState::Coalesced);
            }
            requests.insert(key.clone(), project_root.clone());
            PendingReadinessRequestGuard {
                key,
                requests: self.pending_readiness_requests.clone(),
            }
        };

        let admission = self.clone();
        let state = if ready.is_some() {
            WorkspaceGenerationReadinessRequestState::Ready
        } else {
            WorkspaceGenerationReadinessRequestState::Accepted
        };
        self.submit_background_mutation(async move {
            let _guard = guard;
            let result = match ready {
                Some(receipt) => Ok(receipt),
                None => {
                    admission
                        .ensure_runtime_generation_ready(workspace_identity, project_root)
                        .await
                }
            };
            terminal(result).await;
        })?;
        Ok(state)
    }

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
        self.ensure_runtime_generation_ready_for_provider(workspace_identity, project_root, None)
            .await
    }

    pub async fn ensure_runtime_generation_ready_for_provider(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        provider_target: Option<super::WorkspaceGenerationProviderTarget>,
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
                    && observed.admission_mode.is_complete_generation()
                    && observed.commit.is_some()
                    && self.ready_validator.as_ref().is_none_or(|validator| {
                        validator(&workspace_identity, &project_root).is_ok()
                    })
                {
                    observed.validate()?;
                    return Ok(observed);
                }
            }

            let resolved = crate::runtime_server_admission_catalog::
                RuntimeWorkspaceAdmissionCatalogEntry::resolve(
                    workspace_identity.clone(),
                    project_root.clone(),
                )?;
            if let Some(catalog) = self.catalog.as_ref() {
                let snapshot = catalog.snapshot();
                let mut admitted = snapshot.iter().filter(|entry| {
                    entry.workspace_identity == workspace_identity
                        && entry.project_root == project_root
                });
                let admission = admitted.next().ok_or_else(|| {
                    format!(
                        "Runtime generation demand lacks admitted ProjectId: workspaceId={workspace_identity} projectRoot={}",
                        project_root.display()
                    )
                })?;
                if admitted.next().is_some() || admission != &resolved {
                    return Err(format!(
                        "Runtime generation demand ProjectId binding is ambiguous or drifted: workspaceId={workspace_identity} projectRoot={}",
                        project_root.display()
                    ));
                }
            }
            let candidate = super::WorkspaceGenerationCandidateIdentity::for_runtime_admission(
                &resolved.project_id,
                &workspace_identity,
                provider_target.as_ref(),
            )?;
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
                    if build_mode == WorkspaceGenerationBuildMode::RestoreOrBuild {
                        WorkspaceGenerationAdmissionMode::CompleteGeneration
                    } else {
                        WorkspaceGenerationAdmissionMode::FullRecovery
                    },
                    // The target is an internal provider-work hint only. The
                    // published/read authority remains CompleteGeneration.
                    provider_target.clone(),
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
                || !terminal.admission_mode.is_complete_generation()
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
