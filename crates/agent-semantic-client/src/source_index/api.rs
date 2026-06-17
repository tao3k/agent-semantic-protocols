//! Public refresh API for the Rust SQL source index.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{CacheGenerationId, ProjectContext, ProviderRegistrySnapshot};
use agent_semantic_client_db::ClientDb;

use super::collect::collect_source_index_files;
use super::import::source_index_import;
use super::model::SourceIndexRefreshReport;

/// Refresh the Rust SQL source index for a project without storing raw source.
pub fn refresh_source_index(project_root: &Path) -> Result<SourceIndexRefreshReport, String> {
    let project_context = ProjectContext::resolve(project_root)?;
    project_context.require_inside_workspace(project_root)?;
    let db_path = ClientDb::default_path(project_context.state_layout().client_cache_dir());
    let snapshot = ProviderRegistrySnapshot::load(project_root)?;
    let files = collect_source_index_files(project_root, &snapshot)?;
    let generation_id = source_index_generation_id();
    let import = source_index_import(project_root, generation_id.clone(), &files)?;
    let mut db = ClientDb::open_or_create(&db_path)?;
    let stats = db.replace_source_index(&import)?;
    Ok(SourceIndexRefreshReport {
        db_path,
        generation_id,
        file_count: files.len().min(u32::MAX as usize) as u32,
        owner_count: stats.owner_count,
        selector_count: stats.selector_count,
    })
}

fn source_index_generation_id() -> CacheGenerationId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    CacheGenerationId::from(format!("source-index-{nanos}"))
}
