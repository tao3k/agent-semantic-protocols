//! Schema-backed search projection and rendering boundary.

mod error;
mod model;
mod packet;
mod renderer;
mod topology;

pub use error::SearchProjectionError;
pub use model::{
    RENDERED_SEARCH_PROJECTION_SCHEMA_ID, RenderedSearchProjectionV1,
    SEARCH_PROJECTION_REQUEST_SCHEMA_ID, SEARCH_PROJECTION_SCHEMA_VERSION,
    SearchProjectionDensityV1, SearchProjectionRequestV1,
};
pub use packet::{
    SEMANTIC_SEARCH_PACKET_SCHEMA_ID, SEMANTIC_SEARCH_PACKET_SCHEMA_VERSION, SemanticSearchPacketV1,
};
pub use renderer::{
    SearchProjectionRenderer, TopologySearchProjectionRenderer, render_search_topology_projection,
};
pub use topology::{SEARCH_ROOT_ID, TERSE_GRAPH_MICRO_LEGEND, TopologyProjectionOptions};
pub mod source;
pub use renderer::RankedFrontierSearchProjectionRenderer;
pub use source::{
    GraphTurboResultPacketV1, SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_ID,
    SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_VERSION, SearchProjectionSource,
};
