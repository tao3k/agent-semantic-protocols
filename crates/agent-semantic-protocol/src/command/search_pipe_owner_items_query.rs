//! Owner-local query terms for graph-turbo owner-items handoff.

use super::search_pipe_model::Candidate;
use super::search_pipe_quality_model::SearchPipeQuality;

pub(super) fn owner_items_query_terms(
    quality: &SearchPipeQuality,
    candidates: &[Candidate],
    owner: &str,
) -> Option<Vec<String>> {
    let mut query_terms = Vec::new();
    let candidate_symbols = owner_local_candidate_symbols(candidates, owner);
    let semantic_terms = quality
        .concept_terms
        .iter()
        .chain(quality.owner_seed_terms.iter())
        .filter(|term| !weak_owner_items_action_term(term))
        .collect::<Vec<_>>();
    let owner_seed_terms = quality
        .owner_seed_terms
        .iter()
        .filter(|term| !weak_owner_items_action_term(term))
        .collect::<Vec<_>>();
    let concept_terms = quality
        .concept_terms
        .iter()
        .filter(|term| !weak_owner_items_action_term(term))
        .collect::<Vec<_>>();

    for term in &semantic_terms {
        if owner_path_axis_term(owner, term) {
            continue;
        }
        for variant in owner_items_term_variants(term) {
            push_owner_items_query_term(&mut query_terms, variant);
        }
    }
    for term in owner_seed_terms
        .iter()
        .filter(|term| !owner_path_axis_term(owner, term))
    {
        push_owner_items_query_term(&mut query_terms, (*term).to_string());
    }
    for symbol in candidate_symbols
        .iter()
        .filter(|symbol| !owner_path_axis_term(owner, symbol))
    {
        push_owner_items_query_term(&mut query_terms, (*symbol).clone());
    }
    for term in concept_terms
        .iter()
        .filter(|term| !owner_path_axis_term(owner, term))
    {
        push_owner_items_query_term(&mut query_terms, (*term).to_string());
    }
    let query_terms = query_terms.into_iter().take(6).collect::<Vec<_>>();
    (!query_terms.is_empty()).then_some(query_terms)
}

fn owner_local_candidate_symbols(candidates: &[Candidate], owner: &str) -> Vec<String> {
    candidates
        .iter()
        .filter(|candidate| candidate.path == owner)
        .map(|candidate| candidate.symbol.clone())
        .filter(|symbol| !weak_owner_items_action_term(symbol))
        .collect()
}

fn owner_path_axis_term(owner: &str, term: &str) -> bool {
    let lower_owner = owner.to_ascii_lowercase();
    owner_items_term_axes(term)
        .into_iter()
        .any(|axis| axis.len() >= 3 && lower_owner.contains(axis.as_str()))
}

fn owner_items_term_axes(term: &str) -> Vec<String> {
    let mut axes = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    for character in term.chars() {
        if !character.is_ascii_alphanumeric() {
            push_owner_items_axis(&mut axes, &mut current);
            previous = None;
            continue;
        }
        if character.is_ascii_uppercase()
            && previous
                .is_some_and(|previous| previous.is_ascii_lowercase() || previous.is_ascii_digit())
        {
            push_owner_items_axis(&mut axes, &mut current);
        }
        current.push(character.to_ascii_lowercase());
        previous = Some(character);
    }
    push_owner_items_axis(&mut axes, &mut current);
    axes
}

fn owner_items_term_variants(term: &str) -> Vec<String> {
    let lower = term.to_ascii_lowercase();
    let mut variants = Vec::new();
    if let Some(stem) = lower.strip_suffix("ing")
        && stem.len() >= 4
    {
        variants.push(format!("{stem}ed"));
    }
    variants
}

fn push_owner_items_query_term(terms: &mut Vec<String>, term: String) {
    if term.len() >= 3
        && usable_owner_items_query_term(&term)
        && !weak_owner_items_action_term(&term)
        && !terms.iter().any(|seen| seen == &term)
    {
        terms.push(term);
    }
}

fn push_owner_items_axis(axes: &mut Vec<String>, current: &mut String) {
    if current.len() >= 3 && !axes.iter().any(|axis| axis.as_str() == current.as_str()) {
        axes.push(current.clone());
    }
    current.clear();
}

fn usable_owner_items_query_term(term: &str) -> bool {
    !term.starts_with('_')
        && !term.starts_with('[')
        && term
            .chars()
            .all(|ch| ch == '.' || ch == '_' || ch.is_ascii_alphanumeric())
}

fn weak_owner_items_action_term(term: &str) -> bool {
    matches!(
        term.to_ascii_lowercase().as_str(),
        "through"
            | "smoke"
            | "dev"
            | "dependency"
            | "dependencies"
            | "weak"
            | "natural"
            | "term"
            | "terms"
    )
}
