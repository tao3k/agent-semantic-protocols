// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Schema-backed search projection and rendering boundary.

mod artifact_identity;
pub use artifact_identity::source_index_artifact_digest;
mod error;
mod incremental_search_generation;
pub use incremental_search_generation::IncrementalSearchGenerationValidationError;
pub use incremental_search_generation::validate_incremental_search_generation_v1;
mod model;
mod packet;
mod query_playbook_materialization;
mod renderer;
mod resident_search_result;
mod runtime_graph_request;
mod search_topology_settlement;
mod storage_route;
mod topology;

pub use error::SearchProjectionError;
pub use model::RENDERED_SEARCH_PROJECTION_SCHEMA_ID;
pub use model::RenderedSearchProjectionV1;
pub use model::SEARCH_PROJECTION_REQUEST_SCHEMA_ID;
pub use model::SEARCH_PROJECTION_SCHEMA_VERSION;
pub use model::SearchProjectionDensityV1;
pub use model::SearchProjectionRequestV1;
pub use packet::SEMANTIC_SEARCH_PACKET_SCHEMA_ID;
pub use packet::SEMANTIC_SEARCH_PACKET_SCHEMA_VERSION;
pub use packet::SemanticSearchPacketV1;
pub use query_playbook_materialization::QueryPlaybookGqlRelationship;
pub use query_playbook_materialization::QueryPlaybookMaterializationError;
pub use query_playbook_materialization::QueryPlaybookMaterializationReceipt;
pub use query_playbook_materialization::QueryPlaybookMaterializationRequest;
pub use renderer::SearchProjectionRenderer;
pub use renderer::TopologySearchProjectionRenderer;
pub use renderer::render_search_topology_projection;
pub use resident_search_result::RESIDENT_SEARCH_RESULT_SCHEMA_ID;
pub use resident_search_result::RESIDENT_SEARCH_RESULT_SCHEMA_VERSION;
pub use resident_search_result::RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_ID;
pub use resident_search_result::RUNTIME_PROVIDER_SEARCH_RECEIPT_SCHEMA_VERSION;
pub use resident_search_result::ResidentSearchHit;
pub use resident_search_result::ResidentSearchProjectionTier;
pub use resident_search_result::ResidentSearchReadyResult;
pub use resident_search_result::ResidentSearchReadyState;
pub use resident_search_result::ResidentSearchWorkCounters;
pub use resident_search_result::RuntimeProviderSearchReceipt;
pub use runtime_graph_request::adapt_graph_evaluate_payload;
pub use runtime_graph_request::bind_graph_generation_identity;
pub use runtime_graph_request::validate_graph_generation_receipt_identity;
pub use runtime_graph_request::validate_graph_source_root;
pub use search_topology_settlement::SearchTopologySettlement;
pub use search_topology_settlement::SearchTopologySettlementError;
pub use storage_route::ProviderGraphEvidence;
pub use storage_route::SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_ID;
pub use storage_route::SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_VERSION;
pub use storage_route::SemanticMutationClass;
pub use storage_route::SemanticSearchAlgorithmEvidence;
pub use storage_route::SemanticSearchQueryRoute;
pub use storage_route::SemanticSearchRouteDecision;
pub use storage_route::SemanticSearchStorageClass;
pub use storage_route::SemanticSearchStorageProfile;
pub use storage_route::SemanticSharingScope;
pub use topology::SEARCH_ROOT_ID;
pub use topology::TERSE_GRAPH_MICRO_LEGEND;
pub use topology::TopologyProjectionOptions;
pub mod source;
pub use renderer::RankedFrontierSearchProjectionRenderer;
pub use source::GraphTurboResultPacketV1;
pub use source::ResidentGraphEvaluationRequestV1;
pub use source::ResidentGraphEvaluationResultV1;
pub use source::SEMANTIC_GRAPH_RESIDENT_EVALUATION_REQUEST_SCHEMA_ID;
pub use source::SEMANTIC_GRAPH_RESIDENT_EVALUATION_RESULT_SCHEMA_ID;
pub use source::SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_ID;
pub use source::SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_VERSION;
pub use source::SearchProjectionSource;
