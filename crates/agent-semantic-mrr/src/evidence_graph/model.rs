// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const EVIDENCE_GRAPH_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-evidence-graph";
pub const EVIDENCE_GRAPH_PROTOCOL_ID: &str = "agent.semantic-protocols.evidence-graph";
pub const EVIDENCE_GRAPH_PROGRAM_ID: &str = "mrr.evidence-graph.runtime";

/// Hard bounds applied before and after the fixed-point evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceGraphLimits {
    pub max_nodes: usize,
    pub max_source_facts: usize,
    pub max_derived_edges: usize,
}

/// Runtime generation and project metadata supplied by the publication owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceGraphBuildInput {
    pub graph_id: String,
    pub source_generation_digest: String,
    pub producer: EvidenceGraphProducer,
    pub project: EvidenceGraphProject,
    pub nodes: Vec<EvidenceGraphNode>,
    pub source_facts: Vec<EvidenceGraphSourceFact>,
    pub gaps: Vec<EvidenceGraphGap>,
    pub limits: EvidenceGraphLimits,
}

/// One immutable, normalized fact consumed by the central rule program.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceGraphSourceFact {
    pub fact_id: String,
    pub relation: String,
    pub from_node_id: String,
    pub to_node_id: String,
}

/// Central graph plus the complete derivation receipt used to publish it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceGraphBuildOutput {
    pub(super) graph: EvidenceGraph,
    pub(super) receipt: EvidenceGraphDerivationReceipt,
}

impl EvidenceGraphBuildOutput {
    #[must_use]
    pub const fn graph(&self) -> &EvidenceGraph {
        &self.graph
    }

    #[must_use]
    pub const fn receipt(&self) -> &EvidenceGraphDerivationReceipt {
        &self.receipt
    }

    #[must_use]
    pub fn into_parts(self) -> (EvidenceGraph, EvidenceGraphDerivationReceipt) {
        (self.graph, self.receipt)
    }
}

/// Replay metadata proving which GQL plans and source generation produced the graph.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceGraphDerivationReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub program_id: String,
    pub program_digest: String,
    pub source_generation_digest: String,
    pub source_fact_digest: String,
    pub graph_digest: String,
    pub input_node_count: usize,
    pub input_fact_count: usize,
    pub derived_edge_count: usize,
    pub rules: Vec<EvidenceGraphRuleReceipt>,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceGraphRuleReceipt {
    pub rule_id: String,
    pub gql_plan_digest: String,
    pub input_relation: String,
    pub output_edge_kind: EvidenceGraphEdgeKind,
}

/// The public `semantic-evidence-graph.v1` artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceGraph {
    pub schema_id: String,
    pub schema_version: String,
    pub protocol_id: String,
    pub protocol_version: String,
    pub graph_id: String,
    pub producer: EvidenceGraphProducer,
    pub project: EvidenceGraphProject,
    pub summary: EvidenceGraphSummary,
    pub nodes: Vec<EvidenceGraphNode>,
    pub edges: Vec<EvidenceGraphEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<EvidenceGraphGap>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceGraphProducer {
    pub language_id: String,
    pub provider_id: String,
    pub namespace: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceGraphProject {
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceGraphSummary {
    pub nodes: usize,
    pub edges: usize,
    pub owners: usize,
    pub claims: usize,
    pub stale_items: usize,
    pub gaps: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceGraphNode {
    pub node_id: String,
    pub kind: EvidenceGraphNodeKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiver_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<EvidenceGraphNodeStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<EvidenceGraphLocation>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceGraphNodeKind {
    Owner,
    InvariantCandidate,
    VerificationReceipt,
    BehaviorSnapshot,
    DeterminismReadiness,
    FormalProofPilot,
    ReviewPacket,
    Waiver,
    ReviewAction,
}

impl EvidenceGraphNodeKind {
    pub(super) const fn gql_label(self) -> &'static str {
        match self {
            Self::Owner => "Owner",
            Self::InvariantCandidate => "InvariantCandidate",
            Self::VerificationReceipt => "VerificationReceipt",
            Self::BehaviorSnapshot => "BehaviorSnapshot",
            Self::DeterminismReadiness => "DeterminismReadiness",
            Self::FormalProofPilot => "FormalProofPilot",
            Self::ReviewPacket => "ReviewPacket",
            Self::Waiver => "Waiver",
            Self::ReviewAction => "ReviewAction",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceGraphNodeStatus {
    Current,
    Changed,
    Missing,
    Stale,
    Expired,
    Ready,
    NeedsInjection,
    Blocked,
    Unknown,
    Proved,
    ProvedBounded,
    Failed,
    Skipped,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceGraphLocation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceGraphEdge {
    pub edge_id: String,
    pub kind: EvidenceGraphEdgeKind,
    pub from_node_id: String,
    pub to_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceGraphEdgeKind {
    DerivedFrom,
    RequiresEvidence,
    VerifiedBy,
    ObservedBy,
    WaivedBy,
    ReviewedBy,
    SuggestsAction,
    SupportsClaim,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceGraphGap {
    pub gap_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_path: Option<String>,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
}
