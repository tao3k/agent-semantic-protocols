// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Native Tantivy query-grammar admission for Search Playbook input.

use std::collections::BTreeSet;

use tantivy_query_grammar::{Delimiter, UserInputAst, UserInputLeaf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TantivyQueryDiagnostic {
    pub char_offset: Option<usize>,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TantivyQueryMetrics {
    pub leaf_count: usize,
    pub fielded_leaf_count: usize,
    pub explicit_boolean_count: usize,
    pub phrase_count: usize,
    pub boost_count: usize,
    pub range_count: usize,
    pub set_count: usize,
    pub exists_count: usize,
    pub regex_count: usize,
    pub max_depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TantivyQueryAnalysis {
    pub expression: String,
    pub fields: Vec<String>,
    pub metrics: TantivyQueryMetrics,
    pub syntax_diagnostics: Vec<TantivyQueryDiagnostic>,
    pub unsupported_fields: Vec<String>,
    pub missing_features: Vec<&'static str>,
}

impl TantivyQueryAnalysis {
    pub fn is_admitted(&self) -> bool {
        self.syntax_diagnostics.is_empty()
            && self.unsupported_fields.is_empty()
            && self.missing_features.is_empty()
    }
}

pub fn analyze_tantivy_query(expression: &str) -> TantivyQueryAnalysis {
    let (recovered, lenient_errors) = tantivy_query_grammar::parse_query_lenient(expression);
    let strict = tantivy_query_grammar::parse_query(expression);
    let mut syntax_diagnostics = lenient_errors
        .into_iter()
        .map(|error| TantivyQueryDiagnostic {
            char_offset: Some(error.pos),
            message: error.message,
        })
        .collect::<Vec<_>>();
    if strict.is_err() && syntax_diagnostics.is_empty() {
        syntax_diagnostics.push(TantivyQueryDiagnostic {
            char_offset: None,
            message: "native Tantivy query grammar rejected the expression".to_owned(),
        });
    }
    let ast = strict.as_ref().unwrap_or(&recovered);
    let mut metrics = TantivyQueryMetrics::default();
    let mut fields = BTreeSet::new();
    measure(ast, 1, &mut metrics, &mut fields);
    let unsupported_fields = fields
        .iter()
        .filter(|field| !matches!(field.as_str(), "title" | "body"))
        .cloned()
        .collect::<Vec<_>>();
    let mut missing_features = Vec::new();
    if metrics.leaf_count < 2 {
        missing_features.push("multiple-clauses");
    }
    if metrics.fielded_leaf_count == 0 {
        missing_features.push("fielded-clause");
    }
    if metrics.explicit_boolean_count == 0 {
        missing_features.push("explicit-boolean-composition");
    }
    if metrics.phrase_count
        + metrics.boost_count
        + metrics.range_count
        + metrics.set_count
        + metrics.exists_count
        + metrics.regex_count
        == 0
    {
        missing_features.push("phrase-boost-or-structured-predicate");
    }
    TantivyQueryAnalysis {
        expression: expression.to_owned(),
        fields: fields.into_iter().collect(),
        metrics,
        syntax_diagnostics,
        unsupported_fields,
        missing_features,
    }
}

fn measure(
    ast: &UserInputAst,
    depth: usize,
    metrics: &mut TantivyQueryMetrics,
    fields: &mut BTreeSet<String>,
) {
    metrics.max_depth = metrics.max_depth.max(depth);
    match ast {
        UserInputAst::Clause(clauses) => {
            if clauses.len() >= 2 && clauses.iter().any(|(occur, _)| occur.is_some()) {
                metrics.explicit_boolean_count += 1;
            }
            for (_, child) in clauses {
                measure(child, depth + 1, metrics, fields);
            }
        }
        UserInputAst::Boost(child, _) => {
            metrics.boost_count += 1;
            measure(child, depth + 1, metrics, fields);
        }
        UserInputAst::Leaf(leaf) => {
            metrics.leaf_count += 1;
            match leaf.as_ref() {
                UserInputLeaf::Literal(literal) => {
                    if literal.delimiter != Delimiter::None {
                        metrics.phrase_count += 1;
                    }
                    record_field(literal.field_name.as_deref(), metrics, fields);
                }
                UserInputLeaf::Range { field, .. } => {
                    metrics.range_count += 1;
                    record_field(field.as_deref(), metrics, fields);
                }
                UserInputLeaf::Set { field, .. } => {
                    metrics.set_count += 1;
                    record_field(field.as_deref(), metrics, fields);
                }
                UserInputLeaf::Exists { field } => {
                    metrics.exists_count += 1;
                    record_field(Some(field), metrics, fields);
                }
                UserInputLeaf::Regex { field, .. } => {
                    metrics.regex_count += 1;
                    record_field(field.as_deref(), metrics, fields);
                }
                UserInputLeaf::All => {}
            }
        }
    }
}

fn record_field(
    field: Option<&str>,
    metrics: &mut TantivyQueryMetrics,
    fields: &mut BTreeSet<String>,
) {
    if let Some(field) = field {
        metrics.fielded_leaf_count += 1;
        fields.insert(field.to_owned());
    }
}
