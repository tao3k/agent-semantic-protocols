//! Public compact graph renderer API.

use serde_json::Value;

/// Packet-local alias for the rendered search root.
pub const SEARCH_ROOT_ID: &str = "G";
/// Minimal grammar line emitted by compact graph output.
pub const TERSE_GRAPH_MICRO_LEGEND: &str =
    "legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next";

/// Options that bound prompt-facing topology projection rendering.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TopologyProjectionOptions {
    /// Presentation density; this never changes semantic packet identity.
    pub density: crate::SearchProjectionDensityV1,
    /// Maximum number of ranked aliases to render.
    pub seed_limit: Option<usize>,
}

/// Project a validated semantic search packet as topology line protocol.
pub fn render_search_topology_projection(
    packet: &Value,
    options: TopologyProjectionOptions,
) -> String {
    super::pipeline::render_search_topology_projection(packet, options)
}
