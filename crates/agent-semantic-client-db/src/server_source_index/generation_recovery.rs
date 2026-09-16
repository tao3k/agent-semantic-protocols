// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::generation_build::SourceIndexGenerationRefresh;
use super::generation_commit::PreparedSourceIndexGeneration;

/// Execution identity supplied by the Runtime's admitted provider catalog.
/// This is an internal input, not a new serialized/public protocol.
#[derive(Clone)]
pub struct SourceIndexRecoveryExecution {
    pub runtime_bundle_digest: String,
    pub schema_bundle_digest: String,
    pub workspace_closure_digest: String,
}

impl SourceIndexRecoveryExecution {
    fn matches(
        &self,
        binding: &agent_semantic_artifacts::runtime_provider_execution_binding::RuntimeProviderExecutionBinding,
    ) -> bool {
        self.runtime_bundle_digest == binding.runtime_bundle_digest
            && self.schema_bundle_digest == binding.schema_bundle_digest
            && self.workspace_closure_digest == binding.workspace_closure_digest
    }
}

pub(super) async fn recover_unchanged_generation(
    db_path: &std::path::Path,
    request: &SourceIndexGenerationRefresh<'_>,
    workspace_identity: &str,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    source_blobs: &crate::ClientDbSourceIndexSourceBlobs,
) -> Result<Option<PreparedSourceIndexGeneration>, String> {
    let Some(execution) = request.recovery_execution else {
        return Ok(None);
    };
    let schema_id = super::config::SOURCE_INDEX_SCHEMA_ID.into();
    let schema_version = super::config::SOURCE_INDEX_SCHEMA_VERSION.into();
    eprintln!("[base-generation-recovery-stage] phase=identity-probe state=started");
    let Some(stats) = crate::engine::latest_turso_source_index_stats(
        db_path,
        request.index_root,
        &schema_id,
        &schema_version,
    )
    .await?
    else {
        return Ok(None);
    };
    if stats.source_snapshot != *source_snapshot {
        return Ok(None);
    }
    eprintln!("[base-generation-recovery-stage] phase=canonical-load state=started");
    let Some(materialization) = crate::engine::active_turso_workspace_generation_materialization(
        db_path,
        workspace_identity,
        request.index_root,
    )
    .await?
    else {
        return Ok(None);
    };
    if materialization.source_snapshot != *source_snapshot
        || materialization.project_resolutions != request.project_resolutions
        || !materialization
            .runtime_provider_execution_binding
            .as_ref()
            .is_some_and(|binding| execution.matches(binding))
    {
        return Ok(None);
    }
    eprintln!("[base-generation-recovery-stage] phase=source-index-load state=started");
    let active = crate::active_turso_source_index_generation(
        db_path,
        request.index_root,
        &schema_id,
        &schema_version,
    )
    .await?
    .ok_or_else(|| "recovery canonical generation has no active source-index facts".to_owned())?;
    if active.snapshot.source_snapshot != *source_snapshot {
        return Err("recovery canonical/source-index generation drift".to_owned());
    }
    let import = crate::source_index::recovered_source_index_import(
        &active.snapshot,
        request.index_root,
        schema_id,
        schema_version,
        source_blobs.clone(),
    )?;
    let refresh = crate::ClientDbSourceIndexRefreshRequest {
        file_count: active.snapshot.owner_count,
        import,
        source_snapshot: source_snapshot.clone(),
    };
    let workspace = workspace_identity.to_owned();
    eprintln!("[base-generation-recovery-stage] phase=validation state=started");
    // Large immutable record validation is not reactor work.
    let (materialization, refresh) = tokio::task::spawn_blocking(move || {
        let current_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes(
            refresh.import.source_blobs.iter(),
        );
        if current_snapshot != materialization.workspace_snapshot {
            return Err("recovery current bytes do not match durable snapshot".to_owned());
        }
        materialization
            .validate_refresh_request(&workspace, &refresh)
            .map_err(|error| {
                eprintln!(
                    "[base-generation-recovery-stage] phase=validation state=failed reason={error}"
                );
                error
            })?;
        Ok::<_, String>((materialization, refresh))
    })
    .await
    .map_err(|error| format!("recovery validation worker failed: {error}"))??;
    Ok(Some(PreparedSourceIndexGeneration::new(
        db_path.to_path_buf(),
        refresh,
        materialization,
        request.candidate.clone(),
        std::time::Instant::now(),
    )))
}

#[cfg(test)]
#[path = "../../tests/unit/source_index_generation_recovery.rs"]
mod tests;
