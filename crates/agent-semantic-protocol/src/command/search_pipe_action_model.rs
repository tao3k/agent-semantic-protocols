//! Shared action model for search-pipe planning and rendering.

#[derive(Clone, Debug)]
pub(super) struct PipeAction {
    pub(super) index: usize,
    pub(super) owner: String,
    pub(super) selector: String,
    pub(super) symbol: String,
    pub(super) source_alias: String,
}
