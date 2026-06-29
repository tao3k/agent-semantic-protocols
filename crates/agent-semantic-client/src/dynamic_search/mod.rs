//! Rust-side dynamic search services for high-churn source evidence.

mod lexical_overlay;
mod overlay;
mod owner_item_parts;
mod owner_items;

pub use lexical_overlay::{
    LexicalOverlayCandidateHit, LexicalOverlayDocument, LexicalOverlaySearchHit,
    LexicalOverlaySearchRequest, search_lexical_overlay, search_lexical_overlay_candidates,
};
pub use owner_items::render_dynamic_owner_items_frontier;
pub use owner_items::{
    DynamicOwnerItem, DynamicOwnerItemsRequest, DynamicOwnerPath, DynamicOwnerQuery,
    DynamicSearchLanguage, DynamicSearchRoots, render_dynamic_owner_items_code,
};
