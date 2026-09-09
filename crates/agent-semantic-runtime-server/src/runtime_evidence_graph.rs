// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime publication boundary for the shared semantic EvidenceGraph.

/// Immutable facts and metadata admitted for one Runtime graph generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEvidenceGraphBuildInput {
    pub graph_id: String,
    pub source_generation_digest: String,
    pub project: agent_semantic_mrr::EvidenceGraphProject,
    pub nodes: Vec<agent_semantic_mrr::EvidenceGraphNode>,
    pub source_facts: Vec<agent_semantic_mrr::EvidenceGraphSourceFact>,
    pub gaps: Vec<agent_semantic_mrr::EvidenceGraphGap>,
    pub limits: agent_semantic_mrr::EvidenceGraphLimits,
}

/// Build the only production EvidenceGraph artifact for an immutable Runtime
/// generation. The producer identity is fixed here rather than accepted from a
/// language provider.
pub fn build_runtime_evidence_graph(
    input: RuntimeEvidenceGraphBuildInput,
) -> Result<agent_semantic_mrr::EvidenceGraphBuildOutput, String> {
    agent_semantic_mrr::derive_evidence_graph(agent_semantic_mrr::EvidenceGraphBuildInput {
        graph_id: input.graph_id,
        source_generation_digest: input.source_generation_digest,
        producer: agent_semantic_mrr::EvidenceGraphProducer {
            language_id: "runtime".to_owned(),
            provider_id: "asp-runtime-server".to_owned(),
            namespace: "agent.semantic-protocols.runtime".to_owned(),
        },
        project: input.project,
        nodes: input.nodes,
        source_facts: input.source_facts,
        gaps: input.gaps,
        limits: input.limits,
    })
    .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "../tests/unit/runtime_evidence_graph.rs"]
mod tests;
