use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};

use serde::{Deserialize, Serialize};

use crate::runtime_server_admission::{
    AdmissionEntry, PendingWorkspaceMutation, WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID,
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionKey,
    WorkspaceGenerationAdmissionReceipt, WorkspaceGenerationAdmissionState,
};

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
        let mut catalog_entries = match &self.catalog {
            Some(catalog) => catalog.snapshot().iter().cloned().collect::<Vec<_>>(),
            None => Vec::new(),
        };
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
        let mut tasks = tokio::task::JoinSet::new();
        for (affected_identity, affected_root) in affected {
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
            let changed_paths = Arc::clone(&changed_paths);
            tasks.spawn(async move {
                admission
                    .admit_mutation_candidate(
                        mutation_id,
                        affected_identity,
                        affected_root,
                        changed_paths,
                        affected_candidate,
                    )
                    .await
            });
        }
        let mut receipts = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            receipts.push(
                joined.map_err(|error| format!("workspace mutation task failed: {error}"))??,
            );
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
        if let Some(catalog) = &self.catalog {
            catalog
                .record(
                    crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry {
                        workspace_identity: workspace_identity.clone(),
                        project_root: project_root.clone(),
                    },
                )
                .await?;
        }

        let receipt = WorkspaceGenerationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.clone(),
            candidate_generation: candidate.candidate_generation.clone(),
            policy_overlay_digest: candidate.policy_overlay_digest.clone(),
            state: WorkspaceGenerationAdmissionState::Building,
            accepted: true,
            attempt: 1,
            commit: None,
            error: None,
        };
        receipt.validate()?;
        let candidate_entry = Arc::new(AdmissionEntry::new(
            receipt.clone(),
            Some(super::WorkspaceMutationIdentity {
                mutation_id: mutation_id.clone(),
                changed_paths: Arc::clone(&changed_paths),
                candidate: candidate.clone(),
            }),
        ));
        let (entry, inserted) = match self.entries.entry(key) {
            dashmap::mapref::entry::Entry::Occupied(existing) => {
                (Arc::clone(existing.get()), false)
            }
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                vacant.insert(Arc::clone(&candidate_entry));
                (candidate_entry, true)
            }
        };
        if inserted {
            self.spawn_build(
                entry,
                workspace_identity,
                project_root,
                candidate,
                1,
                super::WorkspaceGenerationBuildMode::RebuildAfterMutation,
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
            return Ok(entry.observed());
        }

        let transition = loop {
            match entry.transition.try_lock() {
                Ok(transition) => break transition,
                Err(_) => {
                    tokio::task::yield_now().await;
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
        if !entry.building.load(Ordering::Acquire) {
            entry.building.store(true, Ordering::Release);
            let attempt = entry.attempt.fetch_add(1, Ordering::AcqRel) + 1;
            let accepted = WorkspaceGenerationAdmissionReceipt {
                schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                schema_version: "1".to_owned(),
                workspace_identity: workspace_identity.clone(),
                candidate_generation: candidate.candidate_generation.clone(),
                policy_overlay_digest: candidate.policy_overlay_digest.clone(),
                state: WorkspaceGenerationAdmissionState::Building,
                accepted: true,
                attempt,
                commit: None,
                error: None,
            };
            accepted.validate()?;
            entry.receipt.send_replace(accepted.clone());
            entry
                .active_mutation
                .send_replace(Some(super::WorkspaceMutationIdentity {
                    mutation_id,
                    changed_paths,
                    candidate: candidate.clone(),
                }));
            drop(transition);
            self.spawn_build(
                entry,
                workspace_identity,
                project_root,
                candidate,
                attempt,
                super::WorkspaceGenerationBuildMode::RebuildAfterMutation,
            );
            return Ok(accepted);
        }

        let mut mutations = entry.mutations.lock().await;
        if let Some(pending) = mutations
            .pending
            .iter()
            .find(|pending| pending.mutation_id == mutation_id)
        {
            if pending.changed_paths.as_ref() != changed_paths.as_ref()
                || pending.candidate != candidate
            {
                return Err(format!(
                    "workspace mutation identity was reused with different candidate evidence: mutationId={mutation_id}"
                ));
            }
            let mut observed = entry.observed();
            observed.attempt = pending.attempt;
            return Ok(observed);
        }
        if entry.building.load(Ordering::Acquire) {
            let attempt = entry.attempt.fetch_add(1, Ordering::AcqRel) + 1;
            mutations.pending.push_back(PendingWorkspaceMutation {
                mutation_id,
                changed_paths,
                attempt,
                candidate: candidate.clone(),
            });
            let queued = WorkspaceGenerationAdmissionReceipt {
                schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                schema_version: "1".to_owned(),
                workspace_identity,
                candidate_generation: candidate.candidate_generation.clone(),
                policy_overlay_digest: candidate.policy_overlay_digest.clone(),
                state: WorkspaceGenerationAdmissionState::Building,
                accepted: true,
                attempt,
                commit: None,
                error: None,
            };
            queued.validate()?;
            return Ok(queued);
        }

        unreachable!("building mutation admission must queue or coalesce before this point")
    }
}
