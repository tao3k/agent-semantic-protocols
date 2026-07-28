use std::sync::OnceLock;

#[derive(Debug)]
pub struct LoadOnceGenerationV1<G> {
    generation: OnceLock<Result<G, String>>,
}

impl<G> Default for LoadOnceGenerationV1<G> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G> LoadOnceGenerationV1<G> {
    pub const fn new() -> Self {
        Self {
            generation: OnceLock::new(),
        }
    }

    pub fn get_or_try_init(&self, load: impl FnOnce() -> Result<G, String>) -> Result<&G, String> {
        self.generation
            .get_or_init(load)
            .as_ref()
            .map_err(Clone::clone)
    }

    pub fn is_initialized(&self) -> bool {
        self.generation.get().is_some()
    }
}
