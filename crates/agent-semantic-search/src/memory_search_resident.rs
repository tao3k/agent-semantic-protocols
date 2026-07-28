use crate::memory_search::{
    MemorySearchBackendV1, MemorySearchIndexV1, MemorySearchRequestV1, MemorySearchResolutionV1,
};
use std::sync::OnceLock;

/// Immutable, load-once Memory Search service for one activated generation.
///
/// Generation invalidation is structural: callers create a new resident when
/// activation selects a different generation. A resident never mutates or
/// silently reloads its generation while serving requests.
pub struct MemorySearchResidentV1<B> {
    backend: B,
    generation: OnceLock<Result<MemorySearchIndexV1, String>>,
}

impl<B> MemorySearchResidentV1<B>
where
    B: MemorySearchBackendV1,
{
    /// Create an unloaded resident for one immutable backend binding.
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            generation: OnceLock::new(),
        }
    }

    /// Resolve from the load-once generation without repeated backend access.
    pub fn resolve(
        &self,
        request: &MemorySearchRequestV1,
    ) -> Result<MemorySearchResolutionV1, String> {
        match self
            .generation
            .get_or_init(|| self.backend.load_generation())
        {
            Ok(generation) => Ok(generation.resolve(request)),
            Err(error) => Err(error.clone()),
        }
    }

    /// Whether the resident has already attempted generation initialization.
    pub fn is_initialized(&self) -> bool {
        self.generation.get().is_some()
    }
}
