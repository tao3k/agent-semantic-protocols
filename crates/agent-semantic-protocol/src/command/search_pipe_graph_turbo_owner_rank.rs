//! Owner candidate ranking for graph-turbo seed construction.

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    path::Path,
};

use super::{
    search_pipe_graph_nodes::{path_is_under, project_submodule_paths},
    search_pipe_model::Candidate,
};

#[derive(Debug)]
struct OwnerRank {
    path: String,
    package_root: String,
    package_query_axis_count: usize,
    topology_query_axis_count: usize,
    topology_local_hits: usize,
    first_index: usize,
    local_hits: usize,
    parser_finder_local_hits: usize,
    path_hits: usize,
    query_axis_terms: HashSet<String>,
    symbols: HashSet<String>,
}

pub(super) fn ranked_candidate_paths_with_topology(
    candidates: &[Candidate],
    query_terms: &[String],
    workspace_root: Option<&Path>,
) -> Vec<String> {
    let submodule_paths = workspace_root
        .map(project_submodule_paths)
        .unwrap_or_default();
    ranked_candidate_paths_for_submodule_paths(candidates, query_terms, &submodule_paths)
}

fn ranked_candidate_paths_for_submodule_paths(
    candidates: &[Candidate],
    query_terms: &[String],
    submodule_paths: &[String],
) -> Vec<String> {
    let query_axes = owner_rank_query_axes(query_terms);
    let mut ranks = owner_rank_entries(candidates, &query_axes, submodule_paths)
        .into_values()
        .collect::<Vec<_>>();
    ranks.sort_by_key(owner_rank_sort_key);
    ranks.into_iter().map(|rank| rank.path).collect()
}

fn owner_rank_entries(
    candidates: &[Candidate],
    query_axes: &[String],
    submodule_paths: &[String],
) -> HashMap<String, OwnerRank> {
    let mut owner_ranks: HashMap<String, OwnerRank> = HashMap::new();
    let package_axes = package_query_axes(candidates, query_axes);
    let topology_axes = topology_query_axes(candidates, query_axes, submodule_paths);
    candidates
        .iter()
        .enumerate()
        .for_each(|(index, candidate)| {
            let rank = owner_ranks
                .entry(candidate.path.clone())
                .or_insert_with(|| new_owner_rank(candidate, index));
            update_owner_rank(rank, candidate, query_axes, submodule_paths);
        });
    owner_ranks.values_mut().for_each(|rank| {
        rank.package_query_axis_count = package_axes
            .get(&rank.package_root)
            .map(HashSet::len)
            .unwrap_or_default();
        rank.topology_query_axis_count = submodule_paths
            .iter()
            .find(|submodule_path| path_is_under(&rank.path, submodule_path))
            .and_then(|submodule_path| topology_axes.get(submodule_path))
            .map(HashSet::len)
            .unwrap_or_default();
    });
    owner_ranks
}

fn new_owner_rank(candidate: &Candidate, first_index: usize) -> OwnerRank {
    OwnerRank {
        path: candidate.path.clone(),
        package_root: owner_rank_package_root(&candidate.path),
        package_query_axis_count: 0,
        topology_query_axis_count: 0,
        topology_local_hits: 0,
        first_index,
        local_hits: 0,
        parser_finder_local_hits: 0,
        path_hits: 0,
        query_axis_terms: HashSet::new(),
        symbols: HashSet::new(),
    }
}

fn update_owner_rank(
    rank: &mut OwnerRank,
    candidate: &Candidate,
    query_axes: &[String],
    submodule_paths: &[String],
) {
    rank.local_hits += 1;
    if submodule_paths
        .iter()
        .any(|submodule_path| path_is_under(&candidate.path, submodule_path))
    {
        rank.topology_local_hits += 1;
    }
    if !candidate.symbol.trim().is_empty() {
        rank.symbols.insert(candidate.symbol.clone());
    }
    if is_parser_finder_local_candidate(candidate) {
        rank.parser_finder_local_hits += 1;
    }
    if is_path_evidence_candidate(candidate) {
        rank.path_hits += 1;
    }
    matched_query_axes(candidate, query_axes)
        .into_iter()
        .for_each(|axis| {
            rank.query_axis_terms.insert(axis);
        });
}

fn matched_query_axes(candidate: &Candidate, query_axes: &[String]) -> Vec<String> {
    if query_axes.is_empty() {
        return Vec::new();
    }
    let evidence = owner_rank_evidence(candidate);
    query_axes
        .iter()
        .filter(|axis| evidence.contains(axis.as_str()))
        .cloned()
        .collect()
}

type OwnerRankSortKey = (
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    usize,
    String,
);

fn owner_rank_sort_key(rank: &OwnerRank) -> OwnerRankSortKey {
    (
        Reverse(rank.package_query_axis_count.min(16)),
        Reverse(rank.topology_query_axis_count.min(16)),
        Reverse(rank.query_axis_terms.len()),
        Reverse(rank.topology_local_hits.min(12)),
        Reverse(rank.parser_finder_local_hits.min(12)),
        Reverse(rank.path_hits.min(8)),
        Reverse(rank.symbols.len().min(12)),
        Reverse(rank.local_hits.min(12)),
        rank.first_index,
        rank.path.clone(),
    )
}

fn package_query_axes(
    candidates: &[Candidate],
    query_axes: &[String],
) -> HashMap<String, HashSet<String>> {
    let mut package_axes: HashMap<String, HashSet<String>> = HashMap::new();
    candidates.iter().for_each(|candidate| {
        let package_root = owner_rank_package_root(&candidate.path);
        matched_query_axes(candidate, query_axes)
            .into_iter()
            .for_each(|axis| {
                package_axes
                    .entry(package_root.clone())
                    .or_default()
                    .insert(axis);
            });
    });
    package_axes
}

fn topology_query_axes(
    candidates: &[Candidate],
    query_axes: &[String],
    submodule_paths: &[String],
) -> HashMap<String, HashSet<String>> {
    let mut topology_axes: HashMap<String, HashSet<String>> = HashMap::new();
    if submodule_paths.is_empty() {
        return topology_axes;
    }
    candidates.iter().for_each(|candidate| {
        let Some(submodule_path) = submodule_paths
            .iter()
            .find(|submodule_path| path_is_under(&candidate.path, submodule_path))
        else {
            return;
        };
        matched_query_axes(candidate, query_axes)
            .into_iter()
            .for_each(|axis| {
                topology_axes
                    .entry(submodule_path.clone())
                    .or_default()
                    .insert(axis);
            });
    });
    topology_axes
}

fn owner_rank_package_root(path: &str) -> String {
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    match segments.as_slice() {
        ["packages", ecosystem, package, ..] => format!("packages/{ecosystem}/{package}"),
        [root, package, ..] if !is_single_root_owner_segment(root) => format!("{root}/{package}"),
        [root, ..] => (*root).to_string(),
        [] => ".".to_string(),
    }
}

fn is_single_root_owner_segment(segment: &str) -> bool {
    matches!(
        segment,
        "." | "src" | "tests" | "test" | "docs" | "schemas" | "fixtures"
    )
}

fn owner_rank_evidence(candidate: &Candidate) -> String {
    format!("{} {} {}", candidate.path, candidate.symbol, candidate.text).to_ascii_lowercase()
}

fn owner_rank_query_axes(query_terms: &[String]) -> Vec<String> {
    let mut axes = Vec::new();
    query_terms.iter().for_each(|term| {
        split_owner_rank_axis(term)
            .into_iter()
            .filter(|axis| axis.len() >= 2)
            .for_each(|axis| push_unique_axis(&mut axes, axis));
    });
    axes
}

fn split_owner_rank_axis(term: &str) -> Vec<String> {
    let mut axes = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    for character in term.chars() {
        if !character.is_ascii_alphanumeric() {
            push_owner_rank_axis(&mut axes, &mut current);
            previous = None;
            continue;
        }
        if character.is_ascii_uppercase()
            && previous
                .is_some_and(|previous| previous.is_ascii_lowercase() || previous.is_ascii_digit())
        {
            push_owner_rank_axis(&mut axes, &mut current);
        }
        current.push(character.to_ascii_lowercase());
        previous = Some(character);
    }
    push_owner_rank_axis(&mut axes, &mut current);
    let raw_axis = term
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();
    if raw_axis.len() >= 2 {
        push_unique_axis(&mut axes, raw_axis);
    }
    axes
}

fn push_owner_rank_axis(axes: &mut Vec<String>, current: &mut String) {
    if current.len() >= 2 {
        push_unique_axis(axes, current.clone());
    }
    current.clear();
}

fn push_unique_axis(axes: &mut Vec<String>, axis: String) {
    if !axes.iter().any(|seen| seen == &axis) {
        axes.push(axis);
    }
}

fn is_parser_finder_local_candidate(candidate: &Candidate) -> bool {
    matches!(
        candidate.source.as_str(),
        "fd-query" | "finder-path" | "package-path-query" | "query-anchor"
    ) || candidate.source.contains("finder")
        || candidate.source.contains("parser")
        || candidate.source.contains("provider")
        || candidate.source.contains("sourceIndex")
        || candidate.source.contains("source-index")
        || matches!(
            candidate.confidence.as_str(),
            "path" | "package-path" | "query-anchor" | "symbol" | "exact" | "high"
        )
}

fn is_path_evidence_candidate(candidate: &Candidate) -> bool {
    matches!(
        candidate.source.as_str(),
        "fd-query" | "finder-path" | "package-path-query" | "query-anchor"
    ) || matches!(
        candidate.confidence.as_str(),
        "path" | "package-path" | "query-anchor"
    )
}
