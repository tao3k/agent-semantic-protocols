use crate::memory_search::{MemorySearchRequest, MemorySearchResolution};
use crate::memory_search_turso::TursoMemorySearchBackend;

/// Process-resident exact-selector service backed by the canonical workspace
/// Turso session. It owns no secondary item map and performs no query-time
/// activation, database open, schema bootstrap, source read, or provider spawn.
#[derive(Clone, Debug)]
pub struct MemorySearchResident {
    backend: TursoMemorySearchBackend,
}

impl MemorySearchResident {
    pub fn new(backend: TursoMemorySearchBackend) -> Self {
        Self { backend }
    }

    pub async fn resolve(
        &self,
        request: &MemorySearchRequest,
    ) -> Result<MemorySearchResolution, String> {
        self.backend.resolve(request).await
    }
}
