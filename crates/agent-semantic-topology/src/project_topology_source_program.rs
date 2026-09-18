// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Orgize projection for the Git-tracked Project Topology program source.

use std::collections::BTreeMap;
use std::fmt;

use agent_semantic_content_identity::CanonicalItemSelector;
use orgize::Org;
use orgize::ast::{
    OrgElementGraph, OrgElementId, OrgElementsIndexCategory, OrgElementsIndexQuery,
    OrgElementsIndexRecord, OrgElementsIndexSummaryValue, ParsedAnnotation,
};

use crate::{ProjectTopologyInferenceProgram, ProjectTopologyRelationCoverage};

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
    pub fn standard() -> Result<Self, ProjectTopologySourceProgramError> {
        Self::parse_org(include_str!(
            "../../../org/templates/asp/topology/program.v1.org"
        ))
    }

    pub fn parse_org(source: &str) -> Result<Self, ProjectTopologySourceProgramError> {
        let document = Org::parse(source).document();
        let graph = document.org_elements_graph();
        let contract_drawers = declaration_drawers(&graph, "CONTRACT_ORG", None, Some(0))
            .into_iter()
            .filter(|drawer| {
                drawer.context == "document"
                    && graph
                        .parent(drawer.id)
                        .is_some_and(|owner| owner.id == graph.root_id)
            })
            .collect::<Vec<_>>();
        let [contract_drawer] = contract_drawers.as_slice() else {
            return invalid(
                "topology-source-program-contract-mismatch",
                format!(
                    "expected one document CONTRACT_ORG declaration, observed {}",
                    contract_drawers.len()
                ),
            );
        };
        let document_properties = unique_indexed_properties(&graph, contract_drawer.id)?;
        let contract = required_property(&document_properties, "CONTRACT_ORG")?;
        if !canonical_contract_reference(contract) {
            return invalid(
                "topology-source-program-contract-mismatch",
                "source program is not governed by asp.topology.program.v1",
            );
        }

        let programs = declaration_drawers(&graph, "TOPOLOGY_PROGRAM_ID", None, Some(1))
            .into_iter()
            .filter_map(|drawer| {
                let owner = graph.parent(drawer.id)?;
                (drawer.context == "headline"
                    && owner.category == OrgElementsIndexCategory::Section
                    && owner.parent_id == Some(graph.root_id))
                .then_some((owner, drawer))
            })
            .collect::<Vec<_>>();
        let [(program_owner, program_drawer)] = programs.as_slice() else {
            return invalid(
                "topology-source-program-ambiguous",
                format!(
                    "expected one program declaration, observed {}",
                    programs.len()
                ),
            );
        };
        let program = unique_indexed_properties(&graph, program_drawer.id)?;
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

        let mut rules = declaration_drawers(&graph, "RULE_ID", Some(program_owner.id), None)
            .into_iter()
            .filter(|drawer| {
                declaration_owner_is_headline(&graph, drawer)
                    && graph
                        .parent(drawer.id)
                        .is_some_and(|owner| owner.id != program_owner.id)
            })
            .map(|drawer| {
                let fields = unique_indexed_properties(&graph, drawer.id)?;
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

        let mut expectations =
            declaration_drawers(&graph, "EXPECTATION_ID", Some(program_owner.id), None)
                .into_iter()
                .filter(|drawer| {
                    declaration_owner_is_headline(&graph, drawer)
                        && graph
                            .parent(drawer.id)
                            .is_some_and(|owner| owner.id != program_owner.id)
                })
                .map(|drawer| {
                    let fields = unique_indexed_properties(&graph, drawer.id)?;
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
        let source_digest = format!("blake3-256:{}", blake3::hash(source.as_bytes()).to_hex());
        Ok(Self {
            program_id,
            engine_profile,
            rules,
            expectations,
            source_digest,
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

fn declaration_drawers<'a>(
    graph: &'a OrgElementGraph<ParsedAnnotation>,
    property_key: &str,
    ancestor: Option<OrgElementId>,
    outline_path_exact_len: Option<usize>,
) -> Vec<&'a OrgElementsIndexRecord<ParsedAnnotation>> {
    let mut query = OrgElementsIndexQuery::new()
        .category(OrgElementsIndexCategory::Property)
        .kind("node-property")
        .context("propertyDrawer")
        .summary_eq("key", property_key);
    if let Some(ancestor) = ancestor {
        query = query.descendant_of(ancestor);
    }
    if let Some(outline_path_exact_len) = outline_path_exact_len {
        query = query.outline_path_exact_len(outline_path_exact_len);
    }
    let mut drawers = graph
        .query(&query)
        .into_iter()
        .filter_map(|property| graph.parent(property.id))
        .collect::<Vec<_>>();
    drawers.sort_by_key(|drawer| drawer.id);
    drawers.dedup_by_key(|drawer| drawer.id);
    drawers
}

fn declaration_owner_is_headline(
    graph: &OrgElementGraph<ParsedAnnotation>,
    drawer: &OrgElementsIndexRecord<ParsedAnnotation>,
) -> bool {
    drawer.context == "headline"
        && graph
            .parent(drawer.id)
            .is_some_and(|owner| owner.category == OrgElementsIndexCategory::Section)
}

fn unique_indexed_properties(
    graph: &OrgElementGraph<ParsedAnnotation>,
    drawer_id: OrgElementId,
) -> Result<BTreeMap<String, String>, ProjectTopologySourceProgramError> {
    let mut values = BTreeMap::new();
    for property in graph.query(
        &OrgElementsIndexQuery::new()
            .category(OrgElementsIndexCategory::Property)
            .kind("node-property")
            .context("propertyDrawer")
            .child_of(drawer_id),
    ) {
        let key = indexed_summary_text(property, "key")?;
        let value = indexed_summary_text(property, "value")?;
        if values.insert(key.to_owned(), value.to_owned()).is_some() {
            return invalid(
                "topology-source-program-property-duplicate",
                format!("property {key} is declared more than once"),
            );
        }
    }
    Ok(values)
}

fn indexed_summary_text<'a>(
    record: &'a OrgElementsIndexRecord<ParsedAnnotation>,
    key: &str,
) -> Result<&'a str, ProjectTopologySourceProgramError> {
    match record.summary.get(key) {
        Some(OrgElementsIndexSummaryValue::Text(value)) => Ok(value),
        _ => invalid(
            "topology-source-program-index-invalid",
            format!("indexed node-property has no text {key}"),
        ),
    }
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
