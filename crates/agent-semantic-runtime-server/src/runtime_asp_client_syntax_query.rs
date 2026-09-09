// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use agent_semantic_client_protocol::{
    AspClientWorkspaceSyntaxQueryEvidence, AspClientWorkspaceSyntaxQueryRequest,
    AspClientWorkspaceSyntaxQueryResponse,
};
use agent_semantic_provider_protocol::{
    SyntaxQueryPattern, SyntaxQueryPlan, SyntaxQueryPredicate, SyntaxQueryPredicateOp,
    SyntaxQueryPredicateValue,
};

use crate::RuntimeQueryGeneration;
use crate::runtime_asp_client::AspClientOperationError;

pub(super) async fn dispatch_workspace_syntax_query(
    params: AspClientWorkspaceSyntaxQueryRequest,
    generation: &RuntimeQueryGeneration,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
) -> Result<serde_json::Value, AspClientOperationError> {
    let evidence =
        execute_workspace_syntax_query_evidence(&params, generation, providers, 3, None).await?;
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
    generation: &RuntimeQueryGeneration,
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
            &block.argv,
            generation,
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

fn query_resident_block(
    argv: &[String],
    generation: &RuntimeQueryGeneration,
    provider: &agent_semantic_search::WorkspaceSearchProvider,
    evidence: &mut Vec<AspClientWorkspaceSyntaxQueryEvidence>,
    seen_selectors: &mut BTreeSet<String>,
    limit: usize,
    admitted_owner_paths: Option<&BTreeSet<String>>,
) -> Result<(), AspClientOperationError> {
    let query_source = native_tree_sitter_query(argv)?;
    let plan =
        agent_semantic_client_core::compile_query_abi_source(query_source).map_err(|error| {
            AspClientOperationError::Message(format!(
                "invalid provider-native syntax query for {}: {}",
                provider.language_id, error.message
            ))
        })?;
    validate_resident_query_plan(&plan)?;
    let source_extensions = provider
        .source_extensions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    if admitted_owner_paths.is_some_and(BTreeSet::is_empty) {
        return Ok(());
    }

    for owner_path in generation.resident().indexed_owner_paths() {
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
        let (projections, _, diagnostics) = generation
            .native_syntax_playbook_projection(std::slice::from_ref(&owner_path))
            .map_err(AspClientOperationError::Message)?;
        if !diagnostics.is_empty() {
            continue;
        }
        for projection in projections {
            for selector in projection.selectors {
                let mut capture = None;
                for pattern in &plan.patterns {
                    if let Some(matched) = resident_pattern_capture(pattern, &plan, &selector)? {
                        capture = Some(matched);
                        break;
                    }
                }
                let Some(capture) = capture else {
                    continue;
                };
                if seen_selectors.insert(selector.selector.clone()) {
                    evidence.push(AspClientWorkspaceSyntaxQueryEvidence {
                        owner: owner_path.clone(),
                        selector: selector.selector,
                        relation: format!("syntax-capture:{capture}"),
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

fn validate_resident_query_plan(plan: &SyntaxQueryPlan) -> Result<(), AspClientOperationError> {
    if plan.patterns.is_empty() || plan.captures.is_empty() {
        return Err(AspClientOperationError::Message(
            "resident syntax Query requires a captured pattern".to_owned(),
        ));
    }
    if plan.patterns.iter().any(|pattern| {
        !pattern.fields.is_empty()
            || pattern.node_types.is_empty()
            || pattern.node_types.iter().any(|node| node != "identifier")
    }) {
        return Err(AspClientOperationError::Message(
            "resident syntax Query V1 supports identifier capture patterns without field traversal"
                .to_owned(),
        ));
    }
    if plan.predicates.iter().any(|predicate| {
        predicate
            .values
            .iter()
            .any(|value| matches!(value, SyntaxQueryPredicateValue::Capture(_)))
    }) {
        return Err(AspClientOperationError::Message(
            "resident syntax Query V1 does not support capture-to-capture predicates".to_owned(),
        ));
    }
    Ok(())
}

fn resident_pattern_capture(
    pattern: &SyntaxQueryPattern,
    plan: &SyntaxQueryPlan,
    selector: &agent_semantic_search::NativeSyntaxSelector,
) -> Result<Option<String>, AspClientOperationError> {
    let Some(capture) = pattern.captures.first() else {
        return Ok(None);
    };
    for predicate in plan
        .predicates
        .iter()
        .filter(|predicate| pattern.captures.contains(&predicate.capture))
    {
        if !resident_predicate_matches(predicate, &selector.query_keys)? {
            return Ok(None);
        }
    }
    Ok(Some(capture.clone()))
}

fn resident_predicate_matches(
    predicate: &SyntaxQueryPredicate,
    query_keys: &[String],
) -> Result<bool, AspClientOperationError> {
    let literals = predicate
        .values
        .iter()
        .map(|value| match value {
            SyntaxQueryPredicateValue::String(value) => Ok(value.as_str()),
            SyntaxQueryPredicateValue::Capture(_) => Err(AspClientOperationError::Message(
                "resident syntax Query capture predicate escaped validation".to_owned(),
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let any_equal = || {
        query_keys
            .iter()
            .any(|key| literals.iter().any(|literal| key == literal))
    };
    let any_regex = || -> Result<bool, AspClientOperationError> {
        let patterns = literals
            .iter()
            .map(|literal| {
                regex::Regex::new(literal).map_err(|error| {
                    AspClientOperationError::Message(format!(
                        "resident syntax Query regex is invalid: {error}"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(query_keys
            .iter()
            .any(|key| patterns.iter().any(|pattern| pattern.is_match(key))))
    };
    match predicate.op {
        SyntaxQueryPredicateOp::Eq
        | SyntaxQueryPredicateOp::AnyEq
        | SyntaxQueryPredicateOp::AnyOf => Ok(any_equal()),
        SyntaxQueryPredicateOp::NotEq => Ok(!any_equal()),
        SyntaxQueryPredicateOp::Match | SyntaxQueryPredicateOp::AnyMatch => any_regex(),
        SyntaxQueryPredicateOp::NotMatch => Ok(!any_regex()?),
    }
}

fn native_tree_sitter_query(argv: &[String]) -> Result<&str, AspClientOperationError> {
    for (index, argument) in argv.iter().enumerate() {
        if argument == "--treesitter-query" {
            return argv.get(index + 1).map(String::as_str).ok_or_else(|| {
                AspClientOperationError::Message(
                    "provider-native --treesitter-query requires an S-expression".to_owned(),
                )
            });
        }
        if let Some(query) = argument.strip_prefix("--treesitter-query=") {
            return Ok(query);
        }
    }
    Err(AspClientOperationError::Message(
        "registered provider syntax contract requires --treesitter-query <s-expr>".to_owned(),
    ))
}

#[cfg(test)]
#[path = "../tests/unit/runtime_asp_client_syntax_query.rs"]
mod tests;
