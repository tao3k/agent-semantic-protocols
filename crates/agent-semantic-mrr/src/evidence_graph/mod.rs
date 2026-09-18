// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-owned construction of the shared semantic EvidenceGraph.
//!
//! Language providers contribute immutable syntax and parser facts. This
//! module is the one graph-derivation authority: it compiles the admitted GQL
//! rule prelude, executes the resulting relation joins in Ascent, and emits a
//! deterministic `semantic-evidence-graph.v1` artifact. Provider code never
//! constructs graph edges or graph summaries.

mod build;
mod model;
mod rules;
mod validation;

pub use build::derive_evidence_graph;
pub use model::{
    EVIDENCE_GRAPH_PROGRAM_ID, EVIDENCE_GRAPH_PROTOCOL_ID, EVIDENCE_GRAPH_SCHEMA_ID, EvidenceGraph,
    EvidenceGraphBuildInput, EvidenceGraphBuildOutput, EvidenceGraphDerivationReceipt,
    EvidenceGraphEdge, EvidenceGraphEdgeKind, EvidenceGraphGap, EvidenceGraphLimits,
    EvidenceGraphLocation, EvidenceGraphNode, EvidenceGraphNodeKind, EvidenceGraphNodeStatus,
    EvidenceGraphProducer, EvidenceGraphProject, EvidenceGraphRuleReceipt, EvidenceGraphSourceFact,
    EvidenceGraphSummary,
};
pub use validation::EvidenceGraphBuildError;
