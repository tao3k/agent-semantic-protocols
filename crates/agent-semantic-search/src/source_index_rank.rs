//! Source-index query tokenization and candidate ranking.

use std::cmp::Reverse;
use std::collections::BTreeSet;

/// Search-time candidate shape for source-index ranking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexRankCandidate {
    pub ordinal: usize,
    pub path: String,
    pub query_keys: Vec<String>,
}

/// Tokenize a source-index lookup query into stable search terms.
#[must_use]
pub fn source_index_lookup_terms(query: &str) -> Vec<String> {
    let mut terms = BTreeSet::new();
    let trimmed = query.trim();
    if !trimmed.is_empty() {
        terms.insert(trimmed.to_ascii_lowercase());
    }
    for term in query
        .split(|character: char| {
            !(character == '_'
                || character == '-'
                || character == ':'
                || character == '/'
                || character.is_ascii_alphanumeric())
        })
        .map(str::trim)
        .filter(|term| !term.is_empty())
    {
        terms.insert(term.to_ascii_lowercase());
    }
    terms.into_iter().collect()
}

/// Rank source-index candidates by query-axis coverage while preserving stable
/// DB order within equal coverage.
#[must_use]
pub fn rank_source_index_candidates(
    candidates: Vec<SourceIndexRankCandidate>,
    query: &str,
) -> Vec<SourceIndexRankCandidate> {
    let terms = source_index_lookup_terms(query);
    let mut indexed = candidates.into_iter().enumerate().collect::<Vec<_>>();
    indexed.sort_by_key(|(index, candidate)| {
        source_index_candidate_sort_key(candidate, terms.as_slice(), *index)
    });
    indexed
        .into_iter()
        .map(|(_index, candidate)| candidate)
        .collect()
}

/// Reorder source-index candidates by the shared source-index ranking policy
/// without making the search crate depend on the concrete DB/client DTO.
#[must_use]
pub fn reorder_source_index_candidates<Candidate>(
    candidates: Vec<Candidate>,
    query: &str,
    mut path: impl FnMut(&Candidate) -> String,
    mut query_keys: impl FnMut(&Candidate) -> Vec<String>,
) -> Vec<Candidate> {
    let ranked = rank_source_index_candidates(
        candidates
            .iter()
            .enumerate()
            .map(|(ordinal, candidate)| SourceIndexRankCandidate {
                ordinal,
                path: path(candidate),
                query_keys: query_keys(candidate),
            })
            .collect(),
        query,
    );
    let mut candidates = candidates
        .into_iter()
        .map(Some)
        .collect::<Vec<Option<Candidate>>>();
    ranked
        .into_iter()
        .filter_map(|candidate| candidates.get_mut(candidate.ordinal).and_then(Option::take))
        .collect()
}

type SourceIndexCandidateSortKey = (Reverse<usize>, usize);

fn source_index_candidate_sort_key(
    candidate: &SourceIndexRankCandidate,
    terms: &[String],
    index: usize,
) -> SourceIndexCandidateSortKey {
    (
        Reverse(source_index_candidate_query_axis_coverage(candidate, terms)),
        index,
    )
}

fn source_index_candidate_query_axis_coverage(
    candidate: &SourceIndexRankCandidate,
    terms: &[String],
) -> usize {
    let normalized_path = candidate.path.to_ascii_lowercase();
    terms
        .iter()
        .filter(|term| {
            !term.is_empty()
                && (normalized_path.contains(term.as_str())
                    || candidate
                        .query_keys
                        .iter()
                        .any(|key| key.contains(term.as_str())))
        })
        .count()
}
