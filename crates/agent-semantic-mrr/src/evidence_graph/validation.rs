// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Serialize;
use serde_json::Value;

use super::{EvidenceGraphBuildInput, EvidenceGraphEdgeKind};

pub(super) fn validate_input(
    input: &EvidenceGraphBuildInput,
) -> Result<(), EvidenceGraphBuildError> {
    if input.limits.max_nodes == 0
        || input.limits.max_source_facts == 0
        || input.limits.max_derived_edges == 0
    {
        return Err(error("evidence graph limits must be non-zero"));
    }
    if input.nodes.len() > input.limits.max_nodes {
        return Err(error(format!(
            "evidence graph node budget exceeded: required={} limit={}",
            input.nodes.len(),
            input.limits.max_nodes
        )));
    }
    if input.source_facts.len() > input.limits.max_source_facts {
        return Err(error(format!(
            "evidence graph source-fact budget exceeded: required={} limit={}",
            input.source_facts.len(),
            input.limits.max_source_facts
        )));
    }
    if !schema_identifier(&input.graph_id)
        || !qualified_blake3(&input.source_generation_digest)
        || input.project.root.trim().is_empty()
        || input.project.package.as_deref().is_some_and(str::is_empty)
        || !producer_identifier(&input.producer.language_id)
        || !producer_identifier(&input.producer.provider_id)
        || !producer_namespace(&input.producer.namespace)
    {
        return Err(error("evidence graph generation metadata is invalid"));
    }
    validate_fields(&input.project.fields)?;

    let mut node_ids = BTreeSet::new();
    for node in &input.nodes {
        if !node_ids.insert(node.node_id.as_str())
            || !schema_identifier(&node.node_id)
            || node.label.trim().is_empty()
            || node.candidate_id.as_deref().is_some_and(str::is_empty)
            || node.receipt_id.as_deref().is_some_and(str::is_empty)
            || node.snapshot_id.as_deref().is_some_and(str::is_empty)
            || node.readiness_id.as_deref().is_some_and(str::is_empty)
            || node.proof_id.as_deref().is_some_and(str::is_empty)
            || node.packet_id.as_deref().is_some_and(str::is_empty)
            || node.waiver_id.as_deref().is_some_and(str::is_empty)
            || node.action_id.as_deref().is_some_and(str::is_empty)
            || node.summary.as_deref().is_some_and(str::is_empty)
            || node.owner_path.as_deref().is_some_and(invalid_project_path)
            || node.location.as_ref().is_some_and(|location| {
                location.path.as_deref().is_some_and(invalid_project_path)
                    || location.line == Some(0)
            })
        {
            return Err(error(
                "evidence graph contains an invalid or duplicate node",
            ));
        }
        validate_fields(&node.fields)?;
    }
    let mut fact_ids = BTreeSet::new();
    for fact in &input.source_facts {
        if !fact_ids.insert(fact.fact_id.as_str())
            || !schema_identifier(&fact.fact_id)
            || !canonical_relation(&fact.relation)
            || !node_ids.contains(fact.from_node_id.as_str())
            || !node_ids.contains(fact.to_node_id.as_str())
        {
            return Err(error(
                "evidence graph contains an invalid source fact or unknown endpoint",
            ));
        }
    }
    let mut gap_ids = BTreeSet::new();
    for gap in &input.gaps {
        if !gap_ids.insert(gap.gap_id.as_str())
            || !schema_identifier(&gap.gap_id)
            || gap.summary.trim().is_empty()
            || gap.owner_path.as_deref().is_some_and(invalid_project_path)
            || gap
                .severity
                .as_deref()
                .is_some_and(|severity| !matches!(severity, "info" | "warning" | "error"))
        {
            return Err(error("evidence graph contains an invalid or duplicate gap"));
        }
        validate_fields(&gap.fields)?;
    }
    Ok(())
}

fn validate_fields(fields: &BTreeMap<String, Value>) -> Result<(), EvidenceGraphBuildError> {
    if fields.values().all(|value| match value {
        Value::String(_) | Value::Number(_) | Value::Bool(_) => true,
        Value::Array(values) => values
            .iter()
            .all(|value| matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_))),
        Value::Null | Value::Object(_) => false,
    }) {
        Ok(())
    } else {
        Err(error(
            "evidence graph fields must contain schema-admitted scalar values",
        ))
    }
}

fn schema_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(first) if first.is_ascii_lowercase())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'.' | b':' | b'-')
        })
}

fn producer_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(first) if first.is_ascii_lowercase())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn producer_namespace(value: &str) -> bool {
    value.split('.').count() >= 2 && value.split('.').all(producer_identifier)
}

fn canonical_relation(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn qualified_blake3(value: &str) -> bool {
    value.strip_prefix("blake3-256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn invalid_project_path(path: &str) -> bool {
    path != "."
        && (path.is_empty()
            || path.starts_with('/')
            || path.contains(['\\', ':'])
            || path.split('/').any(|part| {
                part.is_empty()
                    || matches!(part, "." | "..")
                    || part.bytes().any(|byte| byte.is_ascii_whitespace())
            }))
}

pub(super) fn edge_kind_text(kind: EvidenceGraphEdgeKind) -> &'static str {
    match kind {
        EvidenceGraphEdgeKind::DerivedFrom => "derived-from",
        EvidenceGraphEdgeKind::RequiresEvidence => "requires-evidence",
        EvidenceGraphEdgeKind::VerifiedBy => "verified-by",
        EvidenceGraphEdgeKind::ObservedBy => "observed-by",
        EvidenceGraphEdgeKind::WaivedBy => "waived-by",
        EvidenceGraphEdgeKind::ReviewedBy => "reviewed-by",
        EvidenceGraphEdgeKind::SuggestsAction => "suggests-action",
        EvidenceGraphEdgeKind::SupportsClaim => "supports-claim",
    }
}

pub(super) fn parse_edge_kind(value: &str) -> Option<EvidenceGraphEdgeKind> {
    match value {
        "derived-from" => Some(EvidenceGraphEdgeKind::DerivedFrom),
        "requires-evidence" => Some(EvidenceGraphEdgeKind::RequiresEvidence),
        "verified-by" => Some(EvidenceGraphEdgeKind::VerifiedBy),
        "observed-by" => Some(EvidenceGraphEdgeKind::ObservedBy),
        "waived-by" => Some(EvidenceGraphEdgeKind::WaivedBy),
        "reviewed-by" => Some(EvidenceGraphEdgeKind::ReviewedBy),
        "suggests-action" => Some(EvidenceGraphEdgeKind::SuggestsAction),
        "supports-claim" => Some(EvidenceGraphEdgeKind::SupportsClaim),
        _ => None,
    }
}

pub(super) fn hash_serializable(value: &impl Serialize) -> Result<String, EvidenceGraphBuildError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|cause| error(format!("encode EvidenceGraph digest input: {cause}")))?;
    Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceGraphBuildError {
    message: String,
}

impl fmt::Display for EvidenceGraphBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for EvidenceGraphBuildError {}

pub(super) fn error(message: impl Into<String>) -> EvidenceGraphBuildError {
    EvidenceGraphBuildError {
        message: message.into(),
    }
}
