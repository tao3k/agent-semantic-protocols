// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Client-side source-plane lowering into a generation-bound resident Query plan.

use std::collections::{BTreeMap, BTreeSet};

use agent_semantic_client_protocol::{
    EnhancedQueryCapabilityRowKind, EnhancedQueryCapabilityTable, EnhancedQueryConstraintKind,
    ResidentRegexProgram, ResidentSyntaxQueryCapture, ResidentSyntaxQueryCardinality,
    ResidentSyntaxQueryCondition, ResidentSyntaxQueryDirection, ResidentSyntaxQueryOrigin,
    ResidentSyntaxQueryOriginKind, ResidentSyntaxQueryPattern, ResidentSyntaxQueryPlan,
    ResidentSyntaxQueryRangeMode, ResidentSyntaxQueryRelationOperator,
    ResidentSyntaxQueryResultField, ResidentSyntaxQueryScalarOperator,
    ResidentSyntaxQueryScalarValue, ResidentSyntaxQuerySetOperator,
};
use base64::Engine;

use crate::{
    EnhancedQueryExpression, EnhancedQueryOperand, EnhancedQueryPattern, EnhancedQueryPredicate,
    EnhancedQueryPredicateKind, EnhancedQueryQuantifier, parse_enhanced_query_source,
};

pub fn compile_resident_syntax_plan(
    query_source: &str,
    generation_digest: &str,
    capability: &EnhancedQueryCapabilityTable,
) -> Result<ResidentSyntaxQueryPlan, String> {
    capability.validate()?;
    let document = parse_enhanced_query_source(query_source).map_err(|error| error.message)?;
    let mut required_rows = BTreeSet::new();
    let mut regex_programs = BTreeMap::<String, ResidentRegexProgram>::new();
    let mut selected_fields = BTreeSet::new();
    let patterns = document
        .patterns
        .iter()
        .map(|pattern| {
            lower_pattern(
                pattern,
                capability,
                &mut required_rows,
                &mut regex_programs,
                &mut selected_fields,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if selected_fields.is_empty() {
        admit_selected_field(
            ResidentSyntaxQueryResultField::Selector,
            capability,
            &mut required_rows,
            &mut selected_fields,
        )?;
    }
    let query_digest = digest_bytes(query_source.as_bytes());
    let mut plan = ResidentSyntaxQueryPlan {
        schema_id: agent_semantic_client_protocol::RESIDENT_SYNTAX_QUERY_PLAN_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        profile_id: agent_semantic_client_protocol::ENHANCED_TREE_SITTER_QUERY_PROFILE_ID
            .to_owned(),
        plan_digest: digest_bytes(b"pending"),
        query_digest,
        language_id: capability.language_id.clone(),
        provider_id: capability.provider_id.clone(),
        parser_abi_digest: capability.parser_abi.digest.clone(),
        query_grammar_digest: capability.query_grammar.digest.clone(),
        operator_table_digest: capability.operator_table_digest.clone(),
        capability_table_digest: capability.table_digest.clone(),
        generation_digest: generation_digest.to_owned(),
        patterns,
        selected_fields: selected_fields.into_iter().collect(),
        required_capability_rows: required_rows.into_iter().collect(),
        regex_programs: regex_programs.into_values().collect(),
    };
    plan.plan_digest = plan.recompute_plan_digest()?;
    plan.validate()?;
    Ok(plan)
}

fn lower_pattern(
    pattern: &EnhancedQueryPattern,
    capability: &EnhancedQueryCapabilityTable,
    required_rows: &mut BTreeSet<String>,
    regex_programs: &mut BTreeMap<String, ResidentRegexProgram>,
    selected_fields: &mut BTreeSet<ResidentSyntaxQueryResultField>,
) -> Result<ResidentSyntaxQueryPattern, String> {
    let mut captures = BTreeMap::<String, ResidentSyntaxQueryCapture>::new();
    let structure = lower_structure(&pattern.structure, capability, required_rows, &mut captures)?;
    let mut predicates = Vec::new();
    for predicate in &pattern.predicates {
        if predicate.name == "asp-select" {
            lower_select_directive(
                predicate,
                capability,
                required_rows,
                &captures,
                selected_fields,
            )?;
        } else {
            predicates.push(lower_predicate(
                predicate,
                capability,
                required_rows,
                &captures,
                regex_programs,
            )?);
        }
    }
    Ok(ResidentSyntaxQueryPattern {
        index: pattern.index,
        captures: captures.into_values().collect(),
        structure,
        predicates,
    })
}

fn lower_structure(
    expression: &EnhancedQueryExpression,
    capability: &EnhancedQueryCapabilityTable,
    required_rows: &mut BTreeSet<String>,
    captures: &mut BTreeMap<String, ResidentSyntaxQueryCapture>,
) -> Result<ResidentSyntaxQueryCondition, String> {
    match expression {
        EnhancedQueryExpression::NamedNode {
            name,
            quantifier,
            captures: node_captures,
            children,
        } => {
            if node_captures.len() != 1 {
                return Err(
                    "enhanced-query-cardinality-invalid resident node requires one item capture"
                        .to_owned(),
                );
            }
            if !children.is_empty() {
                return Err(
                    "enhanced-query-fact-not-resident nested node/field structure is not materialized"
                        .to_owned(),
                );
            }
            let capture_name = &node_captures[0];
            let capture_row = capability
                .runtime_row(EnhancedQueryCapabilityRowKind::CaptureBinding, capture_name)?;
            let capture_lowering = capture_row
                .lowering
                .as_ref()
                .expect("validated runtime capability row has lowering");
            if capture_lowering.constraint_kind != EnhancedQueryConstraintKind::Capture {
                return Err("enhanced Query capture row lowering kind mismatch".to_owned());
            }
            required_rows.insert(capture_row.row_id.clone());
            insert_capture(
                captures,
                capture_name,
                capture_lowering.resident_fact_path,
                *quantifier,
                &capture_row.row_id,
            )?;
            if name == "_" {
                return Ok(ResidentSyntaxQueryCondition::True {
                    origin: origin(
                        ResidentSyntaxQueryOriginKind::Capture,
                        capture_row,
                        required_rows,
                    ),
                });
            }
            let node_row =
                capability.runtime_row(EnhancedQueryCapabilityRowKind::NodeType, name)?;
            let lowering = node_row
                .lowering
                .as_ref()
                .expect("validated runtime capability row has lowering");
            let value = lowering.resident_value.clone().ok_or_else(|| {
                format!(
                    "enhanced Query node row lacks resident value: {}",
                    node_row.row_id
                )
            })?;
            Ok(ResidentSyntaxQueryCondition::Scalar {
                capture: capture_name.clone(),
                fact_path: lowering.resident_fact_path,
                operator: ResidentSyntaxQueryScalarOperator::Eq,
                value: ResidentSyntaxQueryScalarValue::Literal(value),
                regex_program_id: None,
                origin: origin(ResidentSyntaxQueryOriginKind::Node, node_row, required_rows),
            })
        }
        EnhancedQueryExpression::Alternation {
            quantifier,
            captures: alternation_captures,
            alternatives,
        } => {
            if *quantifier != EnhancedQueryQuantifier::ONE || !alternation_captures.is_empty() {
                return Err(
                    "enhanced-query-cardinality-invalid resident alternation-level quantifier/capture is unsupported"
                        .to_owned(),
                );
            }
            let terms = alternatives
                .iter()
                .map(|alternative| {
                    lower_structure(alternative, capability, required_rows, captures)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ResidentSyntaxQueryCondition::Any { terms })
        }
        EnhancedQueryExpression::Group {
            quantifier,
            captures: group_captures,
            terms,
        } => {
            if *quantifier != EnhancedQueryQuantifier::ONE
                || !group_captures.is_empty()
                || terms.len() != 1
            {
                return Err(
                    "enhanced-query-fact-not-resident resident group must contain one unquantified captured item"
                        .to_owned(),
                );
            }
            lower_structure(&terms[0], capability, required_rows, captures)
        }
        EnhancedQueryExpression::AnonymousNode { .. }
        | EnhancedQueryExpression::Field { .. }
        | EnhancedQueryExpression::NegatedField(_)
        | EnhancedQueryExpression::Anchor => Err(
            "enhanced-query-fact-not-resident structural form has no resident capability lowering"
                .to_owned(),
        ),
    }
}

fn insert_capture(
    captures: &mut BTreeMap<String, ResidentSyntaxQueryCapture>,
    name: &str,
    resident_fact_path: agent_semantic_client_protocol::ResidentSyntaxQueryFactPath,
    quantifier: EnhancedQueryQuantifier,
    capability_row_id: &str,
) -> Result<(), String> {
    let capture = ResidentSyntaxQueryCapture {
        name: name.to_owned(),
        resident_fact_path,
        cardinality: ResidentSyntaxQueryCardinality {
            minimum: quantifier.minimum,
            maximum: quantifier.maximum,
        },
        capability_row_id: capability_row_id.to_owned(),
    };
    if let Some(previous) = captures.insert(name.to_owned(), capture.clone())
        && previous != capture
    {
        return Err(format!(
            "enhanced-query-cardinality-invalid capture `{name}` has conflicting bindings"
        ));
    }
    Ok(())
}

fn lower_predicate(
    predicate: &EnhancedQueryPredicate,
    capability: &EnhancedQueryCapabilityTable,
    required_rows: &mut BTreeSet<String>,
    captures: &BTreeMap<String, ResidentSyntaxQueryCapture>,
    regex_programs: &mut BTreeMap<String, ResidentRegexProgram>,
) -> Result<ResidentSyntaxQueryCondition, String> {
    if predicate.kind != EnhancedQueryPredicateKind::Predicate {
        return Err(format!(
            "enhanced-query-operator-unsupported directive #{}!",
            predicate.name
        ));
    }
    if predicate.name.starts_with("asp-") {
        lower_asp_predicate(
            predicate,
            capability,
            required_rows,
            captures,
            regex_programs,
        )
    } else {
        lower_standard_predicate(predicate, required_rows, captures, regex_programs)
    }
}

fn lower_standard_predicate(
    predicate: &EnhancedQueryPredicate,
    required_rows: &mut BTreeSet<String>,
    captures: &BTreeMap<String, ResidentSyntaxQueryCapture>,
    regex_programs: &mut BTreeMap<String, ResidentRegexProgram>,
) -> Result<ResidentSyntaxQueryCondition, String> {
    let (capture, literals) = capture_and_literals(&predicate.operands)?;
    let binding = captures
        .get(capture)
        .ok_or_else(|| format!("enhanced-query-operand-invalid unbound capture `{capture}`"))?;
    required_rows.insert(binding.capability_row_id.clone());
    let operator = match predicate.name.as_str() {
        "eq" => ResidentSyntaxQueryScalarOperator::Eq,
        "not-eq" => ResidentSyntaxQueryScalarOperator::NotEq,
        "match" => ResidentSyntaxQueryScalarOperator::Match,
        "not-match" => ResidentSyntaxQueryScalarOperator::NotMatch,
        "any-eq" => ResidentSyntaxQueryScalarOperator::AnyEq,
        "any-not-eq" => ResidentSyntaxQueryScalarOperator::AnyNotEq,
        "any-match" => ResidentSyntaxQueryScalarOperator::AnyMatch,
        "any-not-match" => ResidentSyntaxQueryScalarOperator::AnyNotMatch,
        "any-of" => ResidentSyntaxQueryScalarOperator::AnyOf,
        "not-any-of" => ResidentSyntaxQueryScalarOperator::NotAnyOf,
        _ => {
            return Err(format!(
                "enhanced-query-standard-operator-not-resident #{}?",
                predicate.name
            ));
        }
    };
    let value = if matches!(
        operator,
        ResidentSyntaxQueryScalarOperator::AnyOf | ResidentSyntaxQueryScalarOperator::NotAnyOf
    ) {
        if literals.is_empty() {
            return Err("enhanced-query-operand-invalid any-of requires literals".to_owned());
        }
        ResidentSyntaxQueryScalarValue::Literals(
            literals.iter().map(|value| (*value).to_owned()).collect(),
        )
    } else {
        if literals.len() != 1 {
            return Err(format!(
                "enhanced-query-operand-invalid #{}? requires one literal",
                predicate.name
            ));
        }
        ResidentSyntaxQueryScalarValue::Literal(literals[0].to_owned())
    };
    let regex_program_id = if matches!(
        operator,
        ResidentSyntaxQueryScalarOperator::Match
            | ResidentSyntaxQueryScalarOperator::NotMatch
            | ResidentSyntaxQueryScalarOperator::AnyMatch
            | ResidentSyntaxQueryScalarOperator::AnyNotMatch
    ) {
        Some(compile_regex_program(literals[0], regex_programs)?)
    } else {
        None
    };
    Ok(ResidentSyntaxQueryCondition::Scalar {
        capture: capture.to_owned(),
        fact_path: binding.resident_fact_path,
        operator,
        value,
        regex_program_id,
        origin: ResidentSyntaxQueryOrigin {
            kind: ResidentSyntaxQueryOriginKind::StandardPredicate,
            capability_row_id: binding.capability_row_id.clone(),
        },
    })
}

fn lower_asp_predicate(
    predicate: &EnhancedQueryPredicate,
    capability: &EnhancedQueryCapabilityTable,
    required_rows: &mut BTreeSet<String>,
    captures: &BTreeMap<String, ResidentSyntaxQueryCapture>,
    regex_programs: &mut BTreeMap<String, ResidentRegexProgram>,
) -> Result<ResidentSyntaxQueryCondition, String> {
    let capture = capture_operand(&predicate.operands, 0)?;
    if !captures.contains_key(capture) {
        return Err(format!(
            "enhanced-query-operand-invalid unbound capture `{capture}`"
        ));
    }
    match predicate.name.as_str() {
        "asp-eq" | "asp-not-eq" | "asp-match" | "asp-not-match" => {
            require_operand_count(predicate, 3, 3)?;
            let fact_name = string_operand(&predicate.operands, 1)?;
            let value = string_operand(&predicate.operands, 2)?;
            let row =
                capability.runtime_row(EnhancedQueryCapabilityRowKind::FactPath, fact_name)?;
            let lowering = row
                .lowering
                .as_ref()
                .expect("validated runtime capability row has lowering");
            if lowering.constraint_kind != EnhancedQueryConstraintKind::Scalar {
                return Err(format!(
                    "enhanced-query-operand-invalid `{fact_name}` is not a scalar fact"
                ));
            }
            let operator = match predicate.name.as_str() {
                "asp-eq" => ResidentSyntaxQueryScalarOperator::Eq,
                "asp-not-eq" => ResidentSyntaxQueryScalarOperator::NotEq,
                "asp-match" => ResidentSyntaxQueryScalarOperator::Match,
                "asp-not-match" => ResidentSyntaxQueryScalarOperator::NotMatch,
                _ => unreachable!(),
            };
            let regex_program_id = matches!(
                operator,
                ResidentSyntaxQueryScalarOperator::Match
                    | ResidentSyntaxQueryScalarOperator::NotMatch
            )
            .then(|| compile_regex_program(value, regex_programs))
            .transpose()?;
            Ok(ResidentSyntaxQueryCondition::Scalar {
                capture: capture.to_owned(),
                fact_path: lowering.resident_fact_path,
                operator,
                value: ResidentSyntaxQueryScalarValue::Literal(value.to_owned()),
                regex_program_id,
                origin: origin(
                    ResidentSyntaxQueryOriginKind::AspPredicate,
                    row,
                    required_rows,
                ),
            })
        }
        "asp-any-eq" | "asp-none-eq" | "asp-any-match" | "asp-none-match" => {
            require_operand_count(predicate, 3, 3)?;
            let fact_name = string_operand(&predicate.operands, 1)?;
            let value = string_operand(&predicate.operands, 2)?;
            let row =
                capability.runtime_row(EnhancedQueryCapabilityRowKind::FactPath, fact_name)?;
            let lowering = row
                .lowering
                .as_ref()
                .expect("validated runtime capability row has lowering");
            if lowering.constraint_kind != EnhancedQueryConstraintKind::Set {
                return Err(format!(
                    "enhanced-query-operand-invalid `{fact_name}` is not a set fact"
                ));
            }
            let operator = match predicate.name.as_str() {
                "asp-any-eq" => ResidentSyntaxQuerySetOperator::AnyEq,
                "asp-none-eq" => ResidentSyntaxQuerySetOperator::NoneEq,
                "asp-any-match" => ResidentSyntaxQuerySetOperator::AnyMatch,
                "asp-none-match" => ResidentSyntaxQuerySetOperator::NoneMatch,
                _ => unreachable!(),
            };
            let regex_program_id = matches!(
                operator,
                ResidentSyntaxQuerySetOperator::AnyMatch
                    | ResidentSyntaxQuerySetOperator::NoneMatch
            )
            .then(|| compile_regex_program(value, regex_programs))
            .transpose()?;
            Ok(ResidentSyntaxQueryCondition::Set {
                capture: capture.to_owned(),
                fact_path: lowering.resident_fact_path,
                operator,
                value: value.to_owned(),
                regex_program_id,
                origin: origin(
                    ResidentSyntaxQueryOriginKind::AspPredicate,
                    row,
                    required_rows,
                ),
            })
        }
        "asp-range" => {
            require_operand_count(predicate, 5, 5)?;
            let fact_name = string_operand(&predicate.operands, 1)?;
            let row =
                capability.runtime_row(EnhancedQueryCapabilityRowKind::FactPath, fact_name)?;
            let lowering = row
                .lowering
                .as_ref()
                .expect("validated runtime capability row has lowering");
            if lowering.constraint_kind != EnhancedQueryConstraintKind::Range {
                return Err(format!(
                    "enhanced-query-operand-invalid `{fact_name}` is not a range fact"
                ));
            }
            let mode = match string_operand(&predicate.operands, 2)? {
                "within" => ResidentSyntaxQueryRangeMode::Within,
                "contains" => ResidentSyntaxQueryRangeMode::Contains,
                "overlaps" => ResidentSyntaxQueryRangeMode::Overlaps,
                mode => {
                    return Err(format!(
                        "enhanced-query-operand-invalid range mode `{mode}`"
                    ));
                }
            };
            let start = string_operand(&predicate.operands, 3)?.to_owned();
            let end = string_operand(&predicate.operands, 4)?.to_owned();
            canonical_range_bound(&start)?;
            canonical_range_bound(&end)?;
            Ok(ResidentSyntaxQueryCondition::Range {
                capture: capture.to_owned(),
                mode,
                start,
                end,
                origin: origin(
                    ResidentSyntaxQueryOriginKind::AspPredicate,
                    row,
                    required_rows,
                ),
            })
        }
        "asp-related" | "asp-not-related" => {
            require_operand_count(predicate, 3, 4)?;
            let row =
                capability.runtime_row(EnhancedQueryCapabilityRowKind::FactPath, "relations")?;
            let lowering = row
                .lowering
                .as_ref()
                .expect("validated runtime capability row has lowering");
            if lowering.constraint_kind != EnhancedQueryConstraintKind::Relation {
                return Err("enhanced Query relations capability lowering mismatch".to_owned());
            }
            let direction = match string_operand(&predicate.operands, 1)? {
                "in" => ResidentSyntaxQueryDirection::In,
                "out" => ResidentSyntaxQueryDirection::Out,
                "either" => ResidentSyntaxQueryDirection::Either,
                direction => {
                    return Err(format!(
                        "enhanced-query-operand-invalid relation direction `{direction}`"
                    ));
                }
            };
            Ok(ResidentSyntaxQueryCondition::Relation {
                capture: capture.to_owned(),
                operator: if predicate.name == "asp-related" {
                    ResidentSyntaxQueryRelationOperator::Related
                } else {
                    ResidentSyntaxQueryRelationOperator::NotRelated
                },
                direction,
                relation_kind: string_operand(&predicate.operands, 2)?.to_owned(),
                endpoint_selector: predicate
                    .operands
                    .get(3)
                    .map(|_| string_operand(&predicate.operands, 3).map(str::to_owned))
                    .transpose()?,
                origin: origin(
                    ResidentSyntaxQueryOriginKind::AspPredicate,
                    row,
                    required_rows,
                ),
            })
        }
        _ => Err(format!(
            "enhanced-query-operator-unsupported #{}?",
            predicate.name
        )),
    }
}

fn lower_select_directive(
    predicate: &EnhancedQueryPredicate,
    capability: &EnhancedQueryCapabilityTable,
    required_rows: &mut BTreeSet<String>,
    captures: &BTreeMap<String, ResidentSyntaxQueryCapture>,
    selected_fields: &mut BTreeSet<ResidentSyntaxQueryResultField>,
) -> Result<(), String> {
    if predicate.kind != EnhancedQueryPredicateKind::Directive {
        return Err("enhanced-query-operator-unsupported #asp-select?".to_owned());
    }
    if predicate.operands.len() < 2 {
        return Err("enhanced-query-operand-invalid #asp-select! requires fields".to_owned());
    }
    let capture = capture_operand(&predicate.operands, 0)?;
    if !captures.contains_key(capture) {
        return Err(format!(
            "enhanced-query-operand-invalid unbound capture `{capture}`"
        ));
    }
    for operand in &predicate.operands[1..] {
        let EnhancedQueryOperand::String(field) = operand else {
            return Err(
                "enhanced-query-operand-invalid selected field must be a string".to_owned(),
            );
        };
        let field = match field.as_str() {
            "kind" => ResidentSyntaxQueryResultField::Kind,
            "name" => ResidentSyntaxQueryResultField::Name,
            "selector" => ResidentSyntaxQueryResultField::Selector,
            "byte-range" => ResidentSyntaxQueryResultField::ByteRange,
            "scopes" => ResidentSyntaxQueryResultField::Scopes,
            "queryKeys" => ResidentSyntaxQueryResultField::QueryKeys,
            "projections" => ResidentSyntaxQueryResultField::Projections,
            "relations" => ResidentSyntaxQueryResultField::Relations,
            other => {
                return Err(format!(
                    "enhanced-query-operand-invalid selected field `{other}`"
                ));
            }
        };
        if selected_fields.contains(&field) {
            return Err("enhanced-query-operand-invalid selected fields are not unique".to_owned());
        }
        admit_selected_field(field, capability, required_rows, selected_fields)?;
    }
    Ok(())
}

fn admit_selected_field(
    field: ResidentSyntaxQueryResultField,
    capability: &EnhancedQueryCapabilityTable,
    required_rows: &mut BTreeSet<String>,
    selected_fields: &mut BTreeSet<ResidentSyntaxQueryResultField>,
) -> Result<(), String> {
    let fact_names: &[&str] = match field {
        ResidentSyntaxQueryResultField::Kind => &["kind"],
        ResidentSyntaxQueryResultField::Name => &["name"],
        ResidentSyntaxQueryResultField::Selector => &["selector"],
        ResidentSyntaxQueryResultField::ByteRange => &["byte-range"],
        ResidentSyntaxQueryResultField::Scopes => {
            &["scopes.relation", "scopes.kind", "scopes.symbol"]
        }
        ResidentSyntaxQueryResultField::QueryKeys => &["queryKeys"],
        ResidentSyntaxQueryResultField::Projections => &["projections.kind"],
        ResidentSyntaxQueryResultField::Relations => &["relations"],
    };
    for fact_name in fact_names {
        let row = capability.runtime_row(EnhancedQueryCapabilityRowKind::FactPath, fact_name)?;
        required_rows.insert(row.row_id.clone());
    }
    selected_fields.insert(field);
    Ok(())
}

fn origin(
    kind: ResidentSyntaxQueryOriginKind,
    row: &agent_semantic_client_protocol::EnhancedQueryCapabilityRow,
    required_rows: &mut BTreeSet<String>,
) -> ResidentSyntaxQueryOrigin {
    required_rows.insert(row.row_id.clone());
    ResidentSyntaxQueryOrigin {
        kind,
        capability_row_id: row.row_id.clone(),
    }
}

fn capture_and_literals(operands: &[EnhancedQueryOperand]) -> Result<(&str, Vec<&str>), String> {
    if operands.len() < 2 {
        return Err("enhanced-query-operand-invalid standard predicate is incomplete".to_owned());
    }
    let capture = capture_operand(operands, 0)?;
    let literals = operands[1..]
        .iter()
        .map(|operand| match operand {
            EnhancedQueryOperand::String(value) => Ok(value.as_str()),
            EnhancedQueryOperand::Capture(_) => Err(
                "enhanced-query-standard-operator-not-resident capture-to-capture predicate"
                    .to_owned(),
            ),
            EnhancedQueryOperand::Bare(_) => {
                Err("enhanced-query-operand-invalid bare predicate operand".to_owned())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((capture, literals))
}

fn capture_operand(operands: &[EnhancedQueryOperand], index: usize) -> Result<&str, String> {
    match operands.get(index) {
        Some(EnhancedQueryOperand::Capture(value)) => Ok(value),
        _ => Err(format!(
            "enhanced-query-operand-invalid operand {index} must be a capture"
        )),
    }
}

fn string_operand(operands: &[EnhancedQueryOperand], index: usize) -> Result<&str, String> {
    match operands.get(index) {
        Some(EnhancedQueryOperand::String(value)) => Ok(value),
        _ => Err(format!(
            "enhanced-query-operand-invalid operand {index} must be a string"
        )),
    }
}

fn require_operand_count(
    predicate: &EnhancedQueryPredicate,
    minimum: usize,
    maximum: usize,
) -> Result<(), String> {
    if (minimum..=maximum).contains(&predicate.operands.len()) {
        Ok(())
    } else {
        Err(format!(
            "enhanced-query-operand-invalid #{} has {} operands; expected {minimum}..={maximum}",
            predicate.name,
            predicate.operands.len()
        ))
    }
}

fn canonical_range_bound(value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| "enhanced-query-operand-invalid range bound".to_owned())?;
    if parsed.to_string() != value {
        return Err("enhanced-query-operand-invalid non-canonical range bound".to_owned());
    }
    Ok(parsed)
}

fn compile_regex_program(
    source: &str,
    programs: &mut BTreeMap<String, ResidentRegexProgram>,
) -> Result<String, String> {
    let source_digest = digest_bytes(source.as_bytes());
    let id = format!("regex-{}", &source_digest["blake3-256:".len()..][..20]);
    if programs.contains_key(&id) {
        return Ok(id);
    }
    let dense = regex_automata::dfa::dense::Builder::new()
        .build(source)
        .map_err(|error| format!("enhanced-query-regex-invalid {error}"))?;
    let sparse = dense
        .to_sparse()
        .map_err(|error| format!("enhanced-query-regex-invalid {error}"))?;
    let bytes = sparse.to_bytes_native_endian();
    let program_digest = digest_bytes(&bytes);
    programs.insert(
        id.clone(),
        ResidentRegexProgram {
            id: id.clone(),
            engine_id: "regex-automata-sparse-dfa".to_owned(),
            engine_version: "0.4.18".to_owned(),
            syntax_profile: "tree-sitter-rust-regex-v1".to_owned(),
            encoding: "base64".to_owned(),
            program: base64::engine::general_purpose::STANDARD.encode(bytes),
            program_digest,
        },
    );
    Ok(id)
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

#[cfg(test)]
#[path = "../tests/unit/resident_syntax_plan.rs"]
mod tests;
