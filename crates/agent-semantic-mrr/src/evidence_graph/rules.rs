// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::{GraphRelationDirection, compile_graph_relation_pattern_v1};

use super::validation::{EvidenceGraphBuildError, error, hash_serializable};
use super::{EVIDENCE_GRAPH_PROGRAM_ID, EvidenceGraphEdgeKind, EvidenceGraphNodeKind};

struct RuleSpec {
    suffix: &'static str,
    from_kind: EvidenceGraphNodeKind,
    input_relation: &'static str,
    to_kind: EvidenceGraphNodeKind,
    output_kind: EvidenceGraphEdgeKind,
}

pub(super) struct CompiledRule {
    pub(super) rule_id: String,
    pub(super) from_kind: String,
    pub(super) input_relation: String,
    pub(super) to_kind: String,
    pub(super) output_kind: EvidenceGraphEdgeKind,
    pub(super) gql_plan_digest: String,
}

const RULES: &[RuleSpec] = &[
    rule(
        "invariant-review-source",
        EvidenceGraphNodeKind::InvariantCandidate,
        "DERIVED_FROM",
        EvidenceGraphNodeKind::ReviewPacket,
        EvidenceGraphEdgeKind::DerivedFrom,
    ),
    rule(
        "invariant-owner-source",
        EvidenceGraphNodeKind::InvariantCandidate,
        "DECLARED_BY",
        EvidenceGraphNodeKind::Owner,
        EvidenceGraphEdgeKind::DerivedFrom,
    ),
    rule(
        "receipt-review-source",
        EvidenceGraphNodeKind::VerificationReceipt,
        "DERIVED_FROM",
        EvidenceGraphNodeKind::ReviewPacket,
        EvidenceGraphEdgeKind::DerivedFrom,
    ),
    rule(
        "snapshot-review-source",
        EvidenceGraphNodeKind::BehaviorSnapshot,
        "DERIVED_FROM",
        EvidenceGraphNodeKind::ReviewPacket,
        EvidenceGraphEdgeKind::DerivedFrom,
    ),
    rule(
        "readiness-review-source",
        EvidenceGraphNodeKind::DeterminismReadiness,
        "DERIVED_FROM",
        EvidenceGraphNodeKind::ReviewPacket,
        EvidenceGraphEdgeKind::DerivedFrom,
    ),
    rule(
        "proof-review-source",
        EvidenceGraphNodeKind::FormalProofPilot,
        "DERIVED_FROM",
        EvidenceGraphNodeKind::ReviewPacket,
        EvidenceGraphEdgeKind::DerivedFrom,
    ),
    rule(
        "waiver-review-source",
        EvidenceGraphNodeKind::Waiver,
        "DERIVED_FROM",
        EvidenceGraphNodeKind::ReviewPacket,
        EvidenceGraphEdgeKind::DerivedFrom,
    ),
    rule(
        "action-review-source",
        EvidenceGraphNodeKind::ReviewAction,
        "DERIVED_FROM",
        EvidenceGraphNodeKind::ReviewPacket,
        EvidenceGraphEdgeKind::DerivedFrom,
    ),
    rule(
        "invariant-requires-receipt",
        EvidenceGraphNodeKind::InvariantCandidate,
        "REQUIRES_EVIDENCE",
        EvidenceGraphNodeKind::VerificationReceipt,
        EvidenceGraphEdgeKind::RequiresEvidence,
    ),
    rule(
        "invariant-verified-by-receipt",
        EvidenceGraphNodeKind::InvariantCandidate,
        "VERIFIED_BY",
        EvidenceGraphNodeKind::VerificationReceipt,
        EvidenceGraphEdgeKind::VerifiedBy,
    ),
    rule(
        "invariant-observed-by-snapshot",
        EvidenceGraphNodeKind::InvariantCandidate,
        "OBSERVED_BY",
        EvidenceGraphNodeKind::BehaviorSnapshot,
        EvidenceGraphEdgeKind::ObservedBy,
    ),
    rule(
        "invariant-waived-by-waiver",
        EvidenceGraphNodeKind::InvariantCandidate,
        "WAIVED_BY",
        EvidenceGraphNodeKind::Waiver,
        EvidenceGraphEdgeKind::WaivedBy,
    ),
    rule(
        "invariant-reviewed-by-packet",
        EvidenceGraphNodeKind::InvariantCandidate,
        "REVIEWED_BY",
        EvidenceGraphNodeKind::ReviewPacket,
        EvidenceGraphEdgeKind::ReviewedBy,
    ),
    rule(
        "packet-suggests-action",
        EvidenceGraphNodeKind::ReviewPacket,
        "SUGGESTS_ACTION",
        EvidenceGraphNodeKind::ReviewAction,
        EvidenceGraphEdgeKind::SuggestsAction,
    ),
    rule(
        "proof-supports-invariant",
        EvidenceGraphNodeKind::FormalProofPilot,
        "SUPPORTS_CLAIM",
        EvidenceGraphNodeKind::InvariantCandidate,
        EvidenceGraphEdgeKind::SupportsClaim,
    ),
];

const fn rule(
    suffix: &'static str,
    from_kind: EvidenceGraphNodeKind,
    input_relation: &'static str,
    to_kind: EvidenceGraphNodeKind,
    output_kind: EvidenceGraphEdgeKind,
) -> RuleSpec {
    RuleSpec {
        suffix,
        from_kind,
        input_relation,
        to_kind,
        output_kind,
    }
}

pub(super) fn compile_rules() -> Result<Vec<CompiledRule>, EvidenceGraphBuildError> {
    let mut compiled = Vec::with_capacity(RULES.len());
    for spec in RULES {
        let source = format!(
            "MATCH (source:{})-[:{}]->(target:{}) RETURN source, target",
            spec.from_kind.gql_label(),
            spec.input_relation,
            spec.to_kind.gql_label(),
        );
        let pattern =
            compile_graph_relation_pattern_v1("gql", &[source], None).map_err(|cause| {
                error(format!(
                    "compile EvidenceGraph rule {}: {cause}",
                    spec.suffix
                ))
            })?;
        if pattern.direction != GraphRelationDirection::Out
            || pattern.projected_bindings.len() != 2
            || !pattern
                .left_kind
                .eq_ignore_ascii_case(spec.from_kind.gql_label())
            || !pattern
                .right_kind
                .eq_ignore_ascii_case(spec.to_kind.gql_label())
            || pattern.relation != spec.input_relation
        {
            return Err(error(format!(
                "compiled EvidenceGraph rule {} changed semantic shape: direction={:?} left={} relation={} right={} projected={:?}",
                spec.suffix,
                pattern.direction,
                pattern.left_kind,
                pattern.relation,
                pattern.right_kind,
                pattern.projected_bindings,
            )));
        }
        compiled.push(CompiledRule {
            rule_id: format!("{EVIDENCE_GRAPH_PROGRAM_ID}.{}", spec.suffix),
            from_kind: pattern.left_kind.clone(),
            input_relation: pattern.relation.clone(),
            to_kind: pattern.right_kind.clone(),
            output_kind: spec.output_kind,
            gql_plan_digest: hash_serializable(&pattern)?,
        });
    }
    compiled.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
    Ok(compiled)
}
