// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Orgize projection for the Git-tracked Project Topology program source.

use std::collections::BTreeMap;
use std::fmt;

use agent_semantic_content_identity::CanonicalItemSelector;

use crate::{
    OrgDocumentProjection, OrgDocumentProjectionPort, OrgPropertyEntry,
    ProjectTopologyInferenceProgram, ProjectTopologyRelationCoverage,
};

pub const PROJECT_TOPOLOGY_STANDARD_PROGRAM_RESOURCE: &str =
    "org/templates/asp/topology/program.v1.org";

const CONTRACT_NAME: &str = "program.v1.org";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologySourceRule {
    rule_id: String,
    head_relation: String,
    left_relation: String,
    right_relation: String,
    join: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologySourceExpectation {
    id: String,
    anchor_selector: String,
    target_selector: String,
    relation: String,
    target_kind: String,
    depth: u64,
    coverage: ProjectTopologyRelationCoverage,
}

impl ProjectTopologySourceExpectation {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn anchor_selector(&self) -> &str {
        &self.anchor_selector
    }

    pub fn target_selector(&self) -> &str {
        &self.target_selector
    }

    pub fn relation(&self) -> &str {
        &self.relation
    }

    pub fn target_kind(&self) -> &str {
        &self.target_kind
    }

    pub const fn depth(&self) -> u64 {
        self.depth
    }

    pub const fn coverage(&self) -> ProjectTopologyRelationCoverage {
        self.coverage
    }
}

/// Canonical typed projection of one Org topology source program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologySourceProgram {
    program_id: String,
    engine_profile: String,
    rules: Vec<ProjectTopologySourceRule>,
    expectations: Vec<ProjectTopologySourceExpectation>,
    source_digest: String,
}

impl ProjectTopologySourceProgram {
    /// Load the standard topology program through a host-supplied [`OrgDocumentProjectionPort`].
    pub fn standard_with_port(
        port: &dyn OrgDocumentProjectionPort,
    ) -> Result<Self, ProjectTopologySourceProgramError> {
        Self::parse_with_port(
            include_str!("../../../org/templates/asp/topology/program.v1.org"),
            port,
        )
    }

    /// Parse raw Org program source through a host-supplied [`OrgDocumentProjectionPort`].
    pub fn parse_with_port(
        source: &str,
        port: &dyn OrgDocumentProjectionPort,
    ) -> Result<Self, ProjectTopologySourceProgramError> {
        let projection = port
            .project_document(source)
            .map_err(|cause| error(cause.reason_kind, cause.message))?;
        Self::from_org_projection(&projection)
    }

    /// Admit one canonical Org topology source program from a pure-Rust [`OrgDocumentProjection`].
    pub fn from_org_projection(
        projection: &OrgDocumentProjection,
    ) -> Result<Self, ProjectTopologySourceProgramError> {
        let document_properties = unique_properties(&projection.document_properties)?;
        let contract = document_properties
            .get("CONTRACT_ORG")
            .map(String::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                error(
                    "topology-source-program-contract-mismatch",
                    "expected one document CONTRACT_ORG declaration, observed 0",
                )
            })?;
        if !canonical_contract_reference(contract) {
            return invalid(
                "topology-source-program-contract-mismatch",
                "source program is not governed by asp.topology.program.v1",
            );
        }

        let programs = projection
            .sections
            .iter()
            .enumerate()
            .filter(|(_, section)| {
                section.level == 1
                    && section.parent_index.is_none()
                    && section
                        .properties
                        .iter()
                        .any(|property| property.key == "TOPOLOGY_PROGRAM_ID")
            })
            .collect::<Vec<_>>();
        let [(program_index, program_section)] = programs.as_slice() else {
            return invalid(
                "topology-source-program-ambiguous",
                format!(
                    "expected one program declaration, observed {}",
                    programs.len()
                ),
            );
        };
        let program_index = *program_index;
        let program = unique_properties(&program_section.properties)?;
        let program_id = required_property(&program, "TOPOLOGY_PROGRAM_ID")?.to_owned();
        if program_id != "mrr.topology.standard.v1" {
            return invalid(
                "topology-source-program-identity-unsupported",
                "V1 admits only mrr.topology.standard.v1",
            );
        }
        let engine_profile = required_property(&program, "ENGINE_PROFILE")?.to_owned();
        if engine_profile != "ascent-seminaive.v1" {
            return invalid(
                "topology-source-program-engine-unsupported",
                "V1 topology source requires ascent-seminaive.v1",
            );
        }

        let mut rules = projection
            .sections
            .iter()
            .enumerate()
            .filter(|(index, section)| {
                *index != program_index
                    && projection.is_descendant_of(*index, program_index)
                    && section
                        .properties
                        .iter()
                        .any(|property| property.key == "RULE_ID")
            })
            .map(|(_, section)| {
                let fields = unique_properties(&section.properties)?;
                Ok(ProjectTopologySourceRule {
                    rule_id: required_property(&fields, "RULE_ID")?.to_owned(),
                    head_relation: canonical_relation(&fields, "HEAD_RELATION")?,
                    left_relation: canonical_relation(&fields, "LEFT_RELATION")?,
                    right_relation: canonical_relation(&fields, "RIGHT_RELATION")?,
                    join: required_property(&fields, "JOIN")?.to_owned(),
                })
            })
            .collect::<Result<Vec<_>, ProjectTopologySourceProgramError>>()?;
        rules.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
        if rules.len() != 1
            || rules[0].rule_id != "mrr.topology.domain.depends-on-config.v1"
            || rules[0].head_relation != "DEPENDS_ON_CONFIG"
            || rules[0].left_relation != "CALLS"
            || rules[0].right_relation != "READS_CONFIG"
            || rules[0].join != "left.to=right.from"
        {
            return invalid(
                "topology-source-program-rule-unsupported",
                "V1 admits exactly CALLS(x,y) + READS_CONFIG(y,z) => DEPENDS_ON_CONFIG(x,z)",
            );
        }

        let mut expectations = projection
            .sections
            .iter()
            .enumerate()
            .filter(|(index, section)| {
                *index != program_index
                    && projection.is_descendant_of(*index, program_index)
                    && section
                        .properties
                        .iter()
                        .any(|property| property.key == "EXPECTATION_ID")
            })
            .map(|(_, section)| {
                let fields = unique_properties(&section.properties)?;
                parse_expectation(&fields)
            })
            .collect::<Result<Vec<_>, _>>()?;
        expectations.sort_by(|left, right| left.id.cmp(&right.id));
        if expectations.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return invalid(
                "topology-source-program-expectations-invalid",
                "expected relations must have unique identities",
            );
        }
        Ok(Self {
            program_id,
            engine_profile,
            rules,
            expectations,
            source_digest: projection.source_digest.clone(),
        })
    }

    pub fn inference_program(
        &self,
    ) -> Result<ProjectTopologyInferenceProgram, ProjectTopologySourceProgramError> {
        ProjectTopologyInferenceProgram::standard().map_err(|cause| {
            error(
                "topology-source-program-mrr-admission-failed",
                cause.to_string(),
            )
        })
    }

    pub fn expectations(&self) -> &[ProjectTopologySourceExpectation] {
        &self.expectations
    }

    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }

    pub fn program_id(&self) -> &str {
        &self.program_id
    }

    pub fn engine_profile(&self) -> &str {
        &self.engine_profile
    }

    pub fn rules(&self) -> &[ProjectTopologySourceRule] {
        &self.rules
    }
}

fn parse_expectation(
    fields: &BTreeMap<String, String>,
) -> Result<ProjectTopologySourceExpectation, ProjectTopologySourceProgramError> {
    let anchor_selector = required_property(fields, "ANCHOR_SELECTOR")?;
    CanonicalItemSelector::parse(anchor_selector).map_err(|cause| {
        error(
            "topology-source-program-selector-invalid",
            format!("invalid anchor selector: {cause}"),
        )
    })?;
    let target_selector = required_property(fields, "TARGET_SELECTOR")?;
    let target = CanonicalItemSelector::parse(target_selector).map_err(|cause| {
        error(
            "topology-source-program-selector-invalid",
            format!("invalid target selector: {cause}"),
        )
    })?;
    let target_kind = required_property(fields, "TARGET_KIND")?;
    if target.kind.as_str() != target_kind {
        return invalid(
            "topology-source-program-target-kind-mismatch",
            "TARGET_KIND must equal the native target selector kind",
        );
    }
    let depth = required_property(fields, "DEPTH")?
        .parse::<u64>()
        .map_err(|_| error("topology-source-program-depth-invalid", "DEPTH must be u64"))?;
    let coverage = match required_property(fields, "COVERAGE")? {
        "none" => ProjectTopologyRelationCoverage::None,
        "partial" => ProjectTopologyRelationCoverage::Partial,
        "complete" => ProjectTopologyRelationCoverage::Complete,
        _ => {
            return invalid(
                "topology-source-program-coverage-invalid",
                "COVERAGE must be none, partial, or complete",
            );
        }
    };
    Ok(ProjectTopologySourceExpectation {
        id: required_property(fields, "EXPECTATION_ID")?.to_owned(),
        anchor_selector: anchor_selector.to_owned(),
        target_selector: target_selector.to_owned(),
        relation: canonical_relation(fields, "EXPECTED_RELATION")?,
        target_kind: target_kind.to_owned(),
        depth,
        coverage,
    })
}

fn canonical_relation(
    fields: &BTreeMap<String, String>,
    key: &str,
) -> Result<String, ProjectTopologySourceProgramError> {
    let value = required_property(fields, key)?;
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return invalid(
            "topology-source-program-relation-invalid",
            format!("{key} must be canonical uppercase vocabulary"),
        );
    }
    Ok(value.to_owned())
}

fn unique_properties(
    properties: &[OrgPropertyEntry],
) -> Result<BTreeMap<String, String>, ProjectTopologySourceProgramError> {
    let mut values = BTreeMap::new();
    for property in properties {
        if values
            .insert(property.key.clone(), property.value.clone())
            .is_some()
        {
            return invalid(
                "topology-source-program-property-duplicate",
                format!("property {} is declared more than once", property.key),
            );
        }
    }
    Ok(values)
}

fn required_property<'a>(
    properties: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, ProjectTopologySourceProgramError> {
    properties
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            error(
                "topology-source-program-property-missing",
                format!("property {key} is required"),
            )
        })
}

fn canonical_contract_reference(value: &str) -> bool {
    value
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
        .and_then(|link| link.split_once("]["))
        .is_some_and(|(target, label)| {
            label == "asp.topology.program.v1"
                && (target == CONTRACT_NAME || target.ends_with(&format!("/{CONTRACT_NAME}")))
                && !target.starts_with('/')
                && !target.contains('\\')
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologySourceProgramError {
    reason_kind: &'static str,
    message: String,
}

impl ProjectTopologySourceProgramError {
    pub const fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for ProjectTopologySourceProgramError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for ProjectTopologySourceProgramError {}

fn error(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> ProjectTopologySourceProgramError {
    ProjectTopologySourceProgramError {
        reason_kind,
        message: message.into(),
    }
}

fn invalid<T>(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> Result<T, ProjectTopologySourceProgramError> {
    Err(error(reason_kind, message))
}
