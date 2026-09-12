// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-generation-owned cardinality budget for Search Playbook execution.

use serde::Serialize;

use crate::RuntimeQueryGeneration;

const SCHEMA_ID: &str = "agent.semantic-protocols.runtime-search-execution-budget";
const GRAPH_DEPTH_V1_LIMIT: usize = 16;
const GRAPH_NODE_V1_LIMIT: usize = 256;
const GRAPH_EDGE_V1_LIMIT: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RuntimeSearchExecutionBudget {
    schema_id: String,
    schema_version: String,
    authority: String,
    generation_digest: String,
    cardinality: RuntimeSearchGenerationCardinality,
    limits: RuntimeSearchExecutionLimits,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeSearchGenerationCardinality {
    indexed_owner_count: usize,
    corpus_byte_count: usize,
    corpus_line_count: u32,
    graph_node_count: usize,
    graph_edge_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeSearchExecutionLimits {
    rg_match_count: u32,
    lexical_owner_count: u32,
    syntax_selector_count: usize,
    graph_candidate_owner_count: usize,
    graph_depth: usize,
    graph_node_count: usize,
    graph_edge_count: usize,
    graph_result_count: usize,
    evidence_item_count: usize,
}

impl RuntimeSearchExecutionBudget {
    pub(crate) fn derive(generation: &RuntimeQueryGeneration) -> Result<Self, String> {
        let resident = generation.resident();
        let corpus = resident.resident_grep_corpus();
        let graph = resident.graph_generation()?;
        let graph_node_count = graph.map_or(0, |graph| graph.node_count());
        let graph_edge_count = graph.map_or(0, |graph| graph.edge_count());
        let corpus_line_count = corpus.owner_spans.last().map_or(0, |span| span.end_line);
        Self::from_cardinalities(
            generation.generation_digest(),
            resident.indexed_owner_count(),
            corpus.corpus_byte_count(),
            corpus_line_count,
            graph_node_count,
            graph_edge_count,
        )
    }

    fn from_cardinalities(
        generation_digest: &str,
        indexed_owner_count: usize,
        corpus_byte_count: usize,
        corpus_line_count: u64,
        graph_node_count: usize,
        graph_edge_count: usize,
    ) -> Result<Self, String> {
        let generation_digest = agent_semantic_search::canonical_blake3_digest(generation_digest)?;
        if indexed_owner_count == 0 || corpus_byte_count == 0 || corpus_line_count == 0 {
            return Err("reasonKind=runtime-search-execution-budget-empty-generation".to_owned());
        }
        let corpus_line_count = u32::try_from(corpus_line_count).map_err(|_| {
            "reasonKind=runtime-search-execution-budget-cardinality-overflow field=corpusLineCount"
                .to_owned()
        })?;
        let lexical_owner_count = u32::try_from(indexed_owner_count).map_err(|_| {
            "reasonKind=runtime-search-execution-budget-cardinality-overflow field=indexedOwnerCount"
                .to_owned()
        })?;
        let budget = Self {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            authority: "runtime-generation".to_owned(),
            generation_digest,
            cardinality: RuntimeSearchGenerationCardinality {
                indexed_owner_count,
                corpus_byte_count,
                corpus_line_count,
                graph_node_count,
                graph_edge_count,
            },
            limits: RuntimeSearchExecutionLimits {
                rg_match_count: corpus_line_count,
                lexical_owner_count,
                syntax_selector_count: corpus_byte_count.max(indexed_owner_count),
                graph_candidate_owner_count: indexed_owner_count,
                graph_depth: graph_node_count.min(GRAPH_DEPTH_V1_LIMIT),
                graph_node_count: graph_node_count.min(GRAPH_NODE_V1_LIMIT),
                graph_edge_count: graph_edge_count.min(GRAPH_EDGE_V1_LIMIT),
                graph_result_count: indexed_owner_count
                    .min(agent_semantic_search::WORKSPACE_SEARCH_PLAYBOOK_V1_EVIDENCE_ITEM_LIMIT),
                evidence_item_count:
                    agent_semantic_search::WORKSPACE_SEARCH_PLAYBOOK_V1_EVIDENCE_ITEM_LIMIT,
            },
        };
        budget.validate()?;
        Ok(budget)
    }

    pub(crate) fn validate_for_generation(&self, generation_digest: &str) -> Result<(), String> {
        self.validate()?;
        if self.generation_digest != generation_digest {
            return Err(
                "reasonKind=runtime-search-execution-budget-generation-mismatch".to_owned(),
            );
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        let cardinality = &self.cardinality;
        let limits = &self.limits;
        let valid = self.schema_id == SCHEMA_ID
            && self.schema_version == "1"
            && self.authority == "runtime-generation"
            && cardinality.indexed_owner_count > 0
            && cardinality.corpus_byte_count > 0
            && cardinality.corpus_line_count > 0
            && limits.rg_match_count == cardinality.corpus_line_count
            && usize::try_from(limits.lexical_owner_count).ok()
                == Some(cardinality.indexed_owner_count)
            && limits.syntax_selector_count
                == cardinality
                    .corpus_byte_count
                    .max(cardinality.indexed_owner_count)
            && limits.graph_candidate_owner_count == cardinality.indexed_owner_count
            && limits.graph_depth == cardinality.graph_node_count.min(GRAPH_DEPTH_V1_LIMIT)
            && limits.graph_node_count == cardinality.graph_node_count.min(GRAPH_NODE_V1_LIMIT)
            && limits.graph_edge_count == cardinality.graph_edge_count.min(GRAPH_EDGE_V1_LIMIT)
            && limits.graph_result_count
                == cardinality
                    .indexed_owner_count
                    .min(agent_semantic_search::WORKSPACE_SEARCH_PLAYBOOK_V1_EVIDENCE_ITEM_LIMIT)
            && limits.evidence_item_count
                == agent_semantic_search::WORKSPACE_SEARCH_PLAYBOOK_V1_EVIDENCE_ITEM_LIMIT;
        if !valid {
            return Err("reasonKind=runtime-search-execution-budget-invalid".to_owned());
        }
        Ok(())
    }

    pub(crate) fn rg_match_limit(&self) -> u32 {
        self.limits.rg_match_count
    }

    pub(crate) fn lexical_owner_limit(&self) -> u32 {
        self.limits.lexical_owner_count
    }

    pub(crate) fn syntax_selector_limit(&self) -> usize {
        self.limits.syntax_selector_count
    }

    pub(crate) fn graph_candidate_owner_limit(&self) -> usize {
        self.limits.graph_candidate_owner_count
    }

    pub(crate) fn graph_evaluation_budget(
        &self,
    ) -> Option<agent_semantic_search::ResidentGraphEvaluationBudget> {
        let budget = agent_semantic_search::ResidentGraphEvaluationBudget {
            max_depth: self.limits.graph_depth,
            max_nodes: self.limits.graph_node_count,
            max_edges: self.limits.graph_edge_count,
            max_results: self.limits.graph_result_count,
        };
        (budget.max_depth != 0
            && budget.max_nodes != 0
            && budget.max_edges != 0
            && budget.max_results != 0)
            .then_some(budget)
    }

    pub(crate) fn evidence_item_limit(&self) -> usize {
        self.limits.evidence_item_count
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_search_execution_budget.rs"]
mod tests;
