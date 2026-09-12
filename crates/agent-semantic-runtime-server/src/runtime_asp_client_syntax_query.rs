// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use agent_semantic_client_protocol::{
    AspClientWorkspaceSyntaxQueryEvidence, AspClientWorkspaceSyntaxQueryRequest,
    AspClientWorkspaceSyntaxQueryResponse, AspClientWorkspaceSyntaxQueryScope,
    AspClientWorkspaceSyntaxQuerySelection, ResidentSyntaxQueryCondition,
    ResidentSyntaxQueryPattern, ResidentSyntaxQueryPlan, ResidentSyntaxQueryRangeMode,
    ResidentSyntaxQueryResultField, ResidentSyntaxQueryScalarOperator,
    ResidentSyntaxQueryScalarValue, ResidentSyntaxQuerySetOperator,
};
use base64::Engine;

use crate::runtime_asp_client::AspClientOperationError;

pub(super) async fn dispatch_workspace_syntax_query(
    params: AspClientWorkspaceSyntaxQueryRequest,
    generation_digest: &str,
    resident: &agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
) -> Result<serde_json::Value, AspClientOperationError> {
    let evidence = execute_workspace_syntax_query_evidence(
        &params,
        generation_digest,
        resident,
        providers,
        3,
        None,
    )
    .await?;
    serde_json::to_value(AspClientWorkspaceSyntaxQueryResponse {
        schema_id: "agent.semantic-protocols.asp-client-workspace-syntax-query-response".to_owned(),
        schema_version: "1".to_owned(),
        state: "ready".to_owned(),
        evidence,
    })
    .map_err(|error| AspClientOperationError::Message(error.to_string()))
}

pub(super) async fn execute_workspace_syntax_query_evidence(
    params: &AspClientWorkspaceSyntaxQueryRequest,
    generation_digest: &str,
    resident: &agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
    limit: usize,
    admitted_owner_paths: Option<&BTreeSet<String>>,
) -> Result<Vec<AspClientWorkspaceSyntaxQueryEvidence>, AspClientOperationError> {
    if limit == 0 {
        return Err(AspClientOperationError::Message(
            "workspace syntax Query evidence limit must be non-zero".to_owned(),
        ));
    }
    let selected = params
        .languages
        .iter()
        .chain(params.documents.iter())
        .flat_map(|expression| expression.split('|'))
        .collect::<BTreeSet<_>>();
    let provider_index = providers
        .iter()
        .map(|provider| (provider.language_id.as_str(), provider))
        .collect::<BTreeMap<_, _>>();
    let mut evidence = Vec::new();
    let mut seen_selectors = BTreeSet::new();

    for block in &params.syntax {
        if !producer_matches_optional_calibration(&selected, &block.producer) {
            return Err(AspClientOperationError::Message(format!(
                "syntax producer is not selected by --language: {}",
                block.producer
            )));
        }
        let provider = provider_index.get(block.producer.as_str()).ok_or_else(|| {
            AspClientOperationError::Message(format!(
                "workspace syntax Query provider is not registered: {}",
                block.producer
            ))
        })?;
        query_resident_block(
            &block.plan,
            generation_digest,
            resident,
            provider,
            &mut evidence,
            &mut seen_selectors,
            limit,
            admitted_owner_paths,
        )?;
    }
    Ok(evidence)
}

fn producer_matches_optional_calibration(selected: &BTreeSet<&str>, producer: &str) -> bool {
    selected.is_empty() || selected.contains(producer)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the V1 query block binds route, generation, selector, and telemetry identity explicitly"
)]
fn query_resident_block(
    plan: &ResidentSyntaxQueryPlan,
    generation_digest: &str,
    resident: &agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
    provider: &agent_semantic_search::WorkspaceSearchProvider,
    evidence: &mut Vec<AspClientWorkspaceSyntaxQueryEvidence>,
    seen_selectors: &mut BTreeSet<String>,
    limit: usize,
    admitted_owner_paths: Option<&BTreeSet<String>>,
) -> Result<(), AspClientOperationError> {
    validate_resident_query_plan(plan, generation_digest, provider)?;
    let regex_programs = resident_regex_programs(plan)?;
    let source_extensions = provider
        .source_extensions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    if admitted_owner_paths.is_some_and(BTreeSet::is_empty) {
        return Ok(());
    }

    for owner_path in resident.indexed_owner_paths() {
        if evidence.len() == limit {
            break;
        }
        let extension = Path::new(&owner_path)
            .extension()
            .and_then(|value| value.to_str());
        if !extension.is_some_and(|value| source_extensions.contains(value)) {
            continue;
        }
        if admitted_owner_paths.is_some_and(|owners| !owners.contains(&owner_path)) {
            continue;
        }
        let (projections, _, diagnostics) = resident
            .native_syntax_playbook_projection(std::slice::from_ref(&owner_path))
            .map_err(AspClientOperationError::Message)?;
        if !diagnostics.is_empty() {
            continue;
        }
        for projection in projections {
            for selector in projection.selectors {
                let canonical = agent_semantic_content_identity::CanonicalItemSelector::parse(
                    selector.selector.clone(),
                )
                .map_err(AspClientOperationError::Message)?;
                let mut capture = None;
                for pattern in &plan.patterns {
                    if let Some(matched) =
                        resident_pattern_capture(pattern, &canonical, &selector, &regex_programs)?
                    {
                        capture = Some(matched);
                        break;
                    }
                }
                let Some(capture) = capture else {
                    continue;
                };
                if seen_selectors.insert(selector.selector.clone()) {
                    let selected = project_selected_fields(plan, &canonical, &selector)?;
                    evidence.push(AspClientWorkspaceSyntaxQueryEvidence {
                        owner: owner_path.clone(),
                        selector: selector.selector,
                        capture: capture.clone(),
                        relation: format!("syntax-capture:{capture}"),
                        selected,
                    });
                    if evidence.len() == limit {
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_resident_query_plan(
    plan: &ResidentSyntaxQueryPlan,
    generation_digest: &str,
    provider: &agent_semantic_search::WorkspaceSearchProvider,
) -> Result<(), AspClientOperationError> {
    plan.validate().map_err(AspClientOperationError::Message)?;
    let capability = provider.enhanced_query_capability.as_ref().ok_or_else(|| {
        AspClientOperationError::Message(format!(
            "enhanced-query-capability-table-missing producer={}",
            provider.language_id
        ))
    })?;
    if plan.plan_digest
        != plan
            .recompute_plan_digest()
            .map_err(AspClientOperationError::Message)?
        || plan.language_id != provider.language_id
        || plan.provider_id != provider.provider_id
        || plan.generation_digest != generation_digest
        || plan.parser_abi_digest != capability.parser_abi.digest
        || plan.query_grammar_digest != capability.query_grammar.digest
        || plan.operator_table_digest != capability.operator_table_digest
        || plan.capability_table_digest != capability.table_digest
    {
        return Err(AspClientOperationError::Message(
            "resident syntax Query plan authority mismatch".to_owned(),
        ));
    }
    let runtime_rows = capability
        .rows
        .iter()
        .filter(|row| {
            row.publication_state
                == agent_semantic_client_protocol::EnhancedQueryPublicationState::Runtime
        })
        .map(|row| row.row_id.as_str())
        .collect::<BTreeSet<_>>();
    if plan
        .required_capability_rows
        .iter()
        .any(|row| !runtime_rows.contains(row.as_str()))
    {
        return Err(AspClientOperationError::Message(
            "resident syntax Query plan requires a non-runtime capability row".to_owned(),
        ));
    }
    Ok(())
}

fn resident_regex_programs(
    plan: &ResidentSyntaxQueryPlan,
) -> Result<BTreeMap<String, Vec<u8>>, AspClientOperationError> {
    plan.regex_programs
        .iter()
        .map(|program| {
            if program.engine_id != "regex-automata-sparse-dfa"
                || program.engine_version != "0.4.18"
            {
                return Err(AspClientOperationError::Message(
                    "resident syntax Query regex engine identity mismatch".to_owned(),
                ));
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&program.program)
                .map_err(|error| {
                    AspClientOperationError::Message(format!(
                        "resident syntax Query regex program encoding is invalid: {error}"
                    ))
                })?;
            if digest_bytes(&bytes) != program.program_digest {
                return Err(AspClientOperationError::Message(
                    "resident syntax Query regex program digest mismatch".to_owned(),
                ));
            }
            regex_automata::dfa::sparse::DFA::from_bytes(&bytes).map_err(|error| {
                AspClientOperationError::Message(format!(
                    "resident syntax Query regex program is invalid: {error}"
                ))
            })?;
            Ok((program.id.clone(), bytes))
        })
        .collect()
}

fn resident_pattern_capture(
    pattern: &ResidentSyntaxQueryPattern,
    canonical: &agent_semantic_content_identity::CanonicalItemSelector,
    selector: &agent_semantic_search::NativeSyntaxSelector,
    regex_programs: &BTreeMap<String, Vec<u8>>,
) -> Result<Option<String>, AspClientOperationError> {
    if !resident_condition_matches(&pattern.structure, canonical, selector, regex_programs)? {
        return Ok(None);
    }
    for predicate in &pattern.predicates {
        if !resident_condition_matches(predicate, canonical, selector, regex_programs)? {
            return Ok(None);
        }
    }
    Ok(pattern.captures.first().map(|capture| capture.name.clone()))
}

fn project_selected_fields(
    plan: &ResidentSyntaxQueryPlan,
    canonical: &agent_semantic_content_identity::CanonicalItemSelector,
    selector: &agent_semantic_search::NativeSyntaxSelector,
) -> Result<AspClientWorkspaceSyntaxQuerySelection, AspClientOperationError> {
    let mut selected = AspClientWorkspaceSyntaxQuerySelection::default();
    for field in &plan.selected_fields {
        match field {
            ResidentSyntaxQueryResultField::Kind => {
                selected.kind = Some(canonical.kind.as_str().to_owned());
            }
            ResidentSyntaxQueryResultField::Name => {
                selected.name = Some(canonical.symbol.as_str().to_owned());
            }
            ResidentSyntaxQueryResultField::Selector => {
                selected.selector = Some(selector.selector.clone());
            }
            ResidentSyntaxQueryResultField::ByteRange => {
                selected.byte_range = Some([selector.byte_start, selector.byte_end]);
            }
            ResidentSyntaxQueryResultField::Scopes => {
                selected.scopes = canonical
                    .scopes
                    .iter()
                    .map(|scope| AspClientWorkspaceSyntaxQueryScope {
                        relation: scope.relation.as_str().to_owned(),
                        kind: scope.kind.as_str().to_owned(),
                        symbol: scope.symbol.as_str().to_owned(),
                    })
                    .collect();
            }
            ResidentSyntaxQueryResultField::QueryKeys => {
                selected.query_keys = selector.query_keys.clone();
            }
            ResidentSyntaxQueryResultField::Projections
            | ResidentSyntaxQueryResultField::Relations => {
                return Err(AspClientOperationError::Message(
                    "resident syntax Query selected field lacks an executable projection"
                        .to_owned(),
                ));
            }
        }
    }
    Ok(selected)
}

fn resident_condition_matches(
    condition: &ResidentSyntaxQueryCondition,
    canonical: &agent_semantic_content_identity::CanonicalItemSelector,
    selector: &agent_semantic_search::NativeSyntaxSelector,
    regex_programs: &BTreeMap<String, Vec<u8>>,
) -> Result<bool, AspClientOperationError> {
    match condition {
        ResidentSyntaxQueryCondition::True { .. } => Ok(true),
        ResidentSyntaxQueryCondition::All { terms } => {
            for term in terms {
                if !resident_condition_matches(term, canonical, selector, regex_programs)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        ResidentSyntaxQueryCondition::Any { terms } => {
            for term in terms {
                if resident_condition_matches(term, canonical, selector, regex_programs)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        ResidentSyntaxQueryCondition::Scalar {
            fact_path,
            operator,
            value,
            regex_program_id,
            ..
        } => {
            let actual = match fact_path {
                agent_semantic_client_protocol::ResidentSyntaxQueryFactPath::Kind => {
                    canonical.kind.as_str()
                }
                agent_semantic_client_protocol::ResidentSyntaxQueryFactPath::Name => {
                    canonical.symbol.as_str()
                }
                agent_semantic_client_protocol::ResidentSyntaxQueryFactPath::Selector => {
                    selector.selector.as_str()
                }
                _ => {
                    return Err(AspClientOperationError::Message(
                        "resident syntax Query scalar fact path escaped admission".to_owned(),
                    ));
                }
            };
            scalar_condition_matches(
                actual,
                *operator,
                value,
                regex_program_id.as_deref(),
                regex_programs,
            )
        }
        ResidentSyntaxQueryCondition::Set {
            fact_path,
            operator,
            value,
            regex_program_id,
            ..
        } => {
            let actual = match fact_path {
                agent_semantic_client_protocol::ResidentSyntaxQueryFactPath::QueryKeys => selector
                    .query_keys
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                agent_semantic_client_protocol::ResidentSyntaxQueryFactPath::ScopesRelation => {
                    canonical
                        .scopes
                        .iter()
                        .map(|scope| scope.relation.as_str())
                        .collect()
                }
                agent_semantic_client_protocol::ResidentSyntaxQueryFactPath::ScopesKind => {
                    canonical
                        .scopes
                        .iter()
                        .map(|scope| scope.kind.as_str())
                        .collect()
                }
                agent_semantic_client_protocol::ResidentSyntaxQueryFactPath::ScopesSymbol => {
                    canonical
                        .scopes
                        .iter()
                        .map(|scope| scope.symbol.as_str())
                        .collect()
                }
                _ => {
                    return Err(AspClientOperationError::Message(
                        "resident syntax Query set fact path is not materialized".to_owned(),
                    ));
                }
            };
            set_condition_matches(
                &actual,
                *operator,
                value,
                regex_program_id.as_deref(),
                regex_programs,
            )
        }
        ResidentSyntaxQueryCondition::Range {
            mode, start, end, ..
        } => {
            let start = start.parse::<usize>().map_err(|_| {
                AspClientOperationError::Message(
                    "resident syntax Query range start escaped validation".to_owned(),
                )
            })?;
            let end = end.parse::<usize>().map_err(|_| {
                AspClientOperationError::Message(
                    "resident syntax Query range end escaped validation".to_owned(),
                )
            })?;
            Ok(match mode {
                ResidentSyntaxQueryRangeMode::Within => {
                    selector.byte_start >= start && selector.byte_end <= end
                }
                ResidentSyntaxQueryRangeMode::Contains => {
                    selector.byte_start <= start && selector.byte_end >= end
                }
                ResidentSyntaxQueryRangeMode::Overlaps => {
                    selector.byte_start < end && start < selector.byte_end
                }
            })
        }
        ResidentSyntaxQueryCondition::Relation { .. } => Err(AspClientOperationError::Message(
            "resident syntax Query relation facts are not materialized".to_owned(),
        )),
    }
}

fn scalar_condition_matches(
    actual: &str,
    operator: ResidentSyntaxQueryScalarOperator,
    expected: &ResidentSyntaxQueryScalarValue,
    regex_program_id: Option<&str>,
    regex_programs: &BTreeMap<String, Vec<u8>>,
) -> Result<bool, AspClientOperationError> {
    match (operator, expected) {
        (
            ResidentSyntaxQueryScalarOperator::Eq | ResidentSyntaxQueryScalarOperator::AnyEq,
            ResidentSyntaxQueryScalarValue::Literal(expected),
        ) => Ok(actual == expected),
        (
            ResidentSyntaxQueryScalarOperator::NotEq | ResidentSyntaxQueryScalarOperator::AnyNotEq,
            ResidentSyntaxQueryScalarValue::Literal(expected),
        ) => Ok(actual != expected),
        (
            ResidentSyntaxQueryScalarOperator::Match | ResidentSyntaxQueryScalarOperator::AnyMatch,
            ResidentSyntaxQueryScalarValue::Literal(_),
        ) => regex_program_matches(required_regex(regex_program_id, regex_programs)?, actual),
        (
            ResidentSyntaxQueryScalarOperator::NotMatch
            | ResidentSyntaxQueryScalarOperator::AnyNotMatch,
            ResidentSyntaxQueryScalarValue::Literal(_),
        ) => Ok(!regex_program_matches(
            required_regex(regex_program_id, regex_programs)?,
            actual,
        )?),
        (
            ResidentSyntaxQueryScalarOperator::AnyOf,
            ResidentSyntaxQueryScalarValue::Literals(expected),
        ) => Ok(expected.iter().any(|expected| actual == expected)),
        (
            ResidentSyntaxQueryScalarOperator::NotAnyOf,
            ResidentSyntaxQueryScalarValue::Literals(expected),
        ) => Ok(expected.iter().all(|expected| actual != expected)),
        _ => Err(AspClientOperationError::Message(
            "resident syntax Query scalar value escaped validation".to_owned(),
        )),
    }
}

fn set_condition_matches(
    actual: &[&str],
    operator: ResidentSyntaxQuerySetOperator,
    expected: &str,
    regex_program_id: Option<&str>,
    regex_programs: &BTreeMap<String, Vec<u8>>,
) -> Result<bool, AspClientOperationError> {
    match operator {
        ResidentSyntaxQuerySetOperator::AnyEq => Ok(actual.contains(&expected)),
        ResidentSyntaxQuerySetOperator::NoneEq => Ok(!actual.contains(&expected)),
        ResidentSyntaxQuerySetOperator::AnyMatch => {
            let program = required_regex(regex_program_id, regex_programs)?;
            for value in actual {
                if regex_program_matches(program, value)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        ResidentSyntaxQuerySetOperator::NoneMatch => {
            let program = required_regex(regex_program_id, regex_programs)?;
            for value in actual {
                if regex_program_matches(program, value)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

fn required_regex<'a>(
    id: Option<&str>,
    programs: &'a BTreeMap<String, Vec<u8>>,
) -> Result<&'a [u8], AspClientOperationError> {
    id.and_then(|id| programs.get(id))
        .map(Vec::as_slice)
        .ok_or_else(|| {
            AspClientOperationError::Message(
                "resident syntax Query regex program reference is missing".to_owned(),
            )
        })
}

fn regex_program_matches(program: &[u8], value: &str) -> Result<bool, AspClientOperationError> {
    use regex_automata::dfa::Automaton;
    let (dfa, _) = regex_automata::dfa::sparse::DFA::from_bytes(program).map_err(|error| {
        AspClientOperationError::Message(format!(
            "resident syntax Query regex program is invalid: {error}"
        ))
    })?;
    dfa.try_search_fwd(&regex_automata::Input::new(value))
        .map(|matched| matched.is_some())
        .map_err(|error| {
            AspClientOperationError::Message(format!(
                "resident syntax Query regex execution failed: {error}"
            ))
        })
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

#[cfg(test)]
#[path = "../tests/unit/runtime_asp_client_syntax_query.rs"]
mod tests;
