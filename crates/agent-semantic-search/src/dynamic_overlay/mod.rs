mod core;

pub(crate) use core::{
    DynamicOverlayDocument, DynamicOverlayNamespace, DynamicOverlayQuery, DynamicOverlaySearchHit,
    default_dynamic_overlay_search_backend,
};
pub use core::{DynamicOverlayLane, QUERY_OVERLAY_ROUTE_SOURCE, SEARCH_OVERLAY_ROUTE_SOURCE};
#[cfg(test)]
pub(crate) use core::{DynamicOverlaySearchBackend, InMemoryDynamicOverlaySearch};
