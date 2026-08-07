pub(super) async fn trace_owner_probe_boundary(
    session: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    scope: &agent_semantic_client_db::ProviderIncrementalScoped,
    probes: &[agent_semantic_client_db::ProviderOwnerBatchProbeRequest],
    boundary: &str,
) -> Result<(), String> {
    if std::env::var_os("ASP_TREESITTER_TRACE").is_none() {
        return Ok(());
    }
    let verification = session.probe_provider_owners(scope, probes).await?;
    eprintln!(
        "[query-treesitter-owner-boundary] boundary={boundary} probes={:?}",
        verification
            .results
            .iter()
            .map(|result| (&result.owner_path, result.probe.decision))
            .collect::<Vec<_>>()
    );
    Ok(())
}

pub(super) async fn finish_writes(
    session: &agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession,
    scope: &agent_semantic_client_db::ProviderIncrementalScoped,
    probes: &[agent_semantic_client_db::ProviderOwnerBatchProbeRequest],
) -> Result<(), String> {
    let finish_receipt = session
        .finish_writes(
            scope,
            agent_semantic_client_db::WorkspaceDbWriteFinishMode::OwnerDurabilityBoundary,
        )
        .await?;
    if std::env::var_os("ASP_TREESITTER_TRACE").is_none() {
        return Ok(());
    }
    let verification = session.probe_provider_owners(scope, probes).await?;
    eprintln!(
        "[query-treesitter-durability] transport=resident-workspace-owner workspaceIdentity={} providerWorkspaceDigest={} providerId={} cacheFlushCount={} checkpointMode={:?} checkpointBusy={:?} logFrames={:?} checkpointedFrames={:?} probes={:?}",
        scope.workspace_identity,
        scope.provider_workspace_identity_digest,
        scope.provider_id,
        finish_receipt.cache_flush_count,
        finish_receipt.checkpoint_mode,
        finish_receipt.checkpoint_busy,
        finish_receipt.log_frames,
        finish_receipt.checkpointed_frames,
        verification
            .results
            .iter()
            .map(|result| (&result.owner_path, result.probe.decision))
            .collect::<Vec<_>>()
    );
    Ok(())
}
