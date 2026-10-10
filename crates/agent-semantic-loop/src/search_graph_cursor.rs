// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed, language-neutral cursor over already materialized SearchLoop graph nodes.

use std::collections::BTreeSet;
use std::fmt;

use agent_semantic_context_product::ProtocolId;
use serde::Deserialize;
use serde::Serialize;

use crate::search_runtime::SearchLoopArtifactBindingV1;

pub const SEARCH_GRAPH_CURSOR_SCHEMA_ID: &str = "agent.semantic-protocols.search-graph-cursor";
pub const SEARCH_GRAPH_CURSOR_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphRouteMeasure {
    semantic_graph_hops: u64,
    executed_graph_hops: u64,
    tool_rounds: u64,
    search_tokens: u64,
    uncached_model_tokens: u64,
}

impl GraphRouteMeasure {
    pub fn zero() -> Self {
        Self {
            semantic_graph_hops: 0,
            executed_graph_hops: 0,
            tool_rounds: 0,
            search_tokens: 0,
            uncached_model_tokens: 0,
        }
    }

    pub fn semantic_graph_hops(&self) -> u64 {
        self.semantic_graph_hops
    }

    pub fn executed_graph_hops(&self) -> u64 {
        self.executed_graph_hops
    }

    pub fn tool_rounds(&self) -> u64 {
        self.tool_rounds
    }

    pub fn search_tokens(&self) -> u64 {
        self.search_tokens
    }

    pub fn uncached_model_tokens(&self) -> u64 {
        self.uncached_model_tokens
    }
}

pub struct GraphRouteBudgetRequest {
    pub max_evidence_tokens: u64,
    pub max_raw_tokens: u64,
    pub max_uncached_tokens: u64,
    pub max_rounds: u64,
    pub max_graph_hops: u64,
    pub max_transitions: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphRouteBudget {
    max_evidence_tokens: u64,
    max_raw_tokens: u64,
    max_uncached_tokens: u64,
    max_rounds: u64,
    max_graph_hops: u64,
    max_transitions: u64,
}

impl GraphRouteBudget {
    pub fn new(request: GraphRouteBudgetRequest) -> Self {
        Self {
            max_evidence_tokens: request.max_evidence_tokens,
            max_raw_tokens: request.max_raw_tokens,
            max_uncached_tokens: request.max_uncached_tokens,
            max_rounds: request.max_rounds,
            max_graph_hops: request.max_graph_hops,
            max_transitions: request.max_transitions,
        }
    }

    pub fn max_evidence_tokens(&self) -> u64 {
        self.max_evidence_tokens
    }

    pub fn max_raw_tokens(&self) -> u64 {
        self.max_raw_tokens
    }

    pub fn max_uncached_tokens(&self) -> u64 {
        self.max_uncached_tokens
    }

    pub fn max_rounds(&self) -> u64 {
        self.max_rounds
    }

    pub fn max_graph_hops(&self) -> u64 {
        self.max_graph_hops
    }

    pub fn max_transitions(&self) -> u64 {
        self.max_transitions
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UncheckedInteractiveSearchGraphState {
    state_digest: String,
    graph_generation: u64,
    workspace_generation: String,
    current_node: String,
    remaining_budget: u64,
    obligations: Vec<String>,
    frontier: Vec<String>,
    visited: Vec<String>,
    route_measure: GraphRouteMeasure,
    route_budget: GraphRouteBudget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractiveSearchGraphState(UncheckedInteractiveSearchGraphState);

pub struct InteractiveSearchGraphStateRequest {
    pub state_digest: String,
    pub graph_generation: u64,
    pub workspace_generation: String,
    pub current_node: String,
    pub remaining_budget: u64,
    pub obligations: Vec<String>,
    pub frontier: Vec<String>,
    pub visited: Vec<String>,
    pub route_measure: GraphRouteMeasure,
    pub route_budget: GraphRouteBudget,
}

impl InteractiveSearchGraphState {
    pub fn new(
        request: InteractiveSearchGraphStateRequest,
    ) -> Result<Self, SearchGraphCursorError> {
        Self::validate(UncheckedInteractiveSearchGraphState {
            state_digest: request.state_digest,
            graph_generation: request.graph_generation,
            workspace_generation: request.workspace_generation,
            current_node: request.current_node,
            remaining_budget: request.remaining_budget,
            obligations: request.obligations,
            frontier: request.frontier,
            visited: request.visited,
            route_measure: request.route_measure,
            route_budget: request.route_budget,
        })
    }

    pub fn validate(
        unchecked: UncheckedInteractiveSearchGraphState,
    ) -> Result<Self, SearchGraphCursorError> {
        for value in [
            unchecked.state_digest.as_str(),
            unchecked.workspace_generation.as_str(),
            unchecked.current_node.as_str(),
        ] {
            require_non_empty(value)?;
        }
        validate_unique_non_empty("obligations", &unchecked.obligations)?;
        validate_unique_non_empty("frontier", &unchecked.frontier)?;
        validate_unique_non_empty("visited", &unchecked.visited)?;
        if unchecked.route_measure.executed_graph_hops > unchecked.route_measure.semantic_graph_hops
            || unchecked.route_measure.semantic_graph_hops > unchecked.route_budget.max_graph_hops
            || unchecked.route_measure.tool_rounds > unchecked.route_budget.max_rounds
            || unchecked.route_measure.search_tokens > unchecked.route_budget.max_evidence_tokens
            || unchecked.route_measure.uncached_model_tokens
                > unchecked.route_budget.max_uncached_tokens
            || unchecked.remaining_budget > unchecked.route_budget.max_transitions
        {
            return Err(SearchGraphCursorError::RouteBudgetExceeded);
        }
        Ok(Self(unchecked))
    }

    pub fn state_digest(&self) -> &str {
        &self.0.state_digest
    }

    pub fn graph_generation(&self) -> u64 {
        self.0.graph_generation
    }

    pub fn workspace_generation(&self) -> &str {
        &self.0.workspace_generation
    }

    pub fn current_node(&self) -> &str {
        &self.0.current_node
    }

    pub fn remaining_budget(&self) -> u64 {
        self.0.remaining_budget
    }

    pub fn obligations(&self) -> &[String] {
        &self.0.obligations
    }

    pub fn frontier(&self) -> &[String] {
        &self.0.frontier
    }

    pub fn visited(&self) -> &[String] {
        &self.0.visited
    }

    pub fn route_measure(&self) -> &GraphRouteMeasure {
        &self.0.route_measure
    }

    pub fn route_budget(&self) -> &GraphRouteBudget {
        &self.0.route_budget
    }

    pub fn into_unchecked(self) -> UncheckedInteractiveSearchGraphState {
        self.0
    }
}

pub struct SearchGraphCursorBootstrapRequest {
    pub state: InteractiveSearchGraphState,
    pub materialized_node_ids: Vec<String>,
}

pub struct SearchGraphCursorJumpRequest<'a> {
    pub target_node_id: &'a str,
    pub next_state_digest: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchGraphCursor {
    state: InteractiveSearchGraphState,
    materialized_node_ids: BTreeSet<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UncheckedSearchGraphCursorArtifact {
    schema_id: String,
    schema_version: String,
    loop_id: ProtocolId,
    graph_artifact: SearchLoopArtifactBindingV1,
    state: UncheckedInteractiveSearchGraphState,
    materialized_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchGraphCursorArtifact {
    loop_id: ProtocolId,
    graph_artifact: SearchLoopArtifactBindingV1,
    cursor: SearchGraphCursor,
}

pub struct SearchGraphCursorArtifactRequest {
    pub loop_id: ProtocolId,
    pub graph_artifact: SearchLoopArtifactBindingV1,
    pub cursor: SearchGraphCursor,
}

impl SearchGraphCursorArtifact {
    pub fn new(request: SearchGraphCursorArtifactRequest) -> Self {
        Self {
            loop_id: request.loop_id,
            graph_artifact: request.graph_artifact,
            cursor: request.cursor,
        }
    }

    pub fn validate(
        unchecked: UncheckedSearchGraphCursorArtifact,
    ) -> Result<Self, SearchGraphCursorError> {
        if unchecked.schema_id != SEARCH_GRAPH_CURSOR_SCHEMA_ID
            || unchecked.schema_version != SEARCH_GRAPH_CURSOR_SCHEMA_VERSION
        {
            return Err(SearchGraphCursorError::SchemaIdentity);
        }
        let state = InteractiveSearchGraphState::validate(unchecked.state)?;
        let cursor = SearchGraphCursor::bootstrap(SearchGraphCursorBootstrapRequest {
            state,
            materialized_node_ids: unchecked.materialized_node_ids,
        })?;
        Ok(Self {
            loop_id: unchecked.loop_id,
            graph_artifact: unchecked.graph_artifact,
            cursor,
        })
    }

    pub fn loop_id(&self) -> &ProtocolId {
        &self.loop_id
    }

    pub fn graph_artifact(&self) -> &SearchLoopArtifactBindingV1 {
        &self.graph_artifact
    }

    pub fn cursor(&self) -> &SearchGraphCursor {
        &self.cursor
    }

    pub fn into_unchecked(self) -> UncheckedSearchGraphCursorArtifact {
        let SearchGraphCursor {
            state,
            materialized_node_ids,
        } = self.cursor;
        UncheckedSearchGraphCursorArtifact {
            schema_id: SEARCH_GRAPH_CURSOR_SCHEMA_ID.to_owned(),
            schema_version: SEARCH_GRAPH_CURSOR_SCHEMA_VERSION.to_owned(),
            loop_id: self.loop_id,
            graph_artifact: self.graph_artifact,
            state: state.into_unchecked(),
            materialized_node_ids: materialized_node_ids.into_iter().collect(),
        }
    }
}

impl SearchGraphCursor {
    pub fn bootstrap(
        request: SearchGraphCursorBootstrapRequest,
    ) -> Result<Self, SearchGraphCursorError> {
        let materialized_node_ids = request
            .materialized_node_ids
            .into_iter()
            .map(|node_id| {
                require_non_empty(&node_id)?;
                Ok(node_id)
            })
            .collect::<Result<BTreeSet<_>, SearchGraphCursorError>>()?;
        if materialized_node_ids.is_empty() {
            return Err(SearchGraphCursorError::MaterializedGraphEmpty);
        }
        for node_id in std::iter::once(request.state.current_node())
            .chain(request.state.frontier().iter().map(String::as_str))
            .chain(request.state.visited().iter().map(String::as_str))
        {
            if !materialized_node_ids.contains(node_id) {
                return Err(SearchGraphCursorError::NodeNotMaterialized(
                    node_id.to_string(),
                ));
            }
        }
        Ok(Self {
            state: request.state,
            materialized_node_ids,
        })
    }

    pub fn state(&self) -> &InteractiveSearchGraphState {
        &self.state
    }

    pub fn materialized_node_ids(&self) -> &BTreeSet<String> {
        &self.materialized_node_ids
    }

    pub fn jump(
        &self,
        request: SearchGraphCursorJumpRequest<'_>,
    ) -> Result<Self, SearchGraphCursorError> {
        require_non_empty(request.target_node_id)?;
        require_non_empty(request.next_state_digest)?;
        if request.next_state_digest == self.state.state_digest() {
            return Err(SearchGraphCursorError::StateDigestUnchanged);
        }
        if request.target_node_id == self.state.current_node() {
            return Err(SearchGraphCursorError::CurrentNodeReplay);
        }
        if !self.materialized_node_ids.contains(request.target_node_id) {
            return Err(SearchGraphCursorError::NodeNotMaterialized(
                request.target_node_id.to_string(),
            ));
        }
        let mut unchecked = self.state.clone().into_unchecked();
        unchecked
            .visited
            .retain(|node| node != &unchecked.current_node);
        unchecked.visited.insert(0, unchecked.current_node.clone());
        unchecked.state_digest = request.next_state_digest.to_string();
        unchecked.current_node = request.target_node_id.to_string();
        let state = InteractiveSearchGraphState::validate(unchecked)?;
        Ok(Self {
            state,
            materialized_node_ids: self.materialized_node_ids.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchGraphCursorError {
    SchemaIdentity,
    EmptyIdentity,
    DuplicateSetValue(&'static str, String),
    RouteBudgetExceeded,
    MaterializedGraphEmpty,
    NodeNotMaterialized(String),
    StateDigestUnchanged,
    CurrentNodeReplay,
}

impl fmt::Display for SearchGraphCursorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaIdentity => {
                formatter.write_str("invalid search graph cursor schema identity")
            }
            Self::EmptyIdentity => formatter.write_str("search graph identity must not be empty"),
            Self::DuplicateSetValue(field, value) => {
                write!(formatter, "search graph {field} contains duplicate {value}")
            }
            Self::RouteBudgetExceeded => {
                formatter.write_str("search graph state exceeds its route budget")
            }
            Self::MaterializedGraphEmpty => {
                formatter.write_str("search graph cursor requires materialized nodes")
            }
            Self::NodeNotMaterialized(node_id) => {
                write!(
                    formatter,
                    "search graph node is not materialized: {node_id}"
                )
            }
            Self::StateDigestUnchanged => {
                formatter.write_str("search graph jump requires a new state digest")
            }
            Self::CurrentNodeReplay => {
                formatter.write_str("search graph cursor already points at the requested node")
            }
        }
    }
}

impl std::error::Error for SearchGraphCursorError {}

fn require_non_empty(value: &str) -> Result<(), SearchGraphCursorError> {
    if value.trim().is_empty() {
        Err(SearchGraphCursorError::EmptyIdentity)
    } else {
        Ok(())
    }
}

fn validate_unique_non_empty(
    field: &'static str,
    values: &[String],
) -> Result<(), SearchGraphCursorError> {
    let mut seen = BTreeSet::new();
    for value in values {
        require_non_empty(value)?;
        if !seen.insert(value) {
            return Err(SearchGraphCursorError::DuplicateSetValue(
                field,
                value.clone(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/search_graph_cursor.rs"]
mod tests;
