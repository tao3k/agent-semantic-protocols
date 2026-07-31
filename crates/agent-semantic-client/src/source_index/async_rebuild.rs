use std::path::PathBuf;
use std::time::Instant;

use agent_semantic_client_core::ProviderRegistrySnapshot;

use super::collect::{SourceIndexCollectionScope, collect_source_index_files_async};
use super::generation_build::{SourceIndexGenerationRefresh, SourceIndexRefreshContext};
use super::model::SourceIndexRefreshReport;

pub async fn rebuild_source_index_async(
    project_root: PathBuf,
) -> Result<SourceIndexRefreshReport, String> {
    let snapshot = ProviderRegistrySnapshot::load(&project_root)?;
    rebuild_source_index_with_registry_async(project_root, snapshot).await
}

pub async fn rebuild_source_index_with_registry_async(
    project_root: PathBuf,
    snapshot: ProviderRegistrySnapshot,
) -> Result<SourceIndexRefreshReport, String> {
    let trace_started = Instant::now();
    let context = SourceIndexRefreshContext::resolve(&project_root)?;
    trace("context-resolved", trace_started);
    trace("provider-registry-admitted", trace_started);
    let registry = snapshot.evidence(&project_root);
    let files = collect_source_index_files_async(
        &project_root,
        &snapshot,
        &SourceIndexCollectionScope::CompleteGeneration,
    )
    .await?;
    trace("scope-files-collected", trace_started);
    let prepared = context
        .prepare_generation_async(SourceIndexGenerationRefresh {
            index_root: &project_root,
            files: &files,
            registry: &registry,
        })
        .await?;
    prepared.commit_async().await
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
