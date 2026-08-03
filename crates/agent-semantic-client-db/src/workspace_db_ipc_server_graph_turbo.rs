use std::path::{Path, PathBuf};

use crate::runtime_server::GraphTurboEvaluationBuilder;
use crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use crate::workspace_db_ipc::WorkspaceDbIpcResult;

pub(super) async fn evaluate(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    builder: Option<&GraphTurboEvaluationBuilder>,
    workspace_identity: &str,
    project_root: String,
    message: serde_json::Value,
) -> WorkspaceDbIpcResult {
    let identity = validate_continuation_identity(
        memory_registry,
        workspace_identity,
        Path::new(&project_root),
        &message,
    );
    match (identity, builder) {
        (Ok(()), Some(builder)) => match builder(
            workspace_identity.to_owned(),
            PathBuf::from(&project_root),
            message,
        )
        .await
        {
            Ok(receipt) => WorkspaceDbIpcResult::GraphTurboEvaluation {
                workspace_identity: workspace_identity.to_owned(),
                project_root,
                receipt,
            },
            Err(message) => WorkspaceDbIpcResult::Failed {
                code: "runtime-server-graph-turbo-evaluation-failed".to_owned(),
                message,
            },
        },
        (Ok(()), None) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-graph-turbo-evaluation-unavailable".to_owned(),
            message: "Runtime Server has no resident Graph Turbo evaluator".to_owned(),
        },
        (Err(message), _) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-graph-turbo-continuation-stale".to_owned(),
            message,
        },
    }
}

fn validate_continuation_identity(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    workspace_identity: &str,
    project_root: &Path,
    message: &serde_json::Value,
) -> Result<(), String> {
    let lease = memory_registry.lease(workspace_identity, project_root)?;
    let expected_snapshot = lease.generation().source_snapshot.root_digest.clone();
    let expected_generation = lease.runtime_generation_digest();
    let actual_snapshot = message
        .get("snapshotDigest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Graph Turbo message snapshotDigest must be non-empty text".to_owned())?;
    let actual_generation = message
        .get("workspaceGenerationRootDigest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            "Graph Turbo message workspaceGenerationRootDigest must be non-empty text".to_owned()
        })?;
    if actual_snapshot != expected_snapshot || actual_generation != expected_generation {
        return Err(format!(
            "Graph Turbo continuation identity is stale: expectedSnapshot={} actualSnapshot={} expectedGeneration={} actualGeneration={}",
            expected_snapshot, actual_snapshot, expected_generation, actual_generation,
        ));
    }
    Ok(())
}
