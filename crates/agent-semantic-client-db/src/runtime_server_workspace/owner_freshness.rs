//! Read-only resident owner identity views used by freshness admission.

use std::path::Path;

use super::{RuntimeServerWorkspaceRegistry, WorkspaceOwnerSnapshot};

pub type WorkspaceOwnerProjectionBuildFuture = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<WorkspaceOwnerSnapshot, String>> + Send>,
>;

pub type WorkspaceOwnerProjectionBuilder = std::sync::Arc<
    dyn Fn(
            String,
            String,
            std::path::PathBuf,
            WorkspaceOwnerSnapshot,
        ) -> WorkspaceOwnerProjectionBuildFuture
        + Send
        + Sync,
>;

impl RuntimeServerWorkspaceRegistry {
    /// Returns the admitted owner and generation identity without opening external state.
    pub fn runtime_owner_snapshot(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        owner_path: &str,
    ) -> Option<(String, WorkspaceOwnerSnapshot)> {
        let lease = self.lease(workspace_identity, project_root).ok()?;
        lease.runtime_owner_snapshot(owner_path)
    }

    /// Returns the current resident generation identity.
    pub fn runtime_generation_digest(
        &self,
        workspace_identity: &str,
        project_root: &Path,
    ) -> Option<String> {
        self.lease(workspace_identity, project_root)
            .ok()
            .map(|lease| lease.runtime_generation_digest())
    }

    /// Reconciles one normalized workspace owner before any resident projection can serve.
    pub async fn ensure_runtime_owner_freshness(
        &self,
        request_id: &str,
        workspace_identity: &str,
        project_root: &Path,
        language_id: &str,
        owner_path: &str,
        projection_builder: Option<&WorkspaceOwnerProjectionBuilder>,
    ) -> Result<super::WorkspaceRuntimeOwnerFreshnessReceipt, String> {
        let resolved_root = self
            .admit_runtime_workspace_root(project_root, workspace_identity)
            .await?;
        let owner_admission = {
            let mut admissions = self.owner_admissions.lock().await;
            std::sync::Arc::clone(
                admissions
                    .entry((workspace_identity.to_owned(), owner_path.to_owned()))
                    .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(()))),
            )
        };
        let _owner_admission = owner_admission.lock().await;
        let current = self.runtime_owner_snapshot(workspace_identity, &resolved_root, owner_path);
        let content = match super::owner_content_identity::read(&resolved_root, owner_path).await? {
            Some(content) => content,
            None => {
                return self
                    .remove_missing_owner(
                        request_id,
                        workspace_identity,
                        &resolved_root,
                        owner_path,
                        current,
                    )
                    .await;
            }
        };
        let bytes = content.bytes;
        let content_digest = content.digest;
        if let Some((generation_digest, owner)) = current.as_ref()
            && owner.content_digest == content_digest
            && resident_owner_projection_is_complete(owner)
        {
            return Ok(freshness_receipt(
                workspace_identity,
                generation_digest.clone(),
                owner_path,
                Some(content_digest),
                false,
                false,
            ));
        }
        let candidate = WorkspaceOwnerSnapshot {
            owner_path: owner_path.to_owned(),
            content_digest: content_digest.clone(),
            bytes,
            selectors: Vec::new(),
        };
        let projection_builder = projection_builder.ok_or_else(|| {
            "Runtime Server has no provider-owned owner projection builder".to_owned()
        })?;
        let projected = projection_builder(
            workspace_identity.to_owned(),
            language_id.to_owned(),
            resolved_root.clone(),
            candidate.clone(),
        )
        .await?;
        if projected.owner_path != candidate.owner_path
            || projected.content_digest != candidate.content_digest
            || projected.bytes != candidate.bytes
        {
            return Err(format!(
                "provider owner projection changed canonical owner identity: ownerPath={owner_path}"
            ));
        }
        if projected.selectors.is_empty() {
            return Err(format!(
                "provider owner projection omitted declaration selectors: ownerPath={owner_path}"
            ));
        }
        self.publish_owner_overlay(
            request_id.to_owned(),
            workspace_identity.to_owned(),
            &resolved_root,
            projected,
        )
        .await?;
        let generation_digest = self
            .runtime_generation_digest(workspace_identity, &resolved_root)
            .ok_or_else(|| "runtime workspace generation is unavailable".to_owned())?;
        Ok(freshness_receipt(
            workspace_identity,
            generation_digest,
            owner_path,
            Some(content_digest),
            true,
            false,
        ))
    }

    async fn remove_missing_owner(
        &self,
        request_id: &str,
        workspace_identity: &str,
        project_root: &Path,
        owner_path: &str,
        current: Option<(String, WorkspaceOwnerSnapshot)>,
    ) -> Result<super::WorkspaceRuntimeOwnerFreshnessReceipt, String> {
        if current.is_some() {
            self.tombstone_owner_overlay(
                request_id.to_owned(),
                workspace_identity.to_owned(),
                project_root,
                owner_path.to_owned(),
            )
            .await?;
        }
        let generation_digest = self
            .runtime_generation_digest(workspace_identity, project_root)
            .ok_or_else(|| "runtime workspace generation is unavailable".to_owned())?;
        Ok(freshness_receipt(
            workspace_identity,
            generation_digest,
            owner_path,
            None,
            current.is_some(),
            true,
        ))
    }
}

fn resident_owner_projection_is_complete(owner: &WorkspaceOwnerSnapshot) -> bool {
    !owner.selectors.is_empty()
}

fn freshness_receipt(
    workspace_identity: &str,
    generation_digest: String,
    owner_path: &str,
    owner_content_digest: Option<String>,
    changed: bool,
    removed: bool,
) -> super::WorkspaceRuntimeOwnerFreshnessReceipt {
    super::WorkspaceRuntimeOwnerFreshnessReceipt {
        schema_id: "asp.runtime-owner-freshness-receipt.v1".to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: workspace_identity.to_owned(),
        generation_digest,
        owner_path: owner_path.to_owned(),
        owner_content_digest,
        changed,
        removed,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_workspace_owner_freshness.rs"]
mod tests;
