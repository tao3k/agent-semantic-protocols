// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Source-owned relation expectations and their frontier classification.

use serde_json::{Value, json};

use crate::project_topology_generation_error::{
    ProjectTopologyGenerationBuildError, error, validate_identifier,
};

/// Coverage authority attached to one source-owned expected relationship.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectTopologyRelationCoverage {
    None,
    Partial,
    Complete,
}

/// One expected relation. It is an obligation input, never a positive edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyExpectedRelation {
    pub(crate) id: String,
    pub(crate) anchor: String,
    pub(crate) relation: String,
    pub(crate) target: String,
    pub(crate) target_kind: String,
    pub(crate) depth: u64,
    pub(crate) coverage: ProjectTopologyRelationCoverage,
}

impl ProjectTopologyExpectedRelation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        anchor: impl Into<String>,
        relation: impl Into<String>,
        target: impl Into<String>,
        target_kind: impl Into<String>,
        depth: u64,
        coverage: ProjectTopologyRelationCoverage,
    ) -> Result<Self, ProjectTopologyGenerationBuildError> {
        let value = Self {
            id: id.into(),
            anchor: anchor.into(),
            relation: relation.into(),
            target: target.into(),
            target_kind: target_kind.into(),
            depth,
            coverage,
        };
        validate_identifier(&value.id, "expected relation identity")?;
        for (field, text) in [
            ("expected relation anchor", value.anchor.as_str()),
            ("expected relation target", value.target.as_str()),
            ("expected relation target kind", value.target_kind.as_str()),
        ] {
            validate_identifier(text, field)?;
        }
        if value.relation.is_empty()
            || !value
                .relation
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(error(
                "topology-expected-relation-invalid",
                "expected relation name must be canonical uppercase vocabulary",
            ));
        }
        Ok(value)
    }
}

pub(crate) struct ProjectTopologyFrontierProjection {
    pub(crate) expectation_records: Vec<Value>,
    pub(crate) expectation_digest: String,
    pub(crate) coverage_certificates: Vec<Value>,
    pub(crate) frontiers: Vec<Value>,
}

pub(crate) fn project_topology_frontier_projection(
    expected_relations: &[ProjectTopologyExpectedRelation],
    edge_records: &[Value],
    source_generation_digest: &str,
) -> ProjectTopologyFrontierProjection {
    let expectation_records = expected_relations
        .iter()
        .map(|expected| {
            json!({
                "id": expected.id,
                "anchor": expected.anchor,
                "relation": expected.relation,
                "target": expected.target,
                "targetKind": expected.target_kind,
                "depth": expected.depth,
                "coverage": coverage_name(expected.coverage),
            })
        })
        .collect::<Vec<_>>();
    let expectation_digest = digest_json(&Value::Array(expectation_records.clone()));
    let mut coverage_certificates = Vec::new();
    let mut frontiers = Vec::new();
    for expected in expected_relations {
        let has_positive = edge_records.iter().any(|edge| {
            edge["from"] == expected.anchor
                && edge["to"] == expected.target
                && edge["relation"] == expected.relation
                && edge["modality"] != "proposed"
        });
        if has_positive {
            continue;
        }
        let coverage_ref = if expected.coverage == ProjectTopologyRelationCoverage::None {
            None
        } else {
            let id = format!("coverage-{}", expected.id);
            let scope = coverage_name(expected.coverage);
            coverage_certificates.push(json!({
                "id": id,
                "relation": expected.relation,
                "targetKind": expected.target_kind,
                "scope": scope,
                "digest": digest_json(&json!({
                    "expectation": expected.id,
                    "relation": expected.relation,
                    "targetKind": expected.target_kind,
                    "scope": scope,
                    "sourceGenerationDigest": source_generation_digest,
                }))
            }));
            Some(id)
        };
        let mut frontier = json!({
            "anchor": expected.anchor,
            "target": expected.target,
            "relation": expected.relation,
            "targetKind": expected.target_kind,
            "depth": expected.depth,
            "state": if expected.coverage == ProjectTopologyRelationCoverage::Complete {
                "certified-missing"
            } else {
                "unknown"
            },
            "reason": match expected.coverage {
                ProjectTopologyRelationCoverage::None => "binding-not-established",
                ProjectTopologyRelationCoverage::Partial => "coverage-open",
                ProjectTopologyRelationCoverage::Complete => "complete-coverage-no-witness",
            }
        });
        if let Some(coverage_ref) = coverage_ref {
            frontier["coverageRef"] = Value::String(coverage_ref);
        }
        frontiers.push(frontier);
    }
    ProjectTopologyFrontierProjection {
        expectation_records,
        expectation_digest,
        coverage_certificates,
        frontiers,
    }
}

const fn coverage_name(coverage: ProjectTopologyRelationCoverage) -> &'static str {
    match coverage {
        ProjectTopologyRelationCoverage::None => "none",
        ProjectTopologyRelationCoverage::Partial => "partial",
        ProjectTopologyRelationCoverage::Complete => "complete",
    }
}

fn digest_json(value: &Value) -> String {
    let serialized = serde_json::to_string(value).expect("JSON value serializes");
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(serialized.len() as u64).to_be_bytes());
    hasher.update(serialized.as_bytes());
    format!("blake3-256:{}", hasher.finalize().to_hex())
}
