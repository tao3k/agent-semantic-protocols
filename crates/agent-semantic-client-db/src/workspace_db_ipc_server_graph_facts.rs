use std::path::Path;

use crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use crate::workspace_db_ipc::{
    RuntimeGraphFactSource, RuntimeGraphFactsRead, WorkspaceDbIpcResult,
};

pub(super) fn read(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    workspace_identity: &str,
    project_root: &str,
    sources: Vec<RuntimeGraphFactSource>,
) -> WorkspaceDbIpcResult {
    let result = (|| {
        let lease = memory_registry.lease(workspace_identity, Path::new(project_root))?;
        let mut sources = sources;
        sources.sort();
        sources.dedup();
        let mut relations = Vec::new();
        for source in sources {
            relations.extend(
                lease
                    .relations_from(&source.kind, &source.id)
                    .into_iter()
                    .cloned(),
            );
        }
        relations.sort();
        relations.dedup();
        RuntimeGraphFactsRead::new(lease.runtime_generation_digest(), relations)
    })();
    match result {
        Ok(read) => WorkspaceDbIpcResult::RuntimeGraphFacts { read },
        Err(message) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-graph-facts-read-failed".to_owned(),
            message,
        },
    }
}
