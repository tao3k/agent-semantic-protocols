// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Stable V1 normalized plan executed only against a resident generation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub const RESIDENT_SYNTAX_QUERY_PLAN_SCHEMA_ID: &str =
    "agent.semantic-protocols.resident-syntax-query-plan";
pub const ENHANCED_TREE_SITTER_QUERY_PROFILE_ID: &str = "asp.enhanced-tree-sitter-query.v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentSyntaxQueryPlan {
    pub schema_id: String,
    pub schema_version: String,
    pub profile_id: String,
    pub plan_digest: String,
    pub query_digest: String,
    pub language_id: String,
    pub provider_id: String,
    pub parser_abi_digest: String,
    pub query_grammar_digest: String,
    pub operator_table_digest: String,
    pub capability_table_digest: String,
    pub generation_digest: String,
    pub patterns: Vec<ResidentSyntaxQueryPattern>,
    pub selected_fields: Vec<ResidentSyntaxQueryResultField>,
    pub required_capability_rows: Vec<String>,
    pub regex_programs: Vec<ResidentRegexProgram>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentSyntaxQueryPattern {
    pub index: usize,
    pub captures: Vec<ResidentSyntaxQueryCapture>,
    pub structure: ResidentSyntaxQueryCondition,
    pub predicates: Vec<ResidentSyntaxQueryCondition>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentSyntaxQueryCapture {
    pub name: String,
    pub resident_fact_path: ResidentSyntaxQueryFactPath,
    pub cardinality: ResidentSyntaxQueryCardinality,
    pub capability_row_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentSyntaxQueryCardinality {
    pub minimum: usize,
    pub maximum: Option<usize>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ResidentSyntaxQueryFactPath {
    #[serde(rename = "kind")]
    Kind,
    #[serde(rename = "name")]
    Name,
    #[serde(rename = "selector")]
    Selector,
    #[serde(rename = "byte-range")]
    ByteRange,
    #[serde(rename = "scopes.relation")]
    ScopesRelation,
    #[serde(rename = "scopes.kind")]
    ScopesKind,
    #[serde(rename = "scopes.symbol")]
    ScopesSymbol,
    #[serde(rename = "queryKeys")]
    QueryKeys,
    #[serde(rename = "projections.kind")]
    ProjectionsKind,
    #[serde(rename = "relations")]
    Relations,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResidentSyntaxQueryOriginKind {
    Node,
    Field,
    Capture,
    Anchor,
    Quantifier,
    StandardPredicate,
    AspPredicate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentSyntaxQueryOrigin {
    pub kind: ResidentSyntaxQueryOriginKind,
    pub capability_row_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ResidentSyntaxQueryCondition {
    True {
        origin: ResidentSyntaxQueryOrigin,
    },
    All {
        terms: Vec<ResidentSyntaxQueryCondition>,
    },
    Any {
        terms: Vec<ResidentSyntaxQueryCondition>,
    },
    Scalar {
        capture: String,
        #[serde(rename = "factPath")]
        fact_path: ResidentSyntaxQueryFactPath,
        operator: ResidentSyntaxQueryScalarOperator,
        value: ResidentSyntaxQueryScalarValue,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "regexProgramId"
        )]
        regex_program_id: Option<String>,
        origin: ResidentSyntaxQueryOrigin,
    },
    Set {
        capture: String,
        #[serde(rename = "factPath")]
        fact_path: ResidentSyntaxQueryFactPath,
        operator: ResidentSyntaxQuerySetOperator,
        value: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "regexProgramId"
        )]
        regex_program_id: Option<String>,
        origin: ResidentSyntaxQueryOrigin,
    },
    Range {
        capture: String,
        mode: ResidentSyntaxQueryRangeMode,
        start: String,
        end: String,
        origin: ResidentSyntaxQueryOrigin,
    },
    Relation {
        capture: String,
        operator: ResidentSyntaxQueryRelationOperator,
        direction: ResidentSyntaxQueryDirection,
        #[serde(rename = "relationKind")]
        relation_kind: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "endpointSelector"
        )]
        endpoint_selector: Option<String>,
        origin: ResidentSyntaxQueryOrigin,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResidentSyntaxQueryScalarOperator {
    Eq,
    NotEq,
    Match,
    NotMatch,
    AnyEq,
    AnyNotEq,
    AnyMatch,
    AnyNotMatch,
    AnyOf,
    NotAnyOf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ResidentSyntaxQueryScalarValue {
    Literal(String),
    Literals(Vec<String>),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResidentSyntaxQuerySetOperator {
    AnyEq,
    NoneEq,
    AnyMatch,
    NoneMatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResidentSyntaxQueryRangeMode {
    Within,
    Contains,
    Overlaps,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResidentSyntaxQueryRelationOperator {
    Related,
    NotRelated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResidentSyntaxQueryDirection {
    In,
    Out,
    Either,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ResidentSyntaxQueryResultField {
    #[serde(rename = "kind")]
    Kind,
    #[serde(rename = "name")]
    Name,
    #[serde(rename = "selector")]
    Selector,
    #[serde(rename = "byte-range")]
    ByteRange,
    #[serde(rename = "scopes")]
    Scopes,
    #[serde(rename = "queryKeys")]
    QueryKeys,
    #[serde(rename = "projections")]
    Projections,
    #[serde(rename = "relations")]
    Relations,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentRegexProgram {
    pub id: String,
    pub engine_id: String,
    pub engine_version: String,
    pub syntax_profile: String,
    pub encoding: String,
    pub program: String,
    pub program_digest: String,
}

impl ResidentSyntaxQueryPlan {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != RESIDENT_SYNTAX_QUERY_PLAN_SCHEMA_ID
            || self.schema_version != "1"
            || self.profile_id != ENHANCED_TREE_SITTER_QUERY_PROFILE_ID
        {
            return Err("resident syntax Query plan identity mismatch".to_owned());
        }
        for (name, digest) in [
            ("planDigest", &self.plan_digest),
            ("queryDigest", &self.query_digest),
            ("parserAbiDigest", &self.parser_abi_digest),
            ("queryGrammarDigest", &self.query_grammar_digest),
            ("operatorTableDigest", &self.operator_table_digest),
            ("capabilityTableDigest", &self.capability_table_digest),
            ("generationDigest", &self.generation_digest),
        ] {
            validate_digest(name, digest)?;
        }
        if self.language_id.is_empty()
            || self.provider_id.is_empty()
            || self.patterns.is_empty()
            || self.selected_fields.is_empty()
            || self.required_capability_rows.is_empty()
        {
            return Err("resident syntax Query plan is incomplete".to_owned());
        }
        require_unique("selectedFields", self.selected_fields.iter().copied())?;
        require_unique(
            "requiredCapabilityRows",
            self.required_capability_rows.iter().map(String::as_str),
        )?;
        let required = self
            .required_capability_rows
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let regex_ids = self
            .regex_programs
            .iter()
            .map(|program| program.id.as_str())
            .collect::<BTreeSet<_>>();
        if regex_ids.len() != self.regex_programs.len() {
            return Err("resident syntax Query regex program ids are not unique".to_owned());
        }
        for program in &self.regex_programs {
            if program.engine_id.is_empty()
                || program.engine_version.is_empty()
                || program.syntax_profile != "tree-sitter-rust-regex-v1"
                || program.encoding != "base64"
            {
                return Err("resident syntax Query regex program identity mismatch".to_owned());
            }
            validate_digest("regexProgram.programDigest", &program.program_digest)?;
        }
        for (expected_index, pattern) in self.patterns.iter().enumerate() {
            if pattern.index != expected_index || pattern.captures.is_empty() {
                return Err("resident syntax Query pattern order is invalid".to_owned());
            }
            let captures = pattern
                .captures
                .iter()
                .map(|capture| capture.name.as_str())
                .collect::<BTreeSet<_>>();
            if captures.len() != pattern.captures.len() {
                return Err("resident syntax Query pattern captures are not unique".to_owned());
            }
            for capture in &pattern.captures {
                if capture.name.is_empty()
                    || capture
                        .cardinality
                        .maximum
                        .is_some_and(|maximum| maximum < capture.cardinality.minimum)
                    || !required.contains(capture.capability_row_id.as_str())
                {
                    return Err("resident syntax Query capture binding is invalid".to_owned());
                }
            }
            validate_condition(&pattern.structure, &captures, &required, &regex_ids)?;
            for predicate in &pattern.predicates {
                validate_condition(predicate, &captures, &required, &regex_ids)?;
            }
        }
        Ok(())
    }

    pub fn recompute_plan_digest(&self) -> Result<String, String> {
        let mut value = serde_json::to_value(self)
            .map_err(|error| format!("encode resident syntax Query plan: {error}"))?;
        value
            .as_object_mut()
            .expect("resident plan serializes as object")
            .remove("planDigest");
        let canonical = crate::canonical_json::to_jcs_vec(&value)
            .map_err(|error| format!("canonicalize resident syntax Query plan: {error}"))?;
        Ok(format!("blake3-256:{}", blake3::hash(&canonical).to_hex()))
    }
}

fn validate_condition(
    condition: &ResidentSyntaxQueryCondition,
    captures: &BTreeSet<&str>,
    required: &BTreeSet<&str>,
    regex_ids: &BTreeSet<&str>,
) -> Result<(), String> {
    match condition {
        ResidentSyntaxQueryCondition::All { terms }
        | ResidentSyntaxQueryCondition::Any { terms } => {
            if terms.is_empty() {
                return Err("resident syntax Query composite condition is empty".to_owned());
            }
            for term in terms {
                validate_condition(term, captures, required, regex_ids)?;
            }
        }
        ResidentSyntaxQueryCondition::True { origin } => validate_origin(origin, required)?,
        ResidentSyntaxQueryCondition::Scalar {
            capture,
            operator,
            value,
            regex_program_id,
            origin,
            ..
        } => {
            validate_capture(capture, captures)?;
            validate_origin(origin, required)?;
            match (operator, value) {
                (
                    ResidentSyntaxQueryScalarOperator::AnyOf
                    | ResidentSyntaxQueryScalarOperator::NotAnyOf,
                    ResidentSyntaxQueryScalarValue::Literals(values),
                ) if !values.is_empty() => {}
                (
                    ResidentSyntaxQueryScalarOperator::AnyOf
                    | ResidentSyntaxQueryScalarOperator::NotAnyOf,
                    _,
                ) => {
                    return Err(
                        "resident syntax Query any-of value must be a nonempty literal set"
                            .to_owned(),
                    );
                }
                (_, ResidentSyntaxQueryScalarValue::Literal(_)) => {}
                (_, ResidentSyntaxQueryScalarValue::Literals(_)) => {
                    return Err(
                        "resident syntax Query scalar operator requires one literal".to_owned()
                    );
                }
            }
            validate_regex_reference(
                matches!(
                    operator,
                    ResidentSyntaxQueryScalarOperator::Match
                        | ResidentSyntaxQueryScalarOperator::NotMatch
                        | ResidentSyntaxQueryScalarOperator::AnyMatch
                        | ResidentSyntaxQueryScalarOperator::AnyNotMatch
                ),
                regex_program_id.as_deref(),
                regex_ids,
            )?;
        }
        ResidentSyntaxQueryCondition::Set {
            capture,
            operator,
            regex_program_id,
            origin,
            ..
        } => {
            validate_capture(capture, captures)?;
            validate_origin(origin, required)?;
            validate_regex_reference(
                matches!(
                    operator,
                    ResidentSyntaxQuerySetOperator::AnyMatch
                        | ResidentSyntaxQuerySetOperator::NoneMatch
                ),
                regex_program_id.as_deref(),
                regex_ids,
            )?;
        }
        ResidentSyntaxQueryCondition::Range {
            capture,
            start,
            end,
            origin,
            ..
        } => {
            validate_capture(capture, captures)?;
            validate_origin(origin, required)?;
            let start = canonical_u64(start)?;
            let end = canonical_u64(end)?;
            if start > end {
                return Err("resident syntax Query range is reversed".to_owned());
            }
        }
        ResidentSyntaxQueryCondition::Relation {
            capture,
            relation_kind,
            origin,
            ..
        } => {
            validate_capture(capture, captures)?;
            validate_origin(origin, required)?;
            if relation_kind.is_empty() {
                return Err("resident syntax Query relation kind is empty".to_owned());
            }
        }
    }
    Ok(())
}

fn validate_regex_reference(
    required: bool,
    regex_program_id: Option<&str>,
    regex_ids: &BTreeSet<&str>,
) -> Result<(), String> {
    match (required, regex_program_id) {
        (true, Some(id)) if regex_ids.contains(id) => Ok(()),
        (true, _) => Err("resident syntax Query regex program reference is missing".to_owned()),
        (false, None) => Ok(()),
        (false, Some(_)) => {
            Err("resident syntax Query non-regex operator carries a regex program".to_owned())
        }
    }
}

fn validate_capture(capture: &str, captures: &BTreeSet<&str>) -> Result<(), String> {
    if !captures.contains(capture) {
        return Err("resident syntax Query condition capture is unbound".to_owned());
    }
    Ok(())
}

fn validate_origin(
    origin: &ResidentSyntaxQueryOrigin,
    required: &BTreeSet<&str>,
) -> Result<(), String> {
    if !required.contains(origin.capability_row_id.as_str()) {
        return Err("resident syntax Query condition capability row is missing".to_owned());
    }
    Ok(())
}

fn canonical_u64(value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| "resident syntax Query range bound is invalid".to_owned())?;
    if parsed.to_string() != value {
        return Err("resident syntax Query range bound is not canonical".to_owned());
    }
    Ok(parsed)
}

fn validate_digest(name: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("blake3-256:") else {
        return Err(format!("resident syntax Query {name} is not blake3-256"));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("resident syntax Query {name} is malformed"));
    }
    Ok(())
}

fn require_unique<T: Ord>(name: &str, values: impl Iterator<Item = T>) -> Result<(), String> {
    let values = values.collect::<Vec<_>>();
    if values.iter().collect::<BTreeSet<_>>().len() != values.len() {
        return Err(format!("resident syntax Query {name} is not unique"));
    }
    Ok(())
}
