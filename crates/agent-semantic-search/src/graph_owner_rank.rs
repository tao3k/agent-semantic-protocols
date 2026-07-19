use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    path::Path,
};

use crate::{GraphProjectionCandidate, graph_path_is_under, graph_project_submodule_paths};

/// Request for the Rust graph-owner ranking engine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphOwnerRankRequest {
    /// Candidate set projected from graph/search evidence.
    pub candidates: Vec<GraphOwnerRankCandidate>,
    /// User query terms used to derive language-neutral ranking axes.
    pub query_terms: Vec<String>,
    /// Workspace submodule or package-root paths used as topology evidence.
    pub submodule_paths: Vec<String>,
    /// Merkle source authority from which graph candidates were derived.
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
}

/// Public candidate shape for graph-owner ranking reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphOwnerRankCandidate {
    /// Candidate owner path.
    pub path: String,
    /// Candidate symbol evidence.
    pub symbol: String,
    /// Candidate text evidence.
    pub text: String,
    /// Candidate source/provenance label.
    pub source: String,
    /// Candidate confidence/provenance strength label.
    pub confidence: String,
}

impl GraphOwnerRankCandidate {
    /// Build a graph-owner rank candidate from public evidence fields.
    #[must_use]
    pub fn new(
        path: impl Into<String>,
        symbol: impl Into<String>,
        text: impl Into<String>,
        source: impl Into<String>,
        confidence: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            symbol: symbol.into(),
            text: text.into(),
            source: source.into(),
            confidence: confidence.into(),
        }
    }
}

impl From<&GraphProjectionCandidate> for GraphOwnerRankCandidate {
    fn from(candidate: &GraphProjectionCandidate) -> Self {
        Self::new(
            &candidate.path,
            &candidate.symbol,
            &candidate.text,
            &candidate.source,
            &candidate.confidence,
        )
    }
}

/// Full Rust-computed graph-owner ranking report for analysis consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphOwnerRankReport {
    /// Normalized query axes used by the graph owner ranker.
    pub query_axes: Vec<String>,
    /// Owners with computed scores in final order.
    pub ranked_owners: Vec<GraphOwnerRankedOwner>,
    /// Merkle source authority used for this graph projection.
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    /// Content address for this disposable graph/rank artifact.
    pub graph_artifact_digest: String,
}

/// Ranked owner path with graph-owner score evidence attached.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphOwnerRankedOwner {
    /// Ranked owner path.
    pub path: String,
    /// Path-derived package/root cluster used by the ranker.
    pub package_root: String,
    /// Matching submodule path when topology evidence applies.
    pub topology_submodule_path: Option<String>,
    /// Rust-computed score components used for ordering.
    pub score: GraphOwnerRankScore,
    /// Query axes matched by this owner.
    pub matched_query_axes: Vec<String>,
    /// Distinct symbols observed for this owner.
    pub symbols: Vec<String>,
}

/// Graph-owner score components computed in Rust.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct GraphOwnerRankScore {
    /// Weighted summary for analysis and report consumers.
    pub total: usize,
    /// Number of query axes covered by the owner's package cluster.
    pub package_query_axis_count: usize,
    /// Number of query axes covered by the owner's topology cluster.
    pub topology_query_axis_count: usize,
    /// Number of query axes covered by the owner itself.
    pub query_axis_count: usize,
    /// Number of local hits under a declared topology path.
    pub topology_local_hits: usize,
    /// Number of parser, finder, provider, or source-index local hits.
    pub parser_finder_local_hits: usize,
    /// Number of path-evidence hits.
    pub path_hits: usize,
    /// Number of distinct symbols observed for this owner.
    pub symbol_count: usize,
    /// Number of candidates merged into this owner.
    pub local_hits: usize,
}

#[derive(Debug)]
struct OwnerRank {
    path: String,
    package_root: String,
    topology_submodule_path: Option<String>,
    package_query_axis_count: usize,
    topology_query_axis_count: usize,
    topology_local_hits: usize,
    first_index: usize,
    local_hits: usize,
    parser_finder_local_hits: usize,
    path_hits: usize,
    query_axis_terms: Vec<String>,
    path_query_axis_terms: Vec<String>,
    symbols: Vec<String>,
}

struct PreparedGraphOwnerRankCandidate<'a> {
    candidate: &'a GraphOwnerRankCandidate,
    package_root: String,
    matched_query_axes: Vec<String>,
    matched_path_query_axes: Vec<String>,
    matching_submodule_path: Option<&'a str>,
}

pub fn ranked_graph_owner_paths_with_topology(
    candidates: &[GraphProjectionCandidate],
    query_terms: &[String],
    workspace_root: Option<&Path>,
) -> Vec<String> {
    let submodule_paths = workspace_root
        .map(graph_project_submodule_paths)
        .unwrap_or_default();
    ranked_graph_owner_paths_for_submodule_paths(candidates, query_terms, &submodule_paths)
}

/// Rank graph-owner candidates and return a complete Rust score report.
#[must_use]
pub fn rank_graph_owner_report(request: GraphOwnerRankRequest) -> GraphOwnerRankReport {
    let query_axes = owner_rank_query_axes(&request.query_terms);
    let mut ranks = owner_rank_entries(
        &request.candidates,
        query_axes.as_slice(),
        &request.submodule_paths,
    );
    ranks.sort_unstable_by(owner_rank_compare);
    let query_digest =
        agent_semantic_content_identity::hash_blob(query_axes.join("\0").as_bytes()).value;
    let mut submodule_paths = request.submodule_paths.clone();
    submodule_paths.sort_unstable();
    let submodule_digest = agent_semantic_content_identity::hash_blob(
        submodule_paths.join("\0").as_bytes(),
    )
    .value;
    let graph_artifact_digest = agent_semantic_content_identity::hash_derived_artifact_key(
        agent_semantic_content_identity::DerivedArtifactKeyInput {
            artifact_kind: "graph-owner-rank",
            schema_id: "asp.graph-owner-rank.v1",
            snapshot_root: &request.source_snapshot.root_digest,
            provider_digest: &request.source_snapshot.provider_digest,
            parameters: &[
                ("queryAxesDigest", query_digest.as_str()),
                ("submodulePathsDigest", submodule_digest.as_str()),
            ],
        },
    )
    .value;
    GraphOwnerRankReport {
        query_axes,
        ranked_owners: ranks.into_iter().map(graph_owner_ranked_owner).collect(),
        source_snapshot: request.source_snapshot,
        graph_artifact_digest,
    }
}

pub fn ranked_graph_owner_paths_for_submodule_paths<'a>(
    candidates: &[GraphProjectionCandidate],
    query_terms: &[String],
    submodule_paths: &'a [String],
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> Vec<String> {
    rank_graph_owner_report(GraphOwnerRankRequest {
        candidates: candidates
            .iter()
            .map(GraphOwnerRankCandidate::from)
            .collect(),
        query_terms: query_terms.to_vec(),
        submodule_paths: submodule_paths.to_vec(),
        source_snapshot: source_snapshot.clone(),
    })
    .ranked_owners
    .into_iter()
    .map(|owner| owner.path)
    .collect()
}

fn owner_rank_entries(
    candidates: &[GraphOwnerRankCandidate],
    query_axes: &[String],
    submodule_paths: &[String],
) -> Vec<OwnerRank> {
    let mut owner_ranks: HashMap<&str, OwnerRank> = HashMap::with_capacity(candidates.len());
    let prepared_candidates = candidates
        .iter()
        .map(|candidate| prepare_owner_rank_candidate(candidate, query_axes, submodule_paths))
        .collect::<Vec<_>>();
    let package_axes = package_query_axes(&prepared_candidates);
    let topology_axes = topology_query_axes(&prepared_candidates);
    prepared_candidates
        .into_iter()
        .enumerate()
        .for_each(|(index, prepared_candidate)| {
            match owner_ranks.entry(prepared_candidate.candidate.path.as_str()) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(new_owner_rank(prepared_candidate, index));
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    update_owner_rank(entry.get_mut(), &prepared_candidate);
                }
            }
        });
    owner_ranks.values_mut().for_each(|rank| {
        rank.package_query_axis_count = package_axes
            .get(&rank.package_root)
            .map(HashSet::len)
            .unwrap_or_default();
        rank.topology_query_axis_count = rank
            .topology_submodule_path
            .as_deref()
            .and_then(|submodule_path| topology_axes.get(submodule_path))
            .map(HashSet::len)
            .unwrap_or_default();
    });
    owner_ranks.into_values().collect()
}

fn new_owner_rank(
    prepared_candidate: PreparedGraphOwnerRankCandidate<'_>,
    first_index: usize,
) -> OwnerRank {
    let candidate = prepared_candidate.candidate;
    let symbols = if candidate.symbol.trim().is_empty() {
        Vec::new()
    } else {
        vec![candidate.symbol.clone()]
    };
    OwnerRank {
        path: candidate.path.clone(),
        package_root: prepared_candidate.package_root,
        topology_submodule_path: prepared_candidate
            .matching_submodule_path
            .map(str::to_owned),
        package_query_axis_count: 0,
        topology_query_axis_count: 0,
        topology_local_hits: usize::from(prepared_candidate.matching_submodule_path.is_some()),
        first_index,
        local_hits: 1,
        parser_finder_local_hits: usize::from(is_parser_finder_local_candidate(candidate)),
        path_hits: usize::from(is_path_evidence_candidate(candidate)),
        query_axis_terms: prepared_candidate.matched_query_axes,
        path_query_axis_terms: prepared_candidate.matched_path_query_axes,
        symbols,
    }
}

fn update_owner_rank(
    rank: &mut OwnerRank,
    prepared_candidate: &PreparedGraphOwnerRankCandidate<'_>,
) {
    let candidate = prepared_candidate.candidate;
    rank.local_hits += 1;
    if prepared_candidate.matching_submodule_path.is_some() {
        rank.topology_local_hits += 1;
    }
    if !candidate.symbol.trim().is_empty() {
        push_unique(&mut rank.symbols, &candidate.symbol);
    }
    if is_parser_finder_local_candidate(candidate) {
        rank.parser_finder_local_hits += 1;
    }
    if is_path_evidence_candidate(candidate) {
        rank.path_hits += 1;
    }
    prepared_candidate
        .matched_query_axes
        .iter()
        .for_each(|axis| push_unique(&mut rank.query_axis_terms, axis));
    prepared_candidate
        .matched_path_query_axes
        .iter()
        .for_each(|axis| push_unique(&mut rank.path_query_axis_terms, axis));
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn matched_query_axes(
    candidate: &GraphOwnerRankCandidate,
    normalized_path: &str,
    query_axes: &[String],
) -> Vec<String> {
    if query_axes.is_empty() {
        return Vec::new();
    }
    let evidence = owner_rank_evidence(candidate, normalized_path);
    query_axes
        .iter()
        .filter(|axis| evidence.contains(axis.as_str()))
        .cloned()
        .collect()
}

type OwnerRankSortKey<'a> = (
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    bool,
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
    usize,
    &'a str,
);

fn graph_owner_ranked_owner(mut rank: OwnerRank) -> GraphOwnerRankedOwner {
    let score = graph_owner_rank_score(&rank);
    rank.query_axis_terms.sort();
    rank.symbols.sort();
    GraphOwnerRankedOwner {
        path: rank.path,
        package_root: rank.package_root,
        topology_submodule_path: rank.topology_submodule_path,
        score,
        matched_query_axes: rank.query_axis_terms,
        symbols: rank.symbols,
    }
}

fn graph_owner_rank_score(rank: &OwnerRank) -> GraphOwnerRankScore {
    let package_query_axis_count = rank.package_query_axis_count.min(16);
    let topology_query_axis_count = rank.topology_query_axis_count.min(16);
    let query_axis_count = rank.query_axis_terms.len().min(16);
    let topology_local_hits = rank.topology_local_hits.min(12);
    let parser_finder_local_hits = rank.parser_finder_local_hits.min(12);
    let path_hits = rank.path_hits.min(8);
    let symbol_count = rank.symbols.len().min(12);
    let local_hits = rank.local_hits.min(12);
    GraphOwnerRankScore {
        total: package_query_axis_count * 10_000
            + topology_query_axis_count * 5_000
            + query_axis_count * 2_500
            + topology_local_hits * 1_000
            + parser_finder_local_hits * 500
            + path_hits * 250
            + symbol_count * 100
            + local_hits,
        package_query_axis_count,
        topology_query_axis_count,
        query_axis_count,
        topology_local_hits,
        parser_finder_local_hits,
        path_hits,
        symbol_count,
        local_hits,
    }
}

fn owner_rank_compare(left: &OwnerRank, right: &OwnerRank) -> std::cmp::Ordering {
    owner_rank_sort_key(left).cmp(&owner_rank_sort_key(right))
}

fn owner_rank_sort_key(rank: &OwnerRank) -> OwnerRankSortKey<'_> {
    (
        Reverse(rank.package_query_axis_count.min(16)),
        Reverse(rank.topology_query_axis_count.min(16)),
        Reverse(rank.query_axis_terms.len()),
        owner_path_is_test(&rank.path),
        Reverse(rank.path_query_axis_terms.len()),
        Reverse(rank.topology_local_hits.min(12)),
        Reverse(rank.parser_finder_local_hits.min(12)),
        Reverse(rank.path_hits.min(8)),
        Reverse(rank.symbols.len().min(12)),
        Reverse(rank.local_hits.min(12)),
        rank.first_index,
        rank.path.as_str(),
    )
}

fn prepare_owner_rank_candidate<'a>(
    candidate: &'a GraphOwnerRankCandidate,
    query_axes: &[String],
    submodule_paths: &'a [String],
) -> PreparedGraphOwnerRankCandidate<'a> {
    let normalized_path = candidate.path.to_ascii_lowercase();
    PreparedGraphOwnerRankCandidate {
        candidate,
        package_root: owner_rank_package_root(&candidate.path),
        matched_query_axes: matched_query_axes(candidate, &normalized_path, query_axes),
        matched_path_query_axes: matched_path_query_axes(&normalized_path, query_axes),
        matching_submodule_path: submodule_paths
            .iter()
            .find(|submodule_path| graph_path_is_under(&candidate.path, submodule_path))
            .map(String::as_str),
    }
}

fn matched_path_query_axes(normalized_path: &str, query_axes: &[String]) -> Vec<String> {
    query_axes
        .iter()
        .filter(|axis| normalized_path.contains(axis.as_str()))
        .cloned()
        .collect()
}

fn owner_path_is_test(path: &str) -> bool {
    path.starts_with("tests/")
        || path.contains("/tests/")
        || path.ends_with("_test.rs")
        || path.ends_with("_tests.rs")
}

fn package_query_axes(
    candidates: &[PreparedGraphOwnerRankCandidate<'_>],
) -> HashMap<String, HashSet<String>> {
    let mut package_axes: HashMap<String, HashSet<String>> = HashMap::new();
    candidates.iter().for_each(|candidate| {
        candidate
            .matched_query_axes
            .iter()
            .cloned()
            .for_each(|axis| {
                package_axes
                    .entry(candidate.package_root.clone())
                    .or_default()
                    .insert(axis);
            });
    });
    package_axes
}

fn topology_query_axes(
    candidates: &[PreparedGraphOwnerRankCandidate<'_>],
) -> HashMap<String, HashSet<String>> {
    let mut topology_axes: HashMap<String, HashSet<String>> = HashMap::new();
    candidates.iter().for_each(|candidate| {
        let Some(submodule_path) = candidate.matching_submodule_path else {
            return;
        };
        candidate
            .matched_query_axes
            .iter()
            .cloned()
            .for_each(|axis| {
                topology_axes
                    .entry(submodule_path.to_owned())
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

fn owner_rank_evidence(candidate: &GraphOwnerRankCandidate, normalized_path: &str) -> String {
    let mut evidence = String::with_capacity(
        normalized_path.len() + candidate.symbol.len() + candidate.text.len() + 2,
    );
    evidence.push_str(normalized_path);
    evidence.push(' ');
    evidence.extend(
        candidate
            .symbol
            .chars()
            .map(|character| character.to_ascii_lowercase()),
    );
    evidence.push(' ');
    evidence.extend(
        candidate
            .text
            .chars()
            .map(|character| character.to_ascii_lowercase()),
    );
    evidence
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

fn is_parser_finder_local_candidate(candidate: &GraphOwnerRankCandidate) -> bool {
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

fn is_path_evidence_candidate(candidate: &GraphOwnerRankCandidate) -> bool {
    matches!(
        candidate.source.as_str(),
        "fd-query" | "finder-path" | "package-path-query" | "query-anchor"
    ) || matches!(
        candidate.confidence.as_str(),
        "path" | "package-path" | "query-anchor"
    )
}
