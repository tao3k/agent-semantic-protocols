//! Search route planner for fast filename and path queries.
//!
//! The planner keeps filename/path lookup as a first-class hot path before
//! source-index lookup or provider execution. It does not own workspace
//! enumeration; callers pass an already-built `FileLocatorIndex`.

use crate::file_locator::{FileLocatorCandidate, FileLocatorIndex, FileLocatorQuery};
use agent_semantic_search_projection::{
    SemanticMutationClass, SemanticSearchRouteDecision, SemanticSearchStorageProfile,
    SemanticSharingScope,
};

/// Search route selected by the planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPlannerRoute {
    /// Query was answered by the in-memory file locator.
    FileLocator,
    /// Query should continue through the source-index lookup.
    SourceIndex,
    /// Query should enter the lexical SearchFrame acquisition planner.
    LexicalSearchFrame,
}

/// Request for planning a search route.
#[derive(Clone, Copy, Debug)]
pub struct SearchPlannerRequest<'a> {
    /// User query text.
    pub query: &'a str,
    /// Maximum number of file locator candidates to return.
    pub limit: usize,
    /// Optional warm path index supplied by the caller.
    pub file_locator: Option<&'a FileLocatorIndex>,
}

/// Search route plan returned by `plan_search_route`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPlannerDecision {
    /// First route to execute for this query.
    pub route: SearchPlannerRoute,
    /// File locator candidates when the selected route is `FileLocator`.
    pub file_candidates: Vec<FileLocatorCandidate>,
}

/// Plan the first search route for a query.
#[must_use]
pub fn plan_search_route(request: SearchPlannerRequest<'_>) -> SearchPlannerDecision {
    if let Some(file_locator) = request.file_locator {
        let file_candidates = file_locator
            .locate(&FileLocatorQuery::new(request.query).with_limit(request.limit.max(1)));
        if !file_candidates.is_empty() {
            return SearchPlannerDecision {
                route: SearchPlannerRoute::FileLocator,
                file_candidates,
            };
        }
    }

    SearchPlannerDecision {
        route: SearchPlannerRoute::LexicalSearchFrame,
        file_candidates: Vec::new(),
    }
}

/// Inputs for the language-neutral semantic storage-route planner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticStorageRouteRequest {
    pub profile: SemanticSearchStorageProfile,
    pub has_exact_owner_path: bool,
    pub has_structural_selector: bool,
}

/// Select a query route from stability, sharing, Merkle locality, and graph cost evidence.
pub fn plan_semantic_storage_route(
    request: SemanticStorageRouteRequest,
) -> Result<SemanticSearchRouteDecision, String> {
    request.profile.validate()?;
    if request.has_exact_owner_path != request.has_structural_selector {
        return Err("exact mmap route requires both owner path and structural selector".to_owned());
    }
    if request.has_exact_owner_path {
        return SemanticSearchRouteDecision::exact_mmap(request.profile);
    }

    let profile = &request.profile;
    let static_shared = matches!(
        profile.mutation_class,
        SemanticMutationClass::Immutable | SemanticMutationClass::LowMutation
    ) && matches!(
        profile.sharing_scope,
        SemanticSharingScope::Global
            | SemanticSharingScope::ToolchainVersion
            | SemanticSharingScope::PackageVersion
    );
    if static_shared {
        return SemanticSearchRouteDecision::static_database(request.profile);
    }

    let graph_requires_resident_traversal = graph_requires_resident_memory(profile);
    let owner_local = profile.mutation_class == SemanticMutationClass::OwnerDynamic
        || profile.sharing_scope == SemanticSharingScope::OwnerContent
        || graph_requires_resident_traversal;
    if owner_local {
        return SemanticSearchRouteDecision::resident_memory(request.profile);
    }

    if profile.mutation_class == SemanticMutationClass::GenerationBound
        && profile.sharing_scope == SemanticSharingScope::WorkspaceGeneration
    {
        return SemanticSearchRouteDecision::shallow_database(request.profile);
    }

    Err("semantic search storage profile has no admitted route".to_owned())
}

fn graph_requires_resident_memory(profile: &SemanticSearchStorageProfile) -> bool {
    let evidence = &profile.algorithm_evidence;
    let dense_graph = evidence.graph_node_count > 0
        && evidence.graph_edge_count > evidence.graph_node_count.saturating_mul(8);
    let broad_merkle_delta = evidence.graph_node_count > 0
        && u64::from(evidence.merkle_delta_owner_count).saturating_mul(100)
            > evidence.graph_node_count.saturating_mul(5);
    evidence.tree_depth > 1
        || evidence.traversal_radius > 1
        || evidence.dependency_fan_out > 64
        || dense_graph
        || broad_merkle_delta
}
