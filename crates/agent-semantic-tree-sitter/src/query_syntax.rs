// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Compatibility projection from the official Query grammar AST into the
//! provider-native syntax metadata ABI.
//!
//! This is not a second Query parser. Both the resident-plan compiler and the
//! older native-provider projection consume [`crate::parse_enhanced_query_source`].

use std::collections::BTreeSet;

use crate::{
    EnhancedQueryExpression, EnhancedQueryOperand, EnhancedQueryPredicate,
    EnhancedQueryPredicateKind, parse_enhanced_query_source,
};

pub use agent_semantic_provider_protocol::SyntaxQueryPattern as SyntaxQueryAbiPattern;
pub use agent_semantic_provider_protocol::SyntaxQueryPlan as SyntaxQueryAbiPlan;
pub use agent_semantic_provider_protocol::SyntaxQueryPredicate as SyntaxQueryAbiPredicate;
pub use agent_semantic_provider_protocol::SyntaxQueryPredicateOp;
pub use agent_semantic_provider_protocol::SyntaxQueryPredicateValue;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxQueryAbiError {
    pub message: String,
}

impl std::fmt::Display for SyntaxQueryAbiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SyntaxQueryAbiError {}

/// Project the hierarchy-preserving official Query AST into the transitional
/// flat metadata passed to provider-native query execution.
pub fn compile_query_abi_source(source: &str) -> Result<SyntaxQueryAbiPlan, SyntaxQueryAbiError> {
    let document = parse_enhanced_query_source(source).map_err(|error| SyntaxQueryAbiError {
        message: error.message,
    })?;
    let mut patterns = Vec::with_capacity(document.patterns.len());
    let mut predicates = BTreeSet::new();
    for pattern in document.patterns {
        let mut captures = BTreeSet::new();
        let mut node_types = BTreeSet::new();
        let mut fields = BTreeSet::new();
        collect_structure(
            &pattern.structure,
            &mut captures,
            &mut node_types,
            &mut fields,
        );
        for predicate in &pattern.predicates {
            if let Some(predicate) = project_predicate(predicate) {
                predicates.insert(predicate);
            }
        }
        patterns.push(SyntaxQueryAbiPattern {
            index: pattern.index,
            captures: captures.into_iter().collect(),
            node_types: node_types.into_iter().collect(),
            fields: fields.into_iter().collect(),
        });
    }
    let captures = union_sorted(patterns.iter().flat_map(|pattern| &pattern.captures));
    let node_types = union_sorted(patterns.iter().flat_map(|pattern| &pattern.node_types));
    let fields = union_sorted(patterns.iter().flat_map(|pattern| &pattern.fields));
    Ok(SyntaxQueryAbiPlan {
        patterns,
        captures,
        node_types,
        fields,
        predicates: predicates.into_iter().collect(),
    })
}

fn collect_structure(
    expression: &EnhancedQueryExpression,
    captures: &mut BTreeSet<String>,
    node_types: &mut BTreeSet<String>,
    fields: &mut BTreeSet<String>,
) {
    match expression {
        EnhancedQueryExpression::NamedNode {
            name,
            captures: node_captures,
            children,
            ..
        } => {
            if name != "_" {
                node_types.insert(name.clone());
            }
            captures.extend(node_captures.iter().cloned());
            for child in children {
                collect_structure(child, captures, node_types, fields);
            }
        }
        EnhancedQueryExpression::AnonymousNode {
            captures: node_captures,
            ..
        } => captures.extend(node_captures.iter().cloned()),
        EnhancedQueryExpression::Field { name, value } => {
            fields.insert(name.clone());
            collect_structure(value, captures, node_types, fields);
        }
        EnhancedQueryExpression::NegatedField(name) => {
            fields.insert(name.clone());
        }
        EnhancedQueryExpression::Alternation {
            captures: expression_captures,
            alternatives,
            ..
        } => {
            captures.extend(expression_captures.iter().cloned());
            for alternative in alternatives {
                collect_structure(alternative, captures, node_types, fields);
            }
        }
        EnhancedQueryExpression::Group {
            captures: expression_captures,
            terms,
            ..
        } => {
            captures.extend(expression_captures.iter().cloned());
            for term in terms {
                collect_structure(term, captures, node_types, fields);
            }
        }
        EnhancedQueryExpression::Anchor => {}
    }
}

fn project_predicate(predicate: &EnhancedQueryPredicate) -> Option<SyntaxQueryAbiPredicate> {
    if predicate.kind != EnhancedQueryPredicateKind::Predicate {
        return None;
    }
    let op = match predicate.name.as_str() {
        "eq" => SyntaxQueryPredicateOp::Eq,
        "any-eq" => SyntaxQueryPredicateOp::AnyEq,
        "any-of" => SyntaxQueryPredicateOp::AnyOf,
        "match" => SyntaxQueryPredicateOp::Match,
        "any-match" => SyntaxQueryPredicateOp::AnyMatch,
        "not-eq" => SyntaxQueryPredicateOp::NotEq,
        "not-match" => SyntaxQueryPredicateOp::NotMatch,
        _ => return None,
    };
    let (first, rest) = predicate.operands.split_first()?;
    let EnhancedQueryOperand::Capture(capture) = first else {
        return None;
    };
    let values = rest
        .iter()
        .filter_map(|operand| match operand {
            EnhancedQueryOperand::Capture(value) => {
                Some(SyntaxQueryPredicateValue::Capture(value.clone()))
            }
            EnhancedQueryOperand::String(value) => {
                Some(SyntaxQueryPredicateValue::String(value.clone()))
            }
            EnhancedQueryOperand::Bare(_) => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| SyntaxQueryAbiPredicate {
        op,
        capture: capture.clone(),
        values,
    })
}

fn union_sorted<'a>(values: impl Iterator<Item = &'a String>) -> Vec<String> {
    values
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
