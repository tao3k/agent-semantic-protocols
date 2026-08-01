use std::path::PathBuf;
use std::time::Instant;

use agent_semantic_client_core::ProviderRegistrySnapshot;

use super::collect::{SourceIndexCollectionScope, collect_source_index_scope_async};
use super::generation_build::{SourceIndexGenerationRefresh, SourceIndexRefreshContext};

pub async fn prepare_runtime_server_workspace_generation_async(
    project_root: PathBuf,
) -> Result<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuild, String> {
    let snapshot = ProviderRegistrySnapshot::load(&project_root)?;
    prepare_runtime_server_workspace_generation_with_registry_async(project_root, snapshot).await
}

pub async fn prepare_runtime_server_workspace_generation_with_registry_async(
    project_root: PathBuf,
    snapshot: ProviderRegistrySnapshot,
) -> Result<agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuild, String> {
    let trace_started = Instant::now();
    let context = SourceIndexRefreshContext::resolve(&project_root)?;
    trace("context-resolved", trace_started);
    trace("provider-registry-admitted", trace_started);
    let registry = snapshot.evidence(&project_root);
    let collection = collect_source_index_scope_async(
        &project_root,
        &snapshot,
        &SourceIndexCollectionScope::CompleteGeneration,
    )
    .await?;
    trace("scope-files-collected", trace_started);
    context
        .prepare_generation_async(SourceIndexGenerationRefresh {
            index_root: &project_root,
            files: &collection.files,
            project_resolutions: &collection.project_resolutions,
            registry: &registry,
            provider_registry: &snapshot,
        })
        .await
        .map(|prepared| prepared.into_runtime_server_build())
}

fn trace(stage: &str, started: Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-trace] stage={} elapsedMs={}",
            stage,
            started.elapsed().as_millis()
        );
    }
}
