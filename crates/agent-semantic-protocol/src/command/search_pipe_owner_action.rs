//! Owner-items action eligibility for search pipe frontiers.

use std::collections::BTreeSet;

use super::search_pipe_model::Candidate;
use super::search_pipe_owner_items_query::owner_items_query_terms;
use super::search_pipe_owner_roles::{
    has_strong_secondary_owner_intent, secondary_like_owner, suppress_low_cohesion_secondary_owner,
    suppress_low_cohesion_weak_axis_owner,
};
use super::search_pipe_quality_model::SearchPipeQuality;
use super::search_query_wrapper_model::FdQueryPreview;

pub(super) fn preferred_owner_items_handle(
    quality: &SearchPipeQuality,
    candidates: &[Candidate],
    preview: Option<&FdQueryPreview>,
) -> Option<String> {
    if quality.package_cohesion == "low" && quality.owner_seed_terms.is_empty() {
        return low_cohesion_role_owner_handle(quality, candidates, preview);
    }
    preview_owner_items_handle(quality, preview).or_else(|| {
        (!suppress_low_cohesion_secondary_owner(quality, preview))
            .then(|| owner_items_handle(quality, candidates))
            .flatten()
    })
}

pub(super) fn owner_items_handle(
    quality: &SearchPipeQuality,
    candidates: &[Candidate],
) -> Option<String> {
    let owner = quality.best_owner.as_ref()?.owner.as_str();
    let query_terms = owner_items_query_terms(quality, candidates, owner)?;
    if suppress_low_cohesion_weak_axis_owner(quality, &query_terms) {
        return None;
    }
    let query = query_terms.join("|");
    Some(format!("{owner}:{query}"))
}

pub(super) fn preview_owner_items_handle(
    quality: &SearchPipeQuality,
    preview: Option<&FdQueryPreview>,
) -> Option<String> {
    let preview = preview?;
    if quality.package_cohesion == "low" && strong_owner_seed_count(quality) < 2 {
        return None;
    }
    let owner = quality
        .best_owner
        .as_ref()
        .map(|coverage| coverage.owner.as_str())
        .filter(|owner| {
            preview
                .owner_candidates
                .iter()
                .any(|candidate| candidate == owner)
        })
        .or_else(|| preview.owner_candidates.first().map(String::as_str))?;
    let mut query_terms = Vec::new();
    query_terms.extend(quality.concept_terms.iter().cloned());
    query_terms.extend(quality.owner_seed_terms.iter().cloned());
    let query = unique_terms_without_weak_natural(query_terms, 6)?.join("|");
    Some(format!("{owner}:{query}"))
}

pub(super) fn usable_query_term(term: &str) -> bool {
    !term.starts_with('_')
        && !term.starts_with('[')
        && term
            .chars()
            .all(|ch| ch == '.' || ch == '_' || ch.is_ascii_alphanumeric())
}

pub(super) fn unique_terms_without_weak_natural(
    terms: Vec<String>,
    limit: usize,
) -> Option<Vec<String>> {
    unique_terms(
        terms
            .into_iter()
            .filter(|term| !weak_natural_action_term(term))
            .collect(),
        limit,
    )
}

pub(super) fn weak_natural_action_term(term: &str) -> bool {
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

fn low_cohesion_role_owner_handle(
    quality: &SearchPipeQuality,
    candidates: &[Candidate],
    preview: Option<&FdQueryPreview>,
) -> Option<String> {
    let owner = quality.best_owner.as_ref()?.owner.as_str();
    let preview = preview?;
    if !preview
        .owner_candidates
        .iter()
        .any(|candidate| candidate == owner)
    {
        return None;
    }
    let secondary_intent = has_strong_secondary_owner_intent(
        quality
            .context_terms
            .iter()
            .map(String::as_str)
            .chain(quality.owner_seed_terms.iter().map(String::as_str))
            .chain(quality.concept_terms.iter().map(String::as_str)),
    );
    let has_secondary_competitor = preview
        .owner_candidates
        .iter()
        .any(|candidate| candidate != owner && secondary_like_owner(candidate));
    if secondary_like_owner(owner) {
        if !secondary_intent {
            return None;
        }
    } else if !has_secondary_competitor
        && !secondary_intent
        && !single_preview_owner_has_strong_local_evidence(quality, preview, owner)
    {
        return None;
    }
    let handle = owner_items_handle(quality, candidates)?;
    if single_preview_owner_has_strong_local_evidence(quality, preview, owner) {
        return narrow_owner_items_handle(&handle, 2);
    }
    Some(handle)
}

fn single_preview_owner_has_strong_local_evidence(
    quality: &SearchPipeQuality,
    preview: &FdQueryPreview,
    owner: &str,
) -> bool {
    preview.owner_candidates.len() == 1
        && preview
            .owner_candidates
            .first()
            .is_some_and(|candidate| candidate == owner)
        && quality
            .best_owner
            .as_ref()
            .is_some_and(|coverage| coverage.matched.len() >= 4)
}

fn narrow_owner_items_handle(handle: &str, limit: usize) -> Option<String> {
    let (owner, query) = handle.split_once(':')?;
    let terms = query
        .split('|')
        .filter(|term| !term.is_empty())
        .take(limit)
        .collect::<Vec<_>>();
    (!terms.is_empty()).then(|| format!("{owner}:{}", terms.join("|")))
}

fn strong_owner_seed_count(quality: &SearchPipeQuality) -> usize {
    quality
        .owner_seed_terms
        .iter()
        .filter(|term| {
            quality
                .strong_matched
                .iter()
                .any(|matched| matched == *term)
        })
        .count()
}

fn unique_terms(terms: Vec<String>, limit: usize) -> Option<Vec<String>> {
    let mut seen = BTreeSet::new();
    let result = terms
        .into_iter()
        .filter(|term| usable_query_term(term))
        .filter(|term| seen.insert(term.clone()))
        .take(limit)
        .collect::<Vec<_>>();
    (!result.is_empty()).then_some(result)
}
