mod core;

#[cfg(feature = "turso-overlay")]
mod turso_adapter;

pub(crate) use core::{
    DynamicOverlayDocument, DynamicOverlayNamespace, DynamicOverlayQuery,
    default_dynamic_overlay_search_backend,
};
