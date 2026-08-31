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
    /// Enqueue demand without making the request await admission/build work.
    /// The returned receipt is the server-owned queued snapshot; the dispatcher
    /// performs the actual admission and terminal transition asynchronously.
    pub async fn enqueue_query_demand_for_candidate(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        candidate: super::WorkspaceGenerationCandidateIdentity,
        mut target_paths: Vec<PathBuf>,
        provider_target: Option<super::WorkspaceGenerationProviderTarget>,
    ) -> Result<super::WorkspaceGenerationAdmissionReceipt, String> {
        if workspace_identity.trim().is_empty() {
            return Err("query-demand admission identity must be non-empty".to_owned());
        }
        if !project_root.is_absolute() {
            return Err("query-demand admission root must be absolute".to_owned());
        }
        candidate.validate()?;
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
        self.admit_with_mode(
            workspace_identity,
            project_root,
            candidate,
            super::WorkspaceGenerationBuildMode::RestoreOrBuild,
            super::WorkspaceGenerationAdmissionTrigger::QueryDemand,
            super::WorkspaceGenerationAdmissionMode::ColdTargeted,
            provider_target,
            Arc::new(target_paths),
        )
        .await
    }

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
        target_paths: Vec<PathBuf>,
        provider_target: Option<super::WorkspaceGenerationProviderTarget>,
    ) -> Result<bool, String> {
        self.admit_query_demand_with_provider(
            workspace_identity,
            project_root,
            target_paths,
            provider_target,
        )
        .await
        .map(|(submitted, _terminal)| submitted)
    }

    /// Admits the current workspace candidate and returns its exact terminal.
    ///
    /// Search and Query routes must consume this API rather than treating a
    /// previously resident generation as current.  The terminal binds the
    /// candidate and policy identities to the single server-owned commit.
    pub async fn admit_query_demand_with_provider(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        target_paths: Vec<PathBuf>,
        provider_target: Option<super::WorkspaceGenerationProviderTarget>,
    ) -> Result<(bool, super::WorkspaceGenerationAdmissionReceipt), String> {
        let candidate = discover_workspace_generation_candidate(&project_root).await?;
        let expected_candidate = candidate.clone();
        let submitted = self
            .submit_query_demand_for_candidate(
                workspace_identity.clone(),
                project_root.clone(),
                candidate,
                target_paths,
                provider_target,
            )
            .await?;
        let terminal = self
            .wait_terminal(&workspace_identity, &project_root)
            .await?;
        if !terminal.accepted || terminal.commit.is_none() {
            return Err(serde_json::json!({
                "schemaId": "agent.semantic-protocols.query-not-ready",
                "schemaVersion": "1",
                "reasonKind": "query-not-ready",
                "workspaceIdentity": workspace_identity,
                "accepted": terminal.accepted,
                "state": terminal.state,
                "expectedCandidateGeneration": expected_candidate.candidate_generation,
                "observedCandidateGeneration": terminal.candidate_generation,
                "expectedPolicyOverlayDigest": expected_candidate.policy_overlay_digest,
                "observedPolicyOverlayDigest": terminal.policy_overlay_digest,
                "commitDigest": serde_json::Value::Null,
            })
            .to_string());
        }
        if terminal.candidate_generation != expected_candidate.candidate_generation
            || terminal.policy_overlay_digest != expected_candidate.policy_overlay_digest
        {
            return Err(serde_json::json!({
                "schemaId": "agent.semantic-protocols.query-demand-generation-terminal-mismatch",
                "schemaVersion": "1",
                "reasonKind": "query-demand-generation-terminal-mismatch",
                "workspaceIdentity": workspace_identity,
                "accepted": terminal.accepted,
                "state": terminal.state,
                "expectedCandidateGeneration": expected_candidate.candidate_generation,
                "observedCandidateGeneration": terminal.candidate_generation,
                "expectedPolicyOverlayDigest": expected_candidate.policy_overlay_digest,
                "observedPolicyOverlayDigest": terminal.policy_overlay_digest,
                "commit": terminal.commit,
            })
            .to_string());
        }
        Ok((submitted, terminal))
    }

    /// Enqueues query demand against the candidate pinned by client
    /// initialization. This prevents a second workspace scan from silently
    /// rebinding an admitted client session to a newer source generation.
    pub async fn submit_query_demand_for_candidate(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        candidate: super::WorkspaceGenerationCandidateIdentity,
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
                || (matches!(
                    observed.state,
                    WorkspaceGenerationAdmissionState::Queued
                        | WorkspaceGenerationAdmissionState::Building
                ) && entry.building_covers(&target_paths))
            {
                return Ok(false);
            }
        }
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
