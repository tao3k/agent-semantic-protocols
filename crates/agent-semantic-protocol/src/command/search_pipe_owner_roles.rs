//! Structural owner role scoring for search pipe owner selection.

use super::search_pipe_quality::SearchPipeQuality;
use super::search_query_wrapper_model::FdQueryPreview;

pub(super) fn suppress_low_cohesion_secondary_owner(
    quality: &SearchPipeQuality,
    preview: Option<&FdQueryPreview>,
) -> bool {
    if quality.package_cohesion != "low"
        || has_strong_secondary_owner_intent(
            quality
                .context_terms
                .iter()
                .map(String::as_str)
                .chain(quality.owner_seed_terms.iter().map(String::as_str))
                .chain(quality.concept_terms.iter().map(String::as_str)),
        )
    {
        return false;
    }
    let Some(owner) = quality
        .best_owner
        .as_ref()
        .map(|coverage| coverage.owner.as_str())
    else {
        return false;
    };
    if strong_owner_seed_count(quality) >= 2
        && preview.is_some_and(|preview| {
            preview
                .owner_candidates
                .iter()
                .any(|candidate| candidate == owner)
        })
    {
        return false;
    }
    secondary_like_owner(owner)
}

pub(super) fn suppress_low_cohesion_weak_axis_owner(
    quality: &SearchPipeQuality,
    query_terms: &[String],
) -> bool {
    if quality.package_cohesion != "low"
        || query_terms.len() <= 2
        || has_strong_secondary_owner_intent(
            quality
                .context_terms
                .iter()
                .map(String::as_str)
                .chain(quality.owner_seed_terms.iter().map(String::as_str))
                .chain(quality.concept_terms.iter().map(String::as_str)),
        )
    {
        return false;
    }
    let Some(owner) = quality.best_owner.as_ref() else {
        return false;
    };
    let mut evidence = owner.owner.to_ascii_lowercase();
    for term in &owner.matched {
        evidence.push('\n');
        evidence.push_str(&term.to_ascii_lowercase());
    }
    let covered = query_terms
        .iter()
        .filter(|term| evidence.contains(&term.to_ascii_lowercase()))
        .count();
    covered * 2 < query_terms.len()
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

pub(super) fn secondary_like_owner(owner: &str) -> bool {
    owner
        .split(['/', '\\', '.', '-', '_'])
        .any(|part| secondary_owner_role_token(part.to_ascii_lowercase().as_str()))
}

pub(super) fn has_strong_secondary_owner_intent<'a>(
    terms: impl IntoIterator<Item = &'a str>,
) -> bool {
    terms
        .into_iter()
        .filter(|term| secondary_owner_role_token(term.to_ascii_lowercase().as_str()))
        .take(2)
        .count()
        >= 2
}

fn secondary_owner_role_token(token: &str) -> bool {
    matches!(
        token,
        "test"
            | "tests"
            | "unittest"
            | "unittests"
            | "spec"
            | "specs"
            | "fixture"
            | "fixtures"
            | "baseline"
            | "baselines"
            | "case"
            | "cases"
            | "template"
            | "templates"
            | "example"
            | "examples"
            | "sample"
            | "samples"
            | "demo"
            | "demos"
            | "bench"
            | "benches"
            | "benchmark"
            | "benchmarks"
    )
}
