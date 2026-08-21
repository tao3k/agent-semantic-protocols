//! Runtime-owned cold admission triggered by a query miss.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use super::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionKey,
    WorkspaceGenerationAdmissionMode, WorkspaceGenerationAdmissionState,
    WorkspaceGenerationAdmissionTrigger, WorkspaceGenerationBuildMode,
    discover_workspace_generation_candidate,
};

impl WorkspaceGenerationAdmission {
    /// Enqueues one server-owned cold admission without tying it to the query lifetime.
    pub async fn submit_query_demand(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        target_paths: Vec<PathBuf>,
    ) -> Result<bool, String> {
        self.submit_query_demand_with_provider(workspace_identity, project_root, target_paths, None)
            .await
    }

    pub async fn submit_query_demand_with_provider(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        mut target_paths: Vec<PathBuf>,
        provider_target: Option<super::WorkspaceGenerationProviderTarget>,
    ) -> Result<bool, String> {
        if workspace_identity.trim().is_empty() {
            return Err("query-demand admission identity must be non-empty".to_owned());
        }
        if !project_root.is_absolute() {
            return Err("query-demand admission root must be absolute".to_owned());
        }
        // An empty path set used to mean a complete workspace generation. A
        // provider search has no single owner path, but it is still a narrow
        // request. Keep a stable, in-workspace coverage key so a Rust-only
        // generation is neither promoted to full coverage nor reused for a
        // later language.
        if target_paths.is_empty()
            && let Some(provider_target) = provider_target.as_ref()
        {
            target_paths.push(
                project_root
                    .join(".asp-runtime-query-demand")
                    .join("provider")
                    .join(&provider_target.language_id),
            );
        }
        let target_paths = target_paths
            .into_iter()
            .map(|path| {
                if path.is_absolute() && path.starts_with(&project_root) {
                    Ok(path)
                } else {
                    Err(
                        "query-demand targets must be absolute paths inside the workspace"
                            .to_owned(),
                    )
                }
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let target_paths = Arc::new(target_paths);
        let admission_key = WorkspaceGenerationAdmissionKey {
            workspace_identity: workspace_identity.clone(),
            project_root: project_root.clone(),
        };
        if let Some(entry) = self.entries.get(&admission_key) {
            let observed = entry.observed();
            if (observed.state == WorkspaceGenerationAdmissionState::Ready
                && entry.ready_covers(&target_paths))
                || (observed.state == WorkspaceGenerationAdmissionState::Building
                    && entry.building_covers(&target_paths))
            {
                return Ok(false);
            }
        }
        // Discover the candidate before returning so `admit_with_mode` can
        // synchronously publish the Building entry and detach only the build
        // itself. A waiter beginning immediately after this call therefore
        // observes a real admission receipt rather than an unknown key.
        let candidate = discover_workspace_generation_candidate(&project_root).await?;
        let receipt = self
            .admit_with_mode(
                workspace_identity,
                project_root,
                candidate,
                WorkspaceGenerationBuildMode::RestoreOrBuild,
                WorkspaceGenerationAdmissionTrigger::QueryDemand,
                WorkspaceGenerationAdmissionMode::ColdTargeted,
                provider_target,
                target_paths,
            )
            .await?;
        Ok(receipt.accepted)
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_server_admission_query_demand.rs"]
mod tests;
