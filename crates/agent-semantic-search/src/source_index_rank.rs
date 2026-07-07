//! Source-index query tokenization and candidate ranking.

use std::borrow::Cow;
use std::collections::BTreeSet;

/// Search-time candidate shape for source-index ranking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexRankCandidate {
    pub ordinal: usize,
    pub path: String,
    pub query_keys: Vec<String>,
}

/// Request for the Rust source-index ranking engine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexRankRequest {
    /// User query that drives term extraction and path matching.
    pub query: String,
    /// Candidate set to rank.
    pub candidates: Vec<SourceIndexRankCandidate>,
}

/// Full Rust-computed ranking report for analysis and downstream projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexRankReport {
    /// Normalized query terms used by the ranker.
    pub query_terms: Vec<String>,
    /// Candidates with computed scores in final order.
    pub ranked_candidates: Vec<SourceIndexRankedCandidate>,
}

/// Ranked source-index candidate with the heavy score breakdown attached.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexRankedCandidate {
    /// Original candidate payload.
    pub candidate: SourceIndexRankCandidate,
    /// Rust-computed score used for ordering.
    pub score: SourceIndexRankScore,
}

/// Source-index score components computed in Rust.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceIndexRankScore {
    /// Weighted total score used by sort order.
    pub total: u16,
    /// Query exactly matched the normalized candidate path.
    pub exact_path: u16,
    /// Query matched the file basename.
    pub basename: u16,
    /// Query matched the basename stem.
    pub stem: u16,
    /// Query matched a path suffix.
    pub suffix: u16,
    /// Number of query terms covered by path or query keys.
    pub term_coverage: u16,
    /// Number of query-key matches.
    pub query_key_coverage: u16,
}

/// Tokenize a source-index lookup query into stable search terms.
#[must_use]
pub fn source_index_lookup_terms(query: &str) -> Vec<String> {
    let mut terms = BTreeSet::new();
    let trimmed = query.trim();
    if !trimmed.is_empty() {
        terms.insert(trimmed.to_ascii_lowercase());
        insert_source_index_path_terms(&mut terms, trimmed);
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

fn insert_source_index_path_terms(terms: &mut BTreeSet<String>, path: &str) {
    let normalized = path.trim().replace('\\', "/").to_ascii_lowercase();
    let segments = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    for start in 0..segments.len() {
        terms.insert(segments[start..].join("/"));
    }
    for segment in &segments {
        terms.insert((*segment).to_string());
    }
    if let Some(basename) = segments.last()
        && let Some(stem_index) = basename.rfind('.').filter(|index| *index > 0)
    {
        terms.insert(basename[..stem_index].to_string());
        if let Some(extension) = basename.get(stem_index + 1..) {
            terms.insert(extension.to_string());
        }
    }
}

/// Rank source-index candidates by query-axis coverage while preserving stable
/// DB order within equal coverage.
#[must_use]
pub fn rank_source_index_candidates(
    candidates: Vec<SourceIndexRankCandidate>,
    query: &str,
) -> Vec<SourceIndexRankCandidate> {
    rank_source_index_report(SourceIndexRankRequest {
        query: query.to_string(),
        candidates,
    })
    .ranked_candidates
    .into_iter()
    .map(|ranked| ranked.candidate)
    .collect()
}

/// Rank source-index candidates and return a complete Rust score report.
#[must_use]
pub fn rank_source_index_report(request: SourceIndexRankRequest) -> SourceIndexRankReport {
    let terms = source_index_lookup_terms(&request.query);
    let normalized_query = normalize_source_index_rank_path(&request.query);
    let mut indexed = request
        .candidates
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| {
            let normalized_path = normalize_source_index_rank_path_cow(&candidate.path);
            let query_key_blob = source_index_rank_query_key_blob(candidate.query_keys.as_slice());
            PreparedSourceIndexRankCandidate {
                stable_index: index,
                ranked: SourceIndexRankedCandidate {
                    score: source_index_rank_score_from_normalized(
                        normalized_path.as_ref(),
                        query_key_blob.as_str(),
                        terms.as_slice(),
                        &normalized_query,
                    ),
                    candidate,
                },
            }
        })
        .collect::<Vec<_>>();
    indexed.sort_by(|left, right| {
        right
            .ranked
            .score
            .cmp(&left.ranked.score)
            .then_with(|| left.stable_index.cmp(&right.stable_index))
            .then_with(|| left.ranked.candidate.path.cmp(&right.ranked.candidate.path))
    });
    SourceIndexRankReport {
        query_terms: terms,
        ranked_candidates: indexed
            .into_iter()
            .map(|prepared| prepared.ranked)
            .collect(),
    }
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

struct PreparedSourceIndexRankCandidate {
    stable_index: usize,
    ranked: SourceIndexRankedCandidate,
}

fn source_index_rank_score_from_normalized(
    normalized_path: &str,
    query_key_blob: &str,
    terms: &[String],
    normalized_query: &str,
) -> SourceIndexRankScore {
    let basename = source_index_rank_basename(normalized_path);
    let stem = basename.and_then(source_index_rank_stem);
    let exact_path = u16::from(!normalized_query.is_empty() && normalized_path == normalized_query);
    let basename_score = u16::from(basename == Some(normalized_query));
    let stem_score = u16::from(stem == Some(normalized_query));
    let suffix = u16::from(
        !normalized_query.is_empty()
            && normalized_path.ends_with(normalized_query)
            && normalized_path != normalized_query,
    );
    let term_coverage =
        source_index_candidate_query_axis_coverage(normalized_path, query_key_blob, terms) as u16;
    let query_key_coverage =
        source_index_candidate_query_key_coverage(query_key_blob, terms) as u16;
    SourceIndexRankScore {
        total: exact_path * 1000
            + basename_score * 800
            + stem_score * 700
            + suffix * 600
            + term_coverage * 25
            + query_key_coverage * 10,
        exact_path,
        basename: basename_score,
        stem: stem_score,
        suffix,
        term_coverage,
        query_key_coverage,
    }
}

fn source_index_candidate_query_axis_coverage(
    normalized_path: &str,
    query_key_blob: &str,
    terms: &[String],
) -> usize {
    terms
        .iter()
        .filter(|term| {
            !term.is_empty()
                && (normalized_path.contains(term.as_str())
                    || query_key_blob.contains(term.as_str()))
        })
        .count()
}

fn source_index_candidate_query_key_coverage(query_key_blob: &str, terms: &[String]) -> usize {
    terms
        .iter()
        .filter(|term| !term.is_empty() && query_key_blob.contains(term.as_str()))
        .count()
}

fn source_index_rank_query_key_blob(query_keys: &[String]) -> String {
    let mut blob = String::new();
    for key in query_keys {
        if !blob.is_empty() {
            blob.push(' ');
        }
        blob.push_str(normalize_source_index_rank_path_cow(key).as_ref());
    }
    blob
}

fn normalize_source_index_rank_path(path: &str) -> String {
    normalize_source_index_rank_path_cow(path).into_owned()
}

fn normalize_source_index_rank_path_cow(path: &str) -> Cow<'_, str> {
    let trimmed = path.trim().trim_matches('/');
    if trimmed
        .bytes()
        .any(|byte| byte == b'\\' || byte.is_ascii_uppercase())
    {
        Cow::Owned(trimmed.replace('\\', "/").to_ascii_lowercase())
    } else {
        Cow::Borrowed(trimmed)
    }
}

fn source_index_rank_basename(path: &str) -> Option<&str> {
    path.rsplit('/').find(|segment| !segment.is_empty())
}

fn source_index_rank_stem(basename: &str) -> Option<&str> {
    basename
        .rfind('.')
        .filter(|index| *index > 0)
        .map(|index| &basename[..index])
}
