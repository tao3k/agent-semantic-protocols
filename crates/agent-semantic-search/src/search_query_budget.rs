//! Query-shape budget gates for ASP-owned search wrappers.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchQueryBudgetBlock {
    pub reason: &'static str,
    pub generic_terms: Vec<String>,
    pub term_count: usize,
}

#[must_use]
pub fn search_query_budget_block(
    query: &str,
    scopes: &[PathBuf],
    explicit_filters: bool,
) -> Option<SearchQueryBudgetBlock> {
    let terms = search_query_terms(query);
    search_terms_budget_block(&terms, scopes, explicit_filters)
}

#[must_use]
pub fn search_terms_budget_block(
    terms: &[String],
    scopes: &[PathBuf],
    explicit_filters: bool,
) -> Option<SearchQueryBudgetBlock> {
    if explicit_filters || terms.is_empty() || !broad_scope(scopes) {
        return None;
    }
    let generic_terms = terms
        .iter()
        .filter(|term| generic_search_term(term))
        .cloned()
        .collect::<Vec<_>>();
    let all_generic = terms.len() >= 2 && generic_terms.len() == terms.len();
    let lacks_specific_anchor =
        terms.len() >= 4 && !terms.iter().any(|term| specific_search_term(term));
    let too_many_terms = terms.len() >= 10 && generic_terms.len() >= 5;
    let generic_dominated =
        terms.len() >= 5 && generic_terms.len() >= 5 && generic_terms.len() * 2 >= terms.len();
    let displayed_generic_terms = if generic_terms.is_empty() {
        terms.iter().take(6).cloned().collect()
    } else {
        generic_terms
    };
    (all_generic || lacks_specific_anchor || too_many_terms || generic_dominated).then_some(
        SearchQueryBudgetBlock {
            reason: if lacks_specific_anchor && !all_generic {
                "query-needs-specific-anchor"
            } else {
                "query-too-broad"
            },
            generic_terms: displayed_generic_terms,
            term_count: terms.len(),
        },
    )
}

#[must_use]
pub fn search_query_terms(query: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    query
        .split(|character: char| character == '|' || character == ',' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .filter(|term| seen.insert(term.clone()))
        .collect()
}

#[must_use]
pub fn specific_search_term(term: &str) -> bool {
    term.contains('.')
        || term.contains('/')
        || term.chars().any(|character| character.is_ascii_digit())
        || term.contains('-')
        || term.contains('_')
}

fn broad_scope(scopes: &[PathBuf]) -> bool {
    scopes.is_empty()
        || scopes.iter().any(|scope| {
            scope == Path::new(".")
                || scope.components().count() <= 1
                || scope.extension().is_none() && scope.components().count() <= 2
        })
}

fn generic_search_term(term: &str) -> bool {
    if specific_search_term(term) {
        return false;
    }
    term.chars().count() <= 2
        || matches!(
            term,
            "self"
                | "old"
                | "new"
                | "source"
                | "helper"
                | "helpers"
                | "comment"
                | "comments"
                | "style"
                | "quality"
                | "doc"
                | "docs"
                | "example"
                | "examples"
                | "test"
                | "tests"
                | "migrate"
                | "migration"
                | "apply"
                | "engineer"
                | "engineering"
                | "search"
                | "wrapper"
                | "backend"
                | "interface"
                | "query"
                | "budget"
                | "gate"
                | "broad"
                | "generic"
                | "input"
                | "block"
                | "provider"
                | "finder"
                | "coverage"
                | "performance"
        )
}
