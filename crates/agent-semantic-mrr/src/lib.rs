// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! The single ASP maintenance window for MRR, Ascent-backed closure, GQL, and
//! Scheme/POO ahead-of-time program bindings.
#![forbid(unsafe_code)]

mod evidence_graph;
mod graph_relation_pattern;
mod project_topology_reasoning;
mod scheme_aot_program;

/// The admitted MRR kernel facade. ASP crates consume kernel types through
/// this re-export instead of acquiring the Ascent-backed implementation directly.
pub use meta_relational_reasoning as kernel;

pub use evidence_graph::{
    EVIDENCE_GRAPH_PROGRAM_ID, EVIDENCE_GRAPH_PROTOCOL_ID, EVIDENCE_GRAPH_SCHEMA_ID, EvidenceGraph,
    EvidenceGraphBuildError, EvidenceGraphBuildInput, EvidenceGraphBuildOutput,
    EvidenceGraphDerivationReceipt, EvidenceGraphEdge, EvidenceGraphEdgeKind, EvidenceGraphGap,
    EvidenceGraphLimits, EvidenceGraphLocation, EvidenceGraphNode, EvidenceGraphNodeKind,
    EvidenceGraphNodeStatus, EvidenceGraphProducer, EvidenceGraphProject, EvidenceGraphRuleReceipt,
    EvidenceGraphSourceFact, EvidenceGraphSummary, derive_evidence_graph,
};
pub use graph_relation_pattern::{
    GraphRelationDirection, GraphRelationPattern, GraphRelationPatternError,
    compile_graph_relation_pattern_v1,
};
pub use project_topology_reasoning::{
    ProjectTopologyReasoningCandidateV1, ProjectTopologyReasoningErrorV1,
    ProjectTopologyReasoningInputV1, ProjectTopologyReasoningLimitsV1,
    ProjectTopologyReasoningReceiptV1, evaluate_project_topology_v1,
};
pub use scheme_aot_program::{
    SchemeAotProgramBinding, SchemeAotProgramBindingError, admit_scheme_aot_program_binding,
};
