// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Native Tantivy query-grammar admission for Search Playbook input.

use std::collections::BTreeSet;

use tantivy_query_grammar::{Delimiter, Occur, UserInputAst, UserInputLeaf};

const SELECTOR_PROJECTION_BRANCH_LIMIT: usize = 32;

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
    /// Positive parser-symbol conjunctions that can soundly ground exact
    /// selectors. Owner/title predicates never appear in this carrier.
    pub selector_queries: Vec<String>,
    /// False means the owner query is still valid, but some body predicate is
    /// not representable by the exact Symbol Skeleton Index.
    pub selector_projection_complete: bool,
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
    let selector_projection = selector_projection(ast);
    TantivyQueryAnalysis {
        expression: expression.to_owned(),
        fields: fields.into_iter().collect(),
        metrics,
        syntax_diagnostics,
        unsupported_fields,
        missing_features,
        selector_queries: selector_projection
            .branches
            .into_iter()
            .map(|branch| branch.join(" "))
            .collect(),
        selector_projection_complete: selector_projection.complete,
    }
}

#[derive(Debug)]
struct SelectorProjection {
    branches: Vec<Vec<String>>,
    complete: bool,
}

fn selector_projection(ast: &UserInputAst) -> SelectorProjection {
    let mut projection = selector_projection_inner(ast);
    for branch in &mut projection.branches {
        branch.sort_unstable();
        branch.dedup();
    }
    projection.branches.retain(|branch| !branch.is_empty());
    projection.branches.sort_unstable();
    projection.branches.dedup();
    if projection.branches.len() > SELECTOR_PROJECTION_BRANCH_LIMIT {
        projection.branches.clear();
        projection.complete = false;
    }
    projection
}

fn selector_projection_inner(ast: &UserInputAst) -> SelectorProjection {
    match ast {
        UserInputAst::Boost(child, _) => selector_projection_inner(child),
        UserInputAst::Leaf(leaf) => selector_leaf_projection(leaf),
        UserInputAst::Clause(clauses) => {
            let mut must = vec![Vec::<String>::new()];
            let mut must_has_body = false;
            let mut should = Vec::new();
            let mut complete = true;
            for (occur, child) in clauses {
                let child = selector_projection_inner(child);
                complete &= child.complete;
                match occur.unwrap_or(Occur::Should) {
                    Occur::MustNot => {
                        if !child.branches.is_empty() {
                            complete = false;
                        }
                    }
                    Occur::Must => {
                        if !child.branches.is_empty() {
                            must_has_body = true;
                            must = conjunction_product(must, child.branches, &mut complete);
                        }
                    }
                    Occur::Should => should.extend(child.branches),
                }
            }
            SelectorProjection {
                branches: if must_has_body { must } else { should },
                complete,
            }
        }
    }
}

fn selector_leaf_projection(leaf: &UserInputLeaf) -> SelectorProjection {
    match leaf {
        UserInputLeaf::Literal(literal)
            if literal
                .field_name
                .as_deref()
                .is_none_or(|field| field == "body")
                && !literal.prefix
                && !literal.phrase.trim().is_empty() =>
        {
            SelectorProjection {
                branches: vec![vec![literal.phrase.clone()]],
                complete: true,
            }
        }
        UserInputLeaf::Literal(literal)
            if literal
                .field_name
                .as_deref()
                .is_some_and(|field| field != "body") =>
        {
            SelectorProjection {
                branches: Vec::new(),
                complete: true,
            }
        }
        UserInputLeaf::All => SelectorProjection {
            branches: Vec::new(),
            complete: true,
        },
        UserInputLeaf::Exists { field } if field != "body" => SelectorProjection {
            branches: Vec::new(),
            complete: true,
        },
        UserInputLeaf::Range { field, .. }
        | UserInputLeaf::Set { field, .. }
        | UserInputLeaf::Regex { field, .. }
            if field.as_deref().is_some_and(|field| field != "body") =>
        {
            SelectorProjection {
                branches: Vec::new(),
                complete: true,
            }
        }
        _ => SelectorProjection {
            branches: Vec::new(),
            complete: false,
        },
    }
}

fn conjunction_product(
    left: Vec<Vec<String>>,
    right: Vec<Vec<String>>,
    complete: &mut bool,
) -> Vec<Vec<String>> {
    if left.len().saturating_mul(right.len()) > SELECTOR_PROJECTION_BRANCH_LIMIT {
        *complete = false;
        return Vec::new();
    }
    left.into_iter()
        .flat_map(|left_branch| {
            right.iter().map(move |right_branch| {
                let mut branch = left_branch.clone();
                branch.extend(right_branch.iter().cloned());
                branch
            })
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::analyze_tantivy_query;

    #[test]
    fn title_or_body_projects_only_the_positive_symbol_branch() {
        let analysis = analyze_tantivy_query("title:runtime^2 OR body:owner_snapshot");
        assert_eq!(analysis.selector_queries, ["owner_snapshot"]);
        assert!(analysis.selector_projection_complete);
    }

    #[test]
    fn body_conjunction_remains_one_symbol_posting_intersection() {
        let analysis = analyze_tantivy_query("title:runtime AND body:owner AND body:snapshot");
        assert_eq!(analysis.selector_queries, ["owner snapshot"]);
        assert!(analysis.selector_projection_complete);
    }

    #[test]
    fn negative_or_structured_body_fails_closed_for_selector_projection() {
        let negative = analyze_tantivy_query("title:runtime AND -body:legacy");
        assert!(negative.selector_queries.is_empty());
        assert!(!negative.selector_projection_complete);

        let regex = analyze_tantivy_query("title:runtime OR body:/owner.*/");
        assert!(regex.selector_queries.is_empty());
        assert!(!regex.selector_projection_complete);
    }
}
