//! Schema-backed search projection and rendering boundary.

mod artifact_identity;
pub use artifact_identity::source_index_artifact_digest;
mod error;
mod incremental_search_generation;
pub use incremental_search_generation::{
    IncrementalSearchGenerationValidationError, validate_incremental_search_generation_v1,
};
mod model;
mod packet;
mod renderer;
mod resident_search_result;
mod storage_route;
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
pub use resident_search_result::{
    RESIDENT_SEARCH_RESULT_SCHEMA_ID, RESIDENT_SEARCH_RESULT_SCHEMA_VERSION,
    RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_ID, RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_VERSION,
    ResidentSearchHit, ResidentSearchProjectionTier, ResidentSearchReadyResult,
    ResidentSearchReadyState, ResidentSearchWorkCounters, RuntimeProviderSearchReceipt,
};
pub use storage_route::{
    ProviderGraphEvidence, SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_ID,
    SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_VERSION, SemanticMutationClass,
    SemanticSearchAlgorithmEvidence, SemanticSearchQueryRoute, SemanticSearchRouteDecision,
    SemanticSearchStorageClass, SemanticSearchStorageProfile, SemanticSharingScope,
};
pub use topology::{SEARCH_ROOT_ID, TERSE_GRAPH_MICRO_LEGEND, TopologyProjectionOptions};
pub mod source;
pub use renderer::RankedFrontierSearchProjectionRenderer;
pub use source::{
    GraphTurboEvaluationRequest, GraphTurboResultPacketV1, SEMANTIC_GRAPH_TURBO_REQUEST_SCHEMA_ID,
    SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_ID, SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_VERSION,
    SearchProjectionSource,
};
