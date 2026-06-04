//! Public compact graph renderer API.

use serde_json::Value;

/// Packet-local alias for the rendered search root.
pub const SEARCH_ROOT_ID: &str = "G";
/// Minimal grammar line emitted by compact graph output.
pub const COMPACT_GRAPH_MICRO_LEGEND: &str =
    "legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next";

/// Options that bound prompt-facing compact graph rendering.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GraphRenderOptions {
    /// Maximum number of ranked aliases to render.
    pub seed_limit: Option<usize>,
}

/// Render a validated semantic search packet as compact graph line protocol.
pub fn render_search_graph_packet(packet: &Value, options: GraphRenderOptions) -> String {
    super::pipeline::render_search_graph_packet(packet, options)
}
