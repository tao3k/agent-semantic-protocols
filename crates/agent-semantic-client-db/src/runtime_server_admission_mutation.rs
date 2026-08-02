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
    pub async fn admit_changed_paths(
        &self,
        mutation_id: impl Into<String>,
        origin_workspace_identity: impl Into<String>,
        project_root: PathBuf,
        changed_paths: Vec<PathBuf>,
    ) -> Result<WorkspaceGenerationMutationAdmissionReceipt, String> {
        let mutation_id = mutation_id.into();
        if mutation_id.trim().is_empty() {
            return Err("workspace mutation admission id must be non-empty".to_owned());
        }
        let origin_workspace_identity = origin_workspace_identity.into();
        if origin_workspace_identity.trim().is_empty() {
            return Err("workspace mutation admission identity must be non-empty".to_owned());
        }
        if !project_root.is_absolute() {
            return Err("workspace mutation admission root must be absolute".to_owned());
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
            Some(catalog) => catalog.entries().await,
            None => Vec::new(),
        };
        if let Some(existing) = catalog_entries.iter().find(|entry| {
            entry.workspace_identity == origin_workspace_identity
                && entry.project_root != project_root
        }) {
            return Err(format!(
                "workspace mutation admission identity already owns a different resident root: workspaceIdentity={} catalogProjectRoot={} requestedProjectRoot={}",
                origin_workspace_identity,
                existing.project_root.display(),
                project_root.display()
            ));
        }
        if !catalog_entries.iter().any(|entry| {
            entry.workspace_identity == origin_workspace_identity
                && entry.project_root == project_root
        }) {
            catalog_entries.push(
                crate::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry {
                    workspace_identity: origin_workspace_identity.clone(),
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
                        changed_path.display()
                    )
                })?;
            if let Some(existing_root) = affected.insert(
                matched.workspace_identity.clone(),
                matched.project_root.clone(),
            ) && existing_root != matched.project_root
            {
                return Err(format!(
                    "workspace identity maps to multiple project roots: workspaceIdentity={} left={} right={}",
                    matched.workspace_identity,
                    existing_root.display(),
                    matched.project_root.display()
                ));
            }
        }

        let mut receipts = if affected.len() == 1 {
            let (workspace_identity, workspace_root) = affected
                .pop_first()
                .expect("single affected workspace must exist");
            vec![
                self.admit_mutation(mutation_id.clone(), workspace_identity, workspace_root)
                    .await?,
            ]
        } else {
            let mut tasks = tokio::task::JoinSet::new();
            for (workspace_identity, workspace_root) in affected {
                let admission = self.clone();
                let mutation_id = mutation_id.clone();
                tasks.spawn(async move {
                    admission
                        .admit_mutation(mutation_id, workspace_identity, workspace_root)
                        .await
                });
            }
            let mut receipts = Vec::new();
            while let Some(joined) = tasks.join_next().await {
                receipts.push(joined.map_err(|error| {
                    format!("workspace mutation admission task failed: {error}")
                })??);
            }
            receipts
        };
        receipts.sort_by(|left, right| left.workspace_identity.cmp(&right.workspace_identity));
        let receipt = WorkspaceGenerationMutationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            mutation_id,
            changed_path_count: changed_paths.len(),
            affected_workspace_count: receipts.len(),
            receipts,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    async fn admit_mutation(
        &self,
        mutation_id: String,
        workspace_identity: String,
        project_root: PathBuf,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
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
            state: WorkspaceGenerationAdmissionState::Building,
            accepted: true,
            attempt: 1,
            error: None,
        };
        receipt.validate()?;
        let candidate = Arc::new(AdmissionEntry::new(
            receipt.clone(),
            Some(mutation_id.clone()),
        ));
        let (entry, inserted) = match self.entries.entry(key) {
            dashmap::mapref::entry::Entry::Occupied(existing) => {
                (Arc::clone(existing.get()), false)
            }
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                vacant.insert(Arc::clone(&candidate));
                (candidate, true)
            }
        };
        if inserted {
            self.spawn_build(
                entry,
                workspace_identity,
                project_root,
                1,
                super::WorkspaceGenerationBuildMode::RebuildAfterMutation,
            );
            return Ok(receipt);
        }

        let transition = entry.transition.lock().await;
        let mut mutations = entry.mutations.lock().await;
        if mutations.active_mutation_id.as_deref() == Some(mutation_id.as_str()) {
            return Ok(entry.observed());
        }
        if let Some(pending) = mutations
            .pending
            .iter()
            .find(|pending| pending.mutation_id == mutation_id)
        {
            let mut observed = entry.observed();
            observed.attempt = pending.attempt;
            return Ok(observed);
        }
        if entry.building.load(Ordering::Acquire) {
            let attempt = entry.attempt.fetch_add(1, Ordering::AcqRel) + 1;
            mutations.pending.push_back(PendingWorkspaceMutation {
                mutation_id,
                attempt,
            });
            let queued = WorkspaceGenerationAdmissionReceipt {
                schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                schema_version: "1".to_owned(),
                workspace_identity,
                state: WorkspaceGenerationAdmissionState::Building,
                accepted: true,
                attempt,
                error: None,
            };
            queued.validate()?;
            return Ok(queued);
        }

        entry.building.store(true, Ordering::Release);
        let attempt = entry.attempt.fetch_add(1, Ordering::AcqRel) + 1;
        mutations.active_mutation_id = Some(mutation_id);
        let accepted = WorkspaceGenerationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.clone(),
            state: WorkspaceGenerationAdmissionState::Building,
            accepted: true,
            attempt,
            error: None,
        };
        accepted.validate()?;
        entry.receipt.send_replace(accepted.clone());
        drop(mutations);
        drop(transition);
        self.spawn_build(
            entry,
            workspace_identity,
            project_root,
            attempt,
            super::WorkspaceGenerationBuildMode::RebuildAfterMutation,
        );
        Ok(accepted)
    }
}
