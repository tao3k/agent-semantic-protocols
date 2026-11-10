// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::{BTreeMap, BTreeSet};

use ascent::ascent;
use serde_json::Value;

use super::model::{
    EVIDENCE_GRAPH_PROGRAM_ID, EVIDENCE_GRAPH_PROTOCOL_ID, EVIDENCE_GRAPH_SCHEMA_ID, EvidenceGraph,
    EvidenceGraphBuildInput, EvidenceGraphBuildOutput, EvidenceGraphDerivationReceipt,
    EvidenceGraphEdge, EvidenceGraphNodeKind, EvidenceGraphNodeStatus, EvidenceGraphRuleReceipt,
    EvidenceGraphSummary,
};
use super::rules::compile_rules;
use super::validation::{
    EvidenceGraphBuildError, edge_kind_text, error, hash_serializable, parse_edge_kind,
    validate_input,
};

/// Build one immutable EvidenceGraph from Runtime-normalized source facts.
pub fn derive_evidence_graph(
    mut input: EvidenceGraphBuildInput,
) -> Result<EvidenceGraphBuildOutput, EvidenceGraphBuildError> {
    validate_input(&input)?;
    input
        .nodes
        .sort_by(|left, right| left.node_id.cmp(&right.node_id));
    input.source_facts.sort();
    input
        .gaps
        .sort_by(|left, right| left.gap_id.cmp(&right.gap_id));

    let node_kinds = input
        .nodes
        .iter()
        .map(|node| {
            (
                node.node_id.clone(),
                node.kind.gql_label().to_ascii_uppercase(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let compiled_rules = compile_rules()?;
    let facts = input
        .source_facts
        .iter()
        .map(|fact| {
            (
                fact.relation.clone(),
                node_kinds[&fact.from_node_id].clone(),
                fact.from_node_id.clone(),
                node_kinds[&fact.to_node_id].clone(),
                fact.to_node_id.clone(),
                fact.fact_id.clone(),
            )
        })
        .collect::<Vec<_>>();
    let rules = compiled_rules
        .iter()
        .map(|compiled| {
            (
                compiled.rule_id.clone(),
                compiled.input_relation.clone(),
                compiled.from_kind.clone(),
                compiled.to_kind.clone(),
                edge_kind_text(compiled.output_kind).to_owned(),
                compiled.gql_plan_digest.clone(),
            )
        })
        .collect::<Vec<_>>();

    ascent! {
        relation facts(String, String, String, String, String, String);
        relation rules(String, String, String, String, String, String);
        relation derived(String, String, String, String, String, String);

        derived(output_kind, from_id, to_id, rule_id, gql_digest, fact_id) <--
            facts(input_relation, from_kind, from_id, to_kind, to_id, fact_id),
            rules(rule_id, input_relation, from_kind, to_kind, output_kind, gql_digest);
    }
    let mut program = AscentProgram {
        facts,
        rules,
        ..AscentProgram::default()
    };
    program.run();

    let mut grouped = BTreeMap::<(String, String, String, String, String), BTreeSet<String>>::new();
    for (kind, from, to, rule_id, gql_digest, fact_id) in program.derived {
        grouped
            .entry((kind, from, to, rule_id, gql_digest))
            .or_default()
            .insert(fact_id);
    }
    if grouped.len() > input.limits.max_derived_edges {
        return Err(error(format!(
            "evidence graph derived-edge budget exceeded: required={} limit={}",
            grouped.len(),
            input.limits.max_derived_edges
        )));
    }

    let edges = build_edges(grouped)?;
    let rule_receipts = compiled_rules
        .iter()
        .map(|compiled| EvidenceGraphRuleReceipt {
            rule_id: compiled.rule_id.clone(),
            gql_plan_digest: compiled.gql_plan_digest.clone(),
            input_relation: compiled.input_relation.clone(),
            output_edge_kind: compiled.output_kind,
        })
        .collect::<Vec<_>>();
    let source_fact_digest = hash_serializable(&input.source_facts)?;
    let program_digest = hash_serializable(&rule_receipts)?;
    let graph_digest = hash_serializable(&(
        EVIDENCE_GRAPH_PROGRAM_ID,
        &input.graph_id,
        &input.source_generation_digest,
        &input.producer,
        &input.project,
        &input.nodes,
        &input.source_facts,
        &input.gaps,
        &edges,
        &rule_receipts,
    ))?;
    let graph = build_graph(
        &input,
        edges,
        &rule_receipts,
        &program_digest,
        &graph_digest,
    );
    let receipt = EvidenceGraphDerivationReceipt {
        schema_id: "agent.semantic-protocols.evidence-graph-derivation-receipt".to_owned(),
        schema_version: "1".to_owned(),
        program_id: EVIDENCE_GRAPH_PROGRAM_ID.to_owned(),
        program_digest,
        source_generation_digest: input.source_generation_digest,
        source_fact_digest,
        graph_digest,
        input_node_count: graph.nodes.len(),
        input_fact_count: input.source_facts.len(),
        derived_edge_count: graph.edges.len(),
        rules: rule_receipts,
        complete: true,
    };
    Ok(EvidenceGraphBuildOutput { graph, receipt })
}

fn build_edges(
    grouped: BTreeMap<(String, String, String, String, String), BTreeSet<String>>,
) -> Result<Vec<EvidenceGraphEdge>, EvidenceGraphBuildError> {
    let mut edges = Vec::with_capacity(grouped.len());
    for ((kind, from, to, rule_id, gql_digest), premise_ids) in grouped {
        let kind = parse_edge_kind(&kind).expect("fixed program emits known edge kinds");
        let premise_ids = premise_ids.into_iter().collect::<Vec<_>>();
        let edge_identity = hash_serializable(&(
            edge_kind_text(kind),
            from.as_str(),
            to.as_str(),
            rule_id.as_str(),
            &premise_ids,
        ))?;
        edges.push(EvidenceGraphEdge {
            edge_id: format!(
                "edge:{}",
                edge_identity
                    .strip_prefix("blake3-256:")
                    .expect("hash helper returns a qualified digest")
            ),
            kind,
            from_node_id: from,
            to_node_id: to,
            label: None,
            fields: BTreeMap::from([
                ("derivationRuleId".to_owned(), Value::String(rule_id)),
                ("gqlPlanDigest".to_owned(), Value::String(gql_digest)),
                (
                    "premiseFactIds".to_owned(),
                    Value::Array(premise_ids.into_iter().map(Value::String).collect()),
                ),
            ]),
        });
    }
    edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    Ok(edges)
}

fn build_graph(
    input: &EvidenceGraphBuildInput,
    edges: Vec<EvidenceGraphEdge>,
    rule_receipts: &[EvidenceGraphRuleReceipt],
    program_digest: &str,
    graph_digest: &str,
) -> EvidenceGraph {
    let summary = EvidenceGraphSummary {
        nodes: input.nodes.len(),
        edges: edges.len(),
        owners: input
            .nodes
            .iter()
            .filter(|node| node.kind == EvidenceGraphNodeKind::Owner)
            .count(),
        claims: input
            .nodes
            .iter()
            .filter(|node| node.kind == EvidenceGraphNodeKind::InvariantCandidate)
            .count(),
        stale_items: input
            .nodes
            .iter()
            .filter(|node| {
                matches!(
                    node.status,
                    Some(EvidenceGraphNodeStatus::Stale | EvidenceGraphNodeStatus::Expired)
                )
            })
            .count(),
        gaps: input.gaps.len(),
    };
    let rule_ids = rule_receipts
        .iter()
        .map(|receipt| Value::String(receipt.rule_id.clone()))
        .collect::<Vec<_>>();
    EvidenceGraph {
        schema_id: EVIDENCE_GRAPH_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        protocol_id: EVIDENCE_GRAPH_PROTOCOL_ID.to_owned(),
        protocol_version: "1".to_owned(),
        graph_id: input.graph_id.clone(),
        producer: input.producer.clone(),
        project: input.project.clone(),
        summary,
        nodes: input.nodes.clone(),
        edges,
        gaps: input.gaps.clone(),
        fields: BTreeMap::from([
            (
                "sourceGenerationDigest".to_owned(),
                Value::String(input.source_generation_digest.clone()),
            ),
            (
                "graphDigest".to_owned(),
                Value::String(graph_digest.to_owned()),
            ),
            (
                "derivationProgramId".to_owned(),
                Value::String(EVIDENCE_GRAPH_PROGRAM_ID.to_owned()),
            ),
            (
                "derivationProgramDigest".to_owned(),
                Value::String(program_digest.to_owned()),
            ),
            ("derivationRuleIds".to_owned(), Value::Array(rule_ids)),
        ]),
    }
}
