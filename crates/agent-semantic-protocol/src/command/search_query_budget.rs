//! Query-shape budget gates for ASP-owned search wrappers.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SearchQueryBudgetBlock {
    pub(super) reason: &'static str,
    pub(super) generic_terms: Vec<String>,
    pub(super) term_count: usize,
}

pub(super) fn search_query_budget_block(
    query: &str,
    scopes: &[PathBuf],
    explicit_filters: bool,
) -> Option<SearchQueryBudgetBlock> {
    let terms = search_query_terms(query);
    search_terms_budget_block(&terms, scopes, explicit_filters)
}

pub(super) fn search_terms_budget_block(
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
    let too_many_terms = terms.len() >= 10 && generic_terms.len() >= 5;
    let generic_dominated =
        terms.len() >= 5 && generic_terms.len() >= 5 && generic_terms.len() * 2 >= terms.len();
    let displayed_generic_terms = if generic_terms.is_empty() {
        terms.iter().take(6).cloned().collect()
    } else {
        generic_terms
    };
    (too_many_terms || generic_dominated).then_some(SearchQueryBudgetBlock {
        reason: "query-too-broad",
        generic_terms: displayed_generic_terms,
        term_count: terms.len(),
    })
}

pub(super) fn search_rg_terms_budget_block(
    terms: &[String],
    scopes: &[PathBuf],
    explicit_filters: bool,
) -> Option<SearchQueryBudgetBlock> {
    if explicit_filters || terms.len() < 5 || !broad_scope(scopes) {
        return None;
    }
    Some(SearchQueryBudgetBlock {
        reason: "query-too-broad",
        generic_terms: terms.iter().take(6).cloned().collect(),
        term_count: terms.len(),
    })
}

pub(super) fn search_query_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for term in query
        .split(|character: char| character == '|' || character == ',' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
    {
        if !terms.iter().any(|seen| seen == &term) {
            terms.push(term);
        }
    }
    terms
}

fn broad_scope(scopes: &[PathBuf]) -> bool {
    scopes.is_empty()
        || scopes.iter().any(|scope| {
            scope == Path::new(".")
                || scope.components().count() <= 1
                || scope.extension().is_none() && scope.components().count() <= 2
        })
}

pub(super) fn specific_search_term(term: &str) -> bool {
    term.contains('.')
        || term.contains('/')
        || term.chars().any(|character| character.is_ascii_digit())
        || term.contains('-')
        || term.contains('_')
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
