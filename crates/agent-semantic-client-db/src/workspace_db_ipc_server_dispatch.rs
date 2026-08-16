use std::path::Path;

use super::{
    WorkspaceDbIpcOperation, WorkspaceDbIpcResult, WorkspaceDbRegistry,
    admitted_or_bootstrap_workspace,
};

pub(super) async fn dispatch_workspace_db_session_operation(
    registry: &WorkspaceDbRegistry,
    workspace_identity: &str,
    operation: WorkspaceDbIpcOperation,
) -> WorkspaceDbIpcResult {
    let dispatched = match operation {
        WorkspaceDbIpcOperation::ProjectTreeSitterQuery { .. } => Err(
            "Tree-sitter queries are available only through the Runtime Server".to_owned(),
        ),
        WorkspaceDbIpcOperation::Health => return WorkspaceDbIpcResult::Healthy,
        WorkspaceDbIpcOperation::Shutdown => return WorkspaceDbIpcResult::ShutdownAccepted,
        WorkspaceDbIpcOperation::CacheControl { .. } => Err(
            "cache-control operations are only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::AgentSessionRegistry { .. } => Err(
            "agent-session registry operations are not admitted until the staged registry dispatcher is published"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::ReadSourceIndex { .. } => {
            Err("source-index reads are only accepted by the Runtime Server data plane".to_owned())
        }
        WorkspaceDbIpcOperation::ReadRuntimeSelector { .. }
        | WorkspaceDbIpcOperation::ProviderSearch { .. }
        | WorkspaceDbIpcOperation::ReadRuntimeOwner { .. }
        | WorkspaceDbIpcOperation::ReadRuntimeMerkleOwner { .. }
        | WorkspaceDbIpcOperation::ProjectProviderOwner { .. }
        | WorkspaceDbIpcOperation::ResolveProviderRuntime { .. }
        | WorkspaceDbIpcOperation::ReadRuntimeSearchGenerationAuthority { .. }
        | WorkspaceDbIpcOperation::ReadTreeSitterInventory { .. } => Err(
            "resident runtime selector reads are only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::PublishRuntimeSelectorOverlay { .. }
        | WorkspaceDbIpcOperation::RebindRuntimeSelectorOverlay { .. } => Err(
            "resident runtime selector writes are only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::RequireRuntimeGeneration { .. }
        | WorkspaceDbIpcOperation::AdmitRuntimeGeneration { .. }
        | WorkspaceDbIpcOperation::SubmitRuntimeGenerationMutation { .. }
        | WorkspaceDbIpcOperation::ReadRuntimeGenerationDurability { .. }
        | WorkspaceDbIpcOperation::ReadRuntimeGraphFacts { .. }
        | WorkspaceDbIpcOperation::EvaluateGraphTurbo { .. } => Err(
            "canonical generation admission is only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::RefreshCodexMultiAgentControlPlane { .. }
        | WorkspaceDbIpcOperation::ReadCodexMultiAgentControlPlane { .. } => Err(
            "Codex multi-agent control-plane operations are only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
        WorkspaceDbIpcOperation::WriteProviderIncrementalOwner { request } => {
            let session = registry
                .acquire(&request.scope.project_root, &request.scope)
                .await;
            match session {
                Ok(session) => session
                    .write_provider_incremental_owner(&request)
                    .await
                    .map(|receipt| WorkspaceDbIpcResult::ProviderIncrementalOwner { receipt }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::ReadProviderTreeSitterQuery {
            query,
            incremental_budget,
            continuation,
        } => {
            let session = registry
                .acquire(&query.scope.project_root, &query.scope)
                .await;
            match session {
                Ok(session) => session
                    .read_provider_treesitter_query(
                        &query,
                        incremental_budget,
                        continuation.as_ref(),
                    )
                    .await
                    .map(|read| WorkspaceDbIpcResult::ProviderTreeSitterQuery { read }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::ReadProviderOwnerSnapshot { scope, owner_path } => {
            let session = registry.acquire(&scope.project_root, &scope).await;
            match session {
                Ok(session) => session
                    .read_provider_owner_snapshot(&scope, &owner_path)
                    .await
                    .map(|snapshot| WorkspaceDbIpcResult::ProviderOwnerSnapshot { snapshot }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::ReadProviderOwnerWarm { scope, owner } => {
            let session = admitted_or_bootstrap_workspace(
                registry,
                workspace_identity,
                Path::new(&scope.project_root),
            )
            .await;
            match session {
                Ok(session) => session.read_provider_owner_warm(&scope, &owner).await.map(
                    |(probe, snapshot)| WorkspaceDbIpcResult::ProviderOwnerWarm { probe, snapshot },
                ),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::ReadResidentSelector { request } => {
            let session = admitted_or_bootstrap_workspace(
                registry,
                workspace_identity,
                Path::new(&request.project_root),
            )
            .await;
            match session {
                Ok(session) => session
                    .read_resident_selector(&request)
                    .await
                    .map(|read| WorkspaceDbIpcResult::ResidentSelector { read }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::WriteProviderTreeSitterOwnerResult { query, result } => {
            let session = registry
                .acquire(&query.scope.project_root, &query.scope)
                .await;
            match session {
                Ok(session) => session
                    .write_provider_treesitter_owner_result(&query, &result)
                    .await
                    .map(|receipt| WorkspaceDbIpcResult::ProviderTreeSitterOwner { receipt }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::ProbeProviderOwners { scope, owners } => {
            let session = registry.acquire(&scope.project_root, &scope).await;
            match session {
                Ok(session) => session
                    .probe_provider_owners(&scope, &owners)
                    .await
                    .map(|receipt| WorkspaceDbIpcResult::ProviderOwners { receipt }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::UpsertProviderInventory { request } => {
            let session = registry
                .acquire(&request.scope.project_root, &request.scope)
                .await;
            match session {
                Ok(session) => session
                    .upsert_provider_owner_inventory(&request)
                    .await
                    .map(|receipt| WorkspaceDbIpcResult::ProviderInventory { receipt }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::FinishWrites { scope, mode } => {
            let session = registry.acquire(&scope.project_root, &scope).await;
            match session {
                Ok(session) => session
                    .finish_writes(mode)
                    .await
                    .map(|receipt| WorkspaceDbIpcResult::WriteFinish { receipt }),
                Err(error) => Err(error),
            }
        }
        WorkspaceDbIpcOperation::PublishRuntimeOwner { .. }
        | WorkspaceDbIpcOperation::TombstoneRuntimeOwner { .. }
        | WorkspaceDbIpcOperation::RelocateRuntimeOwner { .. } => Err(
            "Runtime owner publication is only accepted by the Runtime Server data plane"
                .to_owned(),
        ),
    };
    dispatched.unwrap_or_else(|message| WorkspaceDbIpcResult::Failed {
        code: "workspace-owner-operation-failed".to_owned(),
        message,
    })
}
