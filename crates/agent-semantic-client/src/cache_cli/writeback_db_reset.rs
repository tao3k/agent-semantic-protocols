//! Client DB reset helpers for cache manifest universe rebuilds.

use agent_semantic_client_core::{CacheManifestStatus, ClientCacheManifest};
use agent_semantic_client_db::ClientDbEngineWriteSession;

pub(crate) fn sync_client_db_for_manifest_writeback(
    db_session: &mut ClientDbEngineWriteSession,
    manifest: &ClientCacheManifest,
    status: &CacheManifestStatus,
) -> Option<()> {
    db_session
        .sync_cache_generations_for_manifest_writeback(manifest, status)
        .ok()
}
