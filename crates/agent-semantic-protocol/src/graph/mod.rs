//! Compact graph rendering for schema-backed `search` packets.

mod actions;
mod aliases;
mod api;
mod header;
mod packet;
mod pipeline;
mod profiles;

pub use api::{
    COMPACT_GRAPH_MICRO_LEGEND, GraphRenderOptions, SEARCH_ROOT_ID, render_search_graph_packet,
};
