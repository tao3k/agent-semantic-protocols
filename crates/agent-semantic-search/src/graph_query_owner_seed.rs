use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

use crate::GraphProjectionCandidate;

type QueryOwnerSeedSortKey = (
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    String,
);

#[derive(Default)]
struct OwnerSeedEvidence {
    evidence: String,
    local_evidence_hits: usize,
    candidate_count: usize,
}

pub fn graph_has_package_path_candidate(
    candidates: &[GraphProjectionCandidate],
    query_terms: &[String],
) -> bool {
    candidates.iter().any(|candidate| {
        (is_explicit_package_path_candidate(candidate)
            || candidate.source == "finder-path"
            || candidate.source == "fd-query")
            && candidate_path_covers_package_term(candidate, query_terms)
    })
}

pub fn graph_query_owner_seed_paths(
    candidates: &[GraphProjectionCandidate],
    owners: &[String],
    budget: usize,
    query_terms: &[String],
) -> Vec<String> {
    if budget == 0 {
        return Vec::new();
    }
    let package_path_owners = package_path_seed_owners(candidates, owners, budget, query_terms);
    if !package_path_owners.is_empty() {
        return package_path_owners;
    }
    let query_axes = query_seed_axes(query_terms);
    let owner_evidence = owner_seed_evidence(candidates);
    let mut ranked_owners = owners.to_vec();
    ranked_owners
        .sort_by_key(|owner| query_owner_seed_sort_key(owner, &owner_evidence, &query_axes));
    ranked_owners.into_iter().take(budget).collect()
}

fn package_path_seed_owners(
    candidates: &[GraphProjectionCandidate],
    owners: &[String],
    budget: usize,
    query_terms: &[String],
) -> Vec<String> {
    let package_path_owners = candidates
        .iter()
        .filter(|candidate| {
            (is_explicit_package_path_candidate(candidate)
                || candidate.source == "finder-path"
                || candidate.source == "fd-query")
                && candidate_path_covers_package_term(candidate, query_terms)
        })
        .map(|candidate| candidate.path.clone())
        .collect::<HashSet<_>>();
    owners
        .iter()
        .filter(|owner| package_path_owners.contains(*owner))
        .take(budget)
        .cloned()
        .collect()
}

fn is_explicit_package_path_candidate(candidate: &GraphProjectionCandidate) -> bool {
    candidate.source == "package-path-query" || candidate.confidence == "package-path"
}

fn candidate_path_covers_package_term(
    candidate: &GraphProjectionCandidate,
    query_terms: &[String],
) -> bool {
    let lower_path = candidate.path.to_ascii_lowercase();
    query_terms.iter().any(|term| {
        let axes = split_query_seed_axes(term)
            .into_iter()
            .filter(|axis| axis.len() >= 2)
            .collect::<Vec<_>>();
        axes.len() >= 2
            && axes
                .iter()
                .filter(|axis| lower_path.contains(*axis))
                .count()
                >= 2
    })
}

fn query_owner_seed_sort_key(
    owner: &str,
    owner_evidence: &HashMap<String, OwnerSeedEvidence>,
    query_axes: &[String],
) -> QueryOwnerSeedSortKey {
    let evidence = owner_evidence.get(owner);
    (
        Reverse(owner_query_axis_hits(owner, evidence, query_axes)),
        Reverse(evidence.map_or(0, |evidence| evidence.local_evidence_hits)),
        Reverse(evidence.map_or(0, |evidence| evidence.candidate_count)),
        Reverse(owner.len()),
        owner.to_string(),
    )
}

fn owner_seed_evidence(
    candidates: &[GraphProjectionCandidate],
) -> HashMap<String, OwnerSeedEvidence> {
    let mut evidence_by_owner = HashMap::new();
    for candidate in candidates {
        let evidence = evidence_by_owner
            .entry(candidate.path.clone())
            .or_insert_with(|| OwnerSeedEvidence {
                evidence: candidate.path.to_ascii_lowercase(),
                ..OwnerSeedEvidence::default()
            });
        evidence.candidate_count += 1;
        evidence.evidence.push(' ');
        evidence
            .evidence
            .push_str(&candidate.symbol.to_ascii_lowercase());
        evidence.evidence.push(' ');
        evidence
            .evidence
            .push_str(&candidate.text.to_ascii_lowercase());
        if !is_path_only_evidence(candidate) {
            evidence.local_evidence_hits += 1;
        }
    }
    evidence_by_owner
}

fn is_path_only_evidence(candidate: &GraphProjectionCandidate) -> bool {
    matches!(
        candidate.source.as_str(),
        "finder-path" | "fd-query" | "rg-query" | "ingest"
    )
}

fn owner_query_axis_hits(
    owner: &str,
    evidence: Option<&OwnerSeedEvidence>,
    query_axes: &[String],
) -> usize {
    if query_axes.is_empty() {
        return 0;
    }
    let fallback_evidence = owner.to_ascii_lowercase();
    let evidence = evidence
        .map(|evidence| evidence.evidence.as_str())
        .unwrap_or(fallback_evidence.as_str());
    query_axes
        .iter()
        .filter(|axis| evidence.contains(axis.as_str()))
        .count()
}

fn query_seed_axes(query_terms: &[String]) -> Vec<String> {
    query_terms.iter().fold(Vec::new(), |mut axes, term| {
        split_query_seed_axes(term)
            .into_iter()
            .filter(|axis| axis.len() >= 2)
            .for_each(|axis| {
                if !axes.iter().any(|seen| seen == &axis) {
                    axes.push(axis);
                }
            });
        axes
    })
}

fn split_query_seed_axes(term: &str) -> Vec<String> {
    let mut axes = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    for character in term.chars() {
        if !character.is_ascii_alphanumeric() {
            push_query_seed_axis(&mut axes, &mut current);
            previous = None;
            continue;
        }
        if character.is_ascii_uppercase()
            && previous
                .is_some_and(|previous| previous.is_ascii_lowercase() || previous.is_ascii_digit())
        {
            push_query_seed_axis(&mut axes, &mut current);
        }
        current.push(character.to_ascii_lowercase());
        previous = Some(character);
    }
    push_query_seed_axis(&mut axes, &mut current);
    let raw_axis = term
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();
    if raw_axis.len() >= 2 && !axes.iter().any(|seen| seen == &raw_axis) {
        axes.push(raw_axis);
    }
    axes
}

fn push_query_seed_axis(axes: &mut Vec<String>, current: &mut String) {
    let axis = current.trim();
    if axis.len() >= 2 && !axes.iter().any(|seen| seen == axis) {
        axes.push(axis.to_string());
    }
    current.clear();
}
