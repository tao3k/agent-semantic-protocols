//! Client DB reset helpers for cache manifest universe rebuilds.

use agent_semantic_client_core::{CacheManifestStatus, ClientCacheManifest};
use agent_semantic_client_db::ClientDb;

pub(crate) fn sync_client_db_for_manifest_writeback(
    db: &mut ClientDb,
    manifest: &ClientCacheManifest,
    status: &CacheManifestStatus,
) -> Option<()> {
    match status {
        CacheManifestStatus::Missing | CacheManifestStatus::Invalid => {
            db.clear_cache_generations().ok()?;
        }
        CacheManifestStatus::Present => {
            db.prune_cache_generations_to_manifest(manifest).ok()?;
        }
        CacheManifestStatus::Unavailable => return None,
    }
    Some(())
}
