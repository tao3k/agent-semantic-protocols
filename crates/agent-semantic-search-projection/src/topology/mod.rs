//! Compact graph rendering for schema-backed `search` packets.

mod actions;
mod aliases;
mod api;
mod header;
mod packet;
mod pipeline;
mod profiles;

pub use api::{
    SEARCH_ROOT_ID, TERSE_GRAPH_MICRO_LEGEND, TopologyProjectionOptions,
    render_search_topology_projection,
};
