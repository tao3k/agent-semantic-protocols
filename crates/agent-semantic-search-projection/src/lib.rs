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
mod query_playbook_materialization;
mod resident_search_result;
mod runtime_graph_request;
mod search_topology_settlement;
mod search_topology_settlement_support;
mod storage_route;

pub use error::SearchProjectionError;
pub use query_playbook_materialization::QueryPlaybookMaterializationError;
pub use query_playbook_materialization::QueryPlaybookMaterializationReceipt;
pub use query_playbook_materialization::QueryPlaybookMaterializationRequest;
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
pub use search_topology_settlement::WORKSPACE_SEARCH_PLAYBOOK_V1_EVIDENCE_ITEM_LIMIT;
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
pub mod source;
pub use source::ResidentGraphEvaluationRequestV1;
pub use source::ResidentGraphEvaluationResultV1;
pub use source::SEMANTIC_GRAPH_RESIDENT_EVALUATION_REQUEST_SCHEMA_ID;
pub use source::SEMANTIC_GRAPH_RESIDENT_EVALUATION_RESULT_SCHEMA_ID;
