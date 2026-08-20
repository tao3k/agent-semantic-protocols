use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::runtime_server_admission::{
    PendingWorkspaceMutation, WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID,
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionKey,
    WorkspaceGenerationAdmissionReceipt, WorkspaceGenerationAdmissionState,
    WorkspaceGenerationBuildMode,
};

pub(super) fn observed_mutation_build_mode(
    receipt: &WorkspaceGenerationAdmissionReceipt,
) -> WorkspaceGenerationBuildMode {
    if receipt.state == WorkspaceGenerationAdmissionState::Ready && receipt.commit.is_some() {
        WorkspaceGenerationBuildMode::RebuildAfterMutation
    } else {
        WorkspaceGenerationBuildMode::RestoreOrBuild
    }
}

pub const WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-generation-mutation-admission-receipt";
pub const WORKSPACE_GENERATION_MUTATION_SUBMISSION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-generation-mutation-submission-receipt";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationMutationAdmissionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub mutation_id: String,
    pub changed_path_count: usize,
    pub affected_workspace_count: usize,
    pub receipts: Vec<WorkspaceGenerationAdmissionReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationMutationSubmissionState {
    Queued,
    Coalesced,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationMutationSubmissionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub mutation_id: String,
    pub workspace_identity: String,
    pub changed_path_count: usize,
    pub state: WorkspaceGenerationMutationSubmissionState,
}

impl WorkspaceGenerationMutationSubmissionReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_GENERATION_MUTATION_SUBMISSION_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("workspace mutation submission receipt identity mismatch".to_owned());
        }
        if self.mutation_id.trim().is_empty() || self.workspace_identity.trim().is_empty() {
            return Err("workspace mutation submission receipt scope is incomplete".to_owned());
        }
        if self.changed_path_count == 0 {
            return Err("workspace mutation submission receipt has no changed paths".to_owned());
        }
        Ok(())
    }
}

impl WorkspaceGenerationMutationAdmissionReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("workspace mutation admission receipt identity mismatch".to_owned());
        }
        if self.mutation_id.trim().is_empty() {
            return Err("workspace mutation admission receipt identity is empty".to_owned());
        }
        if self.changed_path_count == 0 || self.receipts.is_empty() {
            return Err(
                "workspace mutation admission receipt requires changed paths and workspace receipts"
                    .to_owned(),
            );
        }
        if self.affected_workspace_count != self.receipts.len() {
            return Err(
                "workspace mutation admission receipt count does not match receipts".to_owned(),
            );
        }
        if self.receipts.len() == 1 {
            return self.receipts[0].validate();
        }
        let mut workspace_identities = std::collections::BTreeSet::new();
        for receipt in &self.receipts {
            receipt.validate()?;
            if !workspace_identities.insert(receipt.workspace_identity.as_str()) {
                return Err(
                    "workspace mutation admission receipt repeats a workspace identity".to_owned(),
                );
            }
        }
        Ok(())
    }
}

impl WorkspaceGenerationAdmission {
    pub async fn admit_observed_mutation(
        &self,
        mutation_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
        changed_paths: Vec<PathBuf>,
        candidate: super::WorkspaceGenerationCandidateIdentity,
    ) -> Result<WorkspaceGenerationMutationAdmissionReceipt, String> {
        let mutation_id = mutation_id.into();
        if mutation_id.trim().is_empty() {
            return Err("workspace mutation admission id must be non-empty".to_owned());
        }
        let workspace_identity = workspace_identity.into();
        if workspace_identity.trim().is_empty() {
            return Err("workspace mutation admission identity must be non-empty".to_owned());
        }
        if !project_root.is_absolute() {
            return Err("workspace mutation admission root must be absolute".to_owned());
        }
        if let Some(existing) = self.catalog.as_ref().and_then(|catalog| {
            catalog
                .snapshot()
                .iter()
                .find(|entry| {
                    entry.workspace_identity == workspace_identity
                        && entry.project_root != project_root
                })
                .cloned()
        }) {
            return Err(format!(
                "workspace mutation admission identity already owns a different resident root: workspaceIdentity={} catalogProjectRoot={} requestedProjectRoot={}",
                workspace_identity,
                existing.project_root.display(),
                project_root.display(),
            ));
        }
        let changed_paths = changed_paths
            .into_iter()
            .map(|path| {
                if path.components().any(|component| {
                    matches!(
                        component,
                        std::path::Component::CurDir | std::path::Component::ParentDir
                    )
                }) {
                    return Err(format!(
                        "workspace mutation admission path must be normalized: {}",
                        path.display()
                    ));
                }
                if path.is_absolute() {
                    Ok(path)
                } else {
                    Ok(project_root.join(path))
                }
            })
            .collect::<Result<std::collections::BTreeSet<_>, String>>()?;
        if changed_paths.is_empty() {
            return Err("workspace mutation admission requires changed paths".to_owned());
        }
        if self.catalog.is_none() {
            if let Some(outside) = changed_paths
                .iter()
                .find(|changed_path| !changed_path.starts_with(&project_root))
            {
                return Err(format!(
                    "changed path is outside the resident workspace catalog: {}",
                    outside.display(),
                ));
            }
            let changed_path_count = changed_paths.len();
            let admission = self
                .admit_mutation_candidate(
                    mutation_id.clone(),
                    workspace_identity,
                    project_root,
                    Arc::new(changed_paths),
                    candidate,
                )
                .await?;
            let receipt = WorkspaceGenerationMutationAdmissionReceipt {
                schema_id: WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                schema_version: "1".to_owned(),
                mutation_id,
                changed_path_count,
                affected_workspace_count: 1,
                receipts: vec![admission],
            };
            receipt.validate()?;
            return Ok(receipt);
        }
        let mut catalog_entries = self
            .catalog
            .as_ref()
            .expect("catalog-backed fanout after resident-only route")
            .snapshot()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        if !catalog_entries.iter().any(|entry| {
            entry.workspace_identity == workspace_identity && entry.project_root == project_root
        }) {
            catalog_entries.push(
                crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry {
                    workspace_identity: workspace_identity.clone(),
                    project_root: project_root.clone(),
                },
            );
        }
        let mut affected = std::collections::BTreeMap::<String, PathBuf>::new();
        for changed_path in &changed_paths {
            let matched = catalog_entries
                .iter()
                .filter(|entry| changed_path.starts_with(&entry.project_root))
                .max_by_key(|entry| entry.project_root.components().count())
                .ok_or_else(|| {
                    format!(
                        "changed path is outside the resident workspace catalog: {}",
                        changed_path.display(),
                    )
                })?;
            affected.insert(
                matched.workspace_identity.clone(),
                matched.project_root.clone(),
            );
        }
        let changed_path_count = changed_paths.len();
        let changed_paths = Arc::new(changed_paths);
        let affected = affected.into_iter().collect::<Vec<_>>();
        let mut receipts = Vec::with_capacity(affected.len());
        if affected.len() == 1 {
            let (affected_identity, affected_root) = affected
                .into_iter()
                .next()
                .expect("single affected workspace");
            let affected_changed_paths = Arc::new(
                changed_paths
                    .iter()
                    .filter(|path| path.starts_with(&affected_root))
                    .cloned()
                    .collect(),
            );
            let affected_candidate = if affected_identity == workspace_identity
                && affected_root == project_root
            {
                candidate.clone()
            } else {
                let key = WorkspaceGenerationAdmissionKey {
                    workspace_identity: affected_identity.clone(),
                    project_root: affected_root.clone(),
                };
                let entry = self.entries.get(&key).ok_or_else(|| {
                    format!(
                        "workspace mutation fanout requires resident candidate evidence: workspaceIdentity={} projectRoot={}",
                        affected_identity,
                        affected_root.display(),
                    )
                })?;
                let observed = entry.observed();
                super::WorkspaceGenerationCandidateIdentity {
                    candidate_generation: observed.candidate_generation,
                    policy_overlay_digest: observed.policy_overlay_digest,
                }
            };
            receipts.push(
                self.admit_mutation_candidate(
                    mutation_id.clone(),
                    affected_identity,
                    affected_root,
                    affected_changed_paths,
                    affected_candidate,
                )
                .await?,
            );
        } else {
            let mut tasks = tokio::task::JoinSet::new();
            for (affected_identity, affected_root) in affected {
                let affected_changed_paths = Arc::new(
                    changed_paths
                        .iter()
                        .filter(|path| path.starts_with(&affected_root))
                        .cloned()
                        .collect(),
                );
                let affected_candidate = if affected_identity == workspace_identity
                    && affected_root == project_root
                {
                    candidate.clone()
                } else {
                    let key = WorkspaceGenerationAdmissionKey {
                        workspace_identity: affected_identity.clone(),
                        project_root: affected_root.clone(),
                    };
                    let entry = self.entries.get(&key).ok_or_else(|| {
                        format!(
                            "workspace mutation fanout requires resident candidate evidence: workspaceIdentity={} projectRoot={}",
                            affected_identity,
                            affected_root.display(),
                        )
                    })?;
                    let observed = entry.observed();
                    super::WorkspaceGenerationCandidateIdentity {
                        candidate_generation: observed.candidate_generation,
                        policy_overlay_digest: observed.policy_overlay_digest,
                    }
                };
                let admission = self.clone();
                let mutation_id = mutation_id.clone();
                tasks.spawn(async move {
                    admission
                        .admit_mutation_candidate(
                            mutation_id,
                            affected_identity,
                            affected_root,
                            affected_changed_paths,
                            affected_candidate,
                        )
                        .await
                });
            }
            while let Some(joined) = tasks.join_next().await {
                receipts.push(
                    joined
                        .map_err(|error| format!("workspace mutation task failed: {error}"))??,
                );
            }
        }
        receipts.sort_by(|left, right| left.workspace_identity.cmp(&right.workspace_identity));
        let receipt = WorkspaceGenerationMutationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            mutation_id,
            changed_path_count,
            affected_workspace_count: receipts.len(),
            receipts,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    /// Admit one observed mutation and return only after every affected workspace reaches the
    /// terminal state for the exact attempt scheduled by this mutation.
    pub async fn admit_observed_mutation_terminal(
        &self,
        mutation_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
        changed_paths: Vec<PathBuf>,
        candidate: super::WorkspaceGenerationCandidateIdentity,
    ) -> Result<WorkspaceGenerationMutationAdmissionReceipt, String> {
        let workspace_identity = workspace_identity.into();
        let mut receipt = self
            .admit_observed_mutation(
                mutation_id,
                workspace_identity.clone(),
                project_root.clone(),
                changed_paths,
                candidate,
            )
            .await?;
        for admitted in &mut receipt.receipts {
            let affected_root = if admitted.workspace_identity == workspace_identity {
                project_root.clone()
            } else {
                self.catalog
                    .as_ref()
                    .and_then(|catalog| {
                        catalog
                            .snapshot()
                            .iter()
                            .find(|entry| entry.workspace_identity == admitted.workspace_identity)
                            .map(|entry| entry.project_root.clone())
                    })
                    .ok_or_else(|| {
                        format!(
                            "terminal mutation admission cannot resolve affected workspace root: workspaceIdentity={}",
                            admitted.workspace_identity
                        )
                    })?
            };
            *admitted = self
                .wait_terminal_attempt(
                    &admitted.workspace_identity,
                    &affected_root,
                    admitted.attempt,
                )
                .await?;
        }
        receipt.validate()?;
        Ok(receipt)
    }

    /// Rebuild the canonical server-owned cache generation under an exact-once mutation ID.
    pub async fn admit_cache_rebuild(
        &self,
        mutation_id: impl Into<String>,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
        candidate: super::WorkspaceGenerationCandidateIdentity,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        let mutation_id = mutation_id.into();
        if mutation_id.trim().is_empty() {
            return Err("workspace cache rebuild mutation id must be non-empty".to_owned());
        }
        let workspace_identity = workspace_identity.into();
        if workspace_identity.trim().is_empty() {
            return Err("workspace cache rebuild identity must be non-empty".to_owned());
        }
        if !project_root.is_absolute() {
            return Err("workspace cache rebuild root must be absolute".to_owned());
        }
        self.admit_mutation_candidate(
            mutation_id,
            workspace_identity,
            project_root,
            Arc::new(std::collections::BTreeSet::new()),
            candidate,
        )
        .await
    }

    pub(super) async fn admit_mutation_candidate(
        &self,
        mutation_id: String,
        workspace_identity: String,
        project_root: PathBuf,
        changed_paths: Arc<std::collections::BTreeSet<PathBuf>>,
        candidate: super::WorkspaceGenerationCandidateIdentity,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        candidate.validate()?;
        let key = WorkspaceGenerationAdmissionKey {
            workspace_identity: workspace_identity.clone(),
            project_root: project_root.clone(),
        };
        let receipt = mutation_submission_receipt(&workspace_identity, &candidate, 1)?;
        let active_mutation = Some(super::WorkspaceMutationIdentity {
            mutation_id: mutation_id.clone(),
            changed_paths: Arc::clone(&changed_paths),
            candidate: candidate.clone(),
        });
        let (entry, inserted) = self
            .entries
            .get_or_insert(key, receipt.clone(), active_mutation)
            .await?;
        let inserted_receipt = inserted.then_some(receipt);
        if let Some(receipt) = inserted_receipt {
            // The entry claim is the control-plane linearization point.  Candidate cache and
            // catalog publication must only be performed by that single owner; doing either
            // before `entries.entry` turns a cold workspace into an N-way global-cache write
            // storm and makes the submission receipt depend on scheduler contention.
            //
            // `candidate` has already passed validation, so recording the fresh, private
            // OnceCell cannot fail unless the cache implementation violates its own invariant.
            super::record_workspace_generation_candidate(project_root.clone(), candidate.clone())?;
            self.spawn_build(
                entry,
                workspace_identity,
                project_root,
                candidate,
                1,
                super::WorkspaceGenerationBuildMode::RestoreOrBuild,
                crate::runtime_server_admission::WorkspaceGenerationAdmissionTrigger::WorkspaceChange,
                crate::runtime_server_admission::WorkspaceGenerationAdmissionMode::IncrementalOverlay,
                None,
                Arc::default(),
            );
            return Ok(receipt);
        }

        if let Some(active) = entry.active_mutation.borrow().as_ref()
            && active.mutation_id == mutation_id
        {
            if active.changed_paths.as_ref() != changed_paths.as_ref()
                || active.candidate != candidate
            {
                return Err(format!(
                    "workspace mutation identity was reused with different candidate evidence: mutationId={mutation_id}"
                ));
            }
            let observed = entry.observed();
            let mut submission = mutation_submission_receipt(
                &workspace_identity,
                &active.candidate,
                observed.attempt,
            )?;
            submission.accepted = false;
            return Ok(submission);
        }

        let claim = entry
            .lane
            .claim_mutation(
                mutation_id.clone(),
                Arc::clone(&changed_paths),
                candidate.clone(),
            )
            .await?;
        if !claim.inserted {
            let mut submission = mutation_submission_receipt(
                &workspace_identity,
                claim.candidate.as_ref(),
                claim.attempt,
            )?;
            submission.accepted = false;
            return Ok(submission);
        }
        let claimed_attempt = claim.attempt;
        let claimed_submission =
            mutation_submission_receipt(&workspace_identity, &candidate, claimed_attempt)?;

        let transition = loop {
            match entry.transition.try_lock() {
                Ok(transition) => break transition,
                Err(_) => {
                    let changed = entry.mutation_changed.notified();
                    tokio::pin!(changed);
                    changed.as_mut().enable();
                    if let Some(active) = entry.active_mutation.borrow().as_ref()
                        && active.mutation_id == mutation_id
                    {
                        if active.changed_paths.as_ref() != changed_paths.as_ref()
                            || active.candidate != candidate
                        {
                            return Err(format!(
                                "workspace mutation identity was reused with different candidate evidence: mutationId={mutation_id}"
                            ));
                        }
                        return Ok(entry.observed());
                    }
                    changed.as_mut().await;
                }
            }
        };
        if let Some(active) = entry.active_mutation.borrow().as_ref()
            && active.mutation_id == mutation_id
        {
            if active.changed_paths.as_ref() != changed_paths.as_ref()
                || active.candidate != candidate
            {
                return Err(format!(
                    "workspace mutation identity was reused with different candidate evidence: mutationId={mutation_id}"
                ));
            }
            return Ok(entry.observed());
        }
        if !entry.lane.observed().building {
            entry.lane.begin_claimed(claimed_attempt).await?;
            let attempt = claimed_attempt;
            let accepted = claimed_submission.clone();
            let build_mode = observed_mutation_build_mode(&entry.observed());
            entry.receipt.send_replace(accepted.clone());
            entry
                .active_mutation
                .send_replace(Some(super::WorkspaceMutationIdentity {
                    mutation_id,
                    changed_paths,
                    candidate: candidate.clone(),
                }));
            entry.mutation_changed.notify_waiters();
            drop(transition);
            self.spawn_build(
                entry,
                workspace_identity,
                project_root,
                candidate,
                attempt,
                build_mode,
                crate::runtime_server_admission::WorkspaceGenerationAdmissionTrigger::WorkspaceChange,
                crate::runtime_server_admission::WorkspaceGenerationAdmissionMode::IncrementalOverlay,
                None,
                Arc::default(),
            );
            return Ok(accepted);
        }

        if entry.lane.observed().building {
            let attempt = claimed_attempt;
            entry
                .lane
                .enqueue_mutation(PendingWorkspaceMutation {
                    mutation_id,
                    changed_paths,
                    attempt,
                    candidate: candidate.clone(),
                })
                .await?;
            let queued = claimed_submission;
            return Ok(queued);
        }

        unreachable!("building mutation admission must queue or coalesce before this point")
    }
}

fn mutation_submission_receipt(
    workspace_identity: &str,
    candidate: &super::WorkspaceGenerationCandidateIdentity,
    attempt: u64,
) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
    let receipt = WorkspaceGenerationAdmissionReceipt {
        schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: workspace_identity.to_owned(),
        trigger:
            crate::runtime_server_admission::WorkspaceGenerationAdmissionTrigger::WorkspaceChange,
        admission_mode:
            crate::runtime_server_admission::WorkspaceGenerationAdmissionMode::IncrementalOverlay,
        build_owner: "runtime-server".to_owned(),
        cancellation_authority: "runtime-server".to_owned(),
        request_lifetime_independent: true,
        candidate_generation: candidate.candidate_generation.clone(),
        policy_overlay_digest: candidate.policy_overlay_digest.clone(),
        state: WorkspaceGenerationAdmissionState::Building,
        accepted: true,
        attempt,
        commit: None,
        failure_stage: None,
        error: None,
    };
    receipt.validate()?;
    Ok(receipt)
}
