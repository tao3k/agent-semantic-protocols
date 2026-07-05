//! Search route planner for fast filename and path queries.
//!
//! The planner keeps filename/path lookup as a first-class hot path before
//! source-index lookup or provider execution. It does not own workspace
//! enumeration; callers pass an already-built `FileLocatorIndex`.

use crate::file_locator::{FileLocatorCandidate, FileLocatorIndex, FileLocatorQuery};

/// Search route selected by the planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPlannerRoute {
    /// Query was answered by the in-memory file locator.
    FileLocator,
    /// Query should continue through the source-index lookup.
    SourceIndex,
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
        route: SearchPlannerRoute::SourceIndex,
        file_candidates: Vec::new(),
    }
}
