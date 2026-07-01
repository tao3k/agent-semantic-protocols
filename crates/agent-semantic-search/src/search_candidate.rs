//! Shared search candidate contract for router and DB/search adapters.

use std::cmp::Ordering;

#[cfg(feature = "turso-overlay")]
use crate::structural_index_search::TursoStructuralIndexSearchHit;
use crate::{LexicalOverlaySearchHit, SourceIndexRankCandidate};

/// Search candidate shared by source-index, overlay, Turso FTS, and graph routes.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchCandidate {
    pub route_source: String,
    pub candidate_id: String,
    pub identity_kind: String,
    pub selector: Option<String>,
    pub owner_path: Option<String>,
    pub generation: Option<String>,
    pub overlay_namespace: Option<String>,
    pub score: f32,
    pub field_hits: Vec<FieldHit>,
    pub rank_features: Vec<RankFeature>,
    pub proof_source: String,
}

/// Matched field evidence that contributed to a search candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldHit {
    pub field: String,
    pub value: String,
    pub matched_terms: Vec<String>,
}

/// Named score component used by router and analyzer replay.
#[derive(Clone, Debug, PartialEq)]
pub struct RankFeature {
    pub name: String,
    pub value: f32,
}

/// Candidate after route merge and rank scoring.
#[derive(Clone, Debug, PartialEq)]
pub struct RankedSearchCandidate {
    pub candidate: SearchCandidate,
    pub route_priority: usize,
    pub selector_bonus: usize,
    pub field_hit_count: usize,
    pub ordinal: usize,
}

/// Project a source-index rank candidate into the shared search candidate shape.
#[must_use]
pub fn source_index_candidate_to_search_candidate(
    candidate: SourceIndexRankCandidate,
    query_terms: &[String],
) -> SearchCandidate {
    let matched_terms = query_terms
        .iter()
        .filter(|term| source_index_candidate_matches_term(&candidate, term))
        .cloned()
        .collect::<Vec<_>>();
    SearchCandidate {
        route_source: "source-index".to_string(),
        candidate_id: format!("source-index:{}", candidate.path),
        identity_kind: "owner-path".to_string(),
        selector: None,
        owner_path: Some(candidate.path.clone()),
        generation: None,
        overlay_namespace: None,
        score: matched_terms.len() as f32,
        field_hits: vec![FieldHit {
            field: "query_keys".to_string(),
            value: candidate.query_keys.join(" "),
            matched_terms,
        }],
        rank_features: vec![RankFeature {
            name: "query-axis-coverage".to_string(),
            value: candidate.query_keys.len() as f32,
        }],
        proof_source: "agent-semantic-search/source-index".to_string(),
    }
}

/// Project an overlay hit into the shared search candidate shape.
#[must_use]
pub fn lexical_overlay_hit_to_search_candidate(
    hit: &LexicalOverlaySearchHit,
    overlay_namespace: impl Into<String>,
) -> SearchCandidate {
    SearchCandidate {
        route_source: "dynamic-overlay".to_string(),
        candidate_id: format!("overlay:{}", hit.selector()),
        identity_kind: "selector".to_string(),
        selector: Some(hit.selector().to_string()),
        owner_path: Some(hit.owner_path().to_string()),
        generation: None,
        overlay_namespace: Some(overlay_namespace.into()),
        score: hit.score(),
        field_hits: vec![
            FieldHit {
                field: "name".to_string(),
                value: hit.name().to_string(),
                matched_terms: hit.matched_terms().to_vec(),
            },
            FieldHit {
                field: "search_text".to_string(),
                value: hit.search_text().to_string(),
                matched_terms: hit.matched_terms().to_vec(),
            },
        ],
        rank_features: vec![RankFeature {
            name: "lexical-overlay-score".to_string(),
            value: hit.score(),
        }],
        proof_source: "agent-semantic-search/lexical-overlay".to_string(),
    }
}

/// Project a Turso structural-index hit into the shared search candidate shape.
#[cfg(feature = "turso-overlay")]
#[must_use]
pub fn structural_index_hit_to_search_candidate(
    hit: &TursoStructuralIndexSearchHit,
    query_terms: &[String],
) -> SearchCandidate {
    let matched_terms = query_terms
        .iter()
        .filter(|term| structural_index_hit_matches_term(hit, term))
        .cloned()
        .collect::<Vec<_>>();
    SearchCandidate {
        route_source: "turso-fts".to_string(),
        candidate_id: hit.document_id.clone(),
        identity_kind: if hit.selector.is_some() {
            "selector".to_string()
        } else {
            "entity-id".to_string()
        },
        selector: hit.selector.clone(),
        owner_path: None,
        generation: structural_index_generation(hit.document_id.as_str()),
        overlay_namespace: None,
        score: matched_terms.len() as f32,
        field_hits: vec![FieldHit {
            field: "structural_index_document".to_string(),
            value: hit.document.clone(),
            matched_terms,
        }],
        rank_features: vec![RankFeature {
            name: "stable-structural-fts".to_string(),
            value: 1.0,
        }],
        proof_source: "agent-semantic-search/structural-index-turso".to_string(),
    }
}

/// Return true when a candidate still uses `path:start:end` as executable identity.
#[must_use]
pub fn search_candidate_has_executable_line_identity(candidate: &SearchCandidate) -> bool {
    candidate
        .selector
        .as_deref()
        .is_some_and(contains_executable_line_identity)
        || contains_executable_line_identity(candidate.candidate_id.as_str())
}

/// Merge heterogeneous search candidates into one router-ready order.
#[must_use]
pub fn merge_search_candidates(candidates: Vec<SearchCandidate>) -> Vec<RankedSearchCandidate> {
    let mut ranked = candidates
        .into_iter()
        .enumerate()
        .filter(|(_, candidate)| !search_candidate_has_executable_line_identity(candidate))
        .map(|(ordinal, candidate)| RankedSearchCandidate {
            route_priority: search_candidate_route_priority(candidate.route_source.as_str()),
            selector_bonus: usize::from(candidate.selector.is_some()),
            field_hit_count: candidate
                .field_hits
                .iter()
                .map(|field| field.matched_terms.len())
                .sum(),
            candidate,
            ordinal,
        })
        .collect::<Vec<_>>();
    ranked.sort_by(compare_ranked_search_candidates);
    ranked
}

fn compare_ranked_search_candidates(
    left: &RankedSearchCandidate,
    right: &RankedSearchCandidate,
) -> Ordering {
    left.route_priority
        .cmp(&right.route_priority)
        .then_with(|| right.selector_bonus.cmp(&left.selector_bonus))
        .then_with(|| {
            right
                .candidate
                .score
                .partial_cmp(&left.candidate.score)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| right.field_hit_count.cmp(&left.field_hit_count))
        .then_with(|| left.ordinal.cmp(&right.ordinal))
}

fn search_candidate_route_priority(route_source: &str) -> usize {
    match route_source {
        "receipt-anchor" => 0,
        "dynamic-overlay" => 1,
        "provider-delta" => 2,
        "turso-fts" => 3,
        "source-index" => 4,
        "semantic-vector" => 5,
        "evidence-graph-rank" => 6,
        _ => 9,
    }
}

fn source_index_candidate_matches_term(candidate: &SourceIndexRankCandidate, term: &str) -> bool {
    let normalized_term = term.to_ascii_lowercase();
    let normalized_path = candidate.path.to_ascii_lowercase();
    !normalized_term.is_empty()
        && (normalized_path.contains(normalized_term.as_str())
            || candidate
                .query_keys
                .iter()
                .any(|key| key.contains(normalized_term.as_str())))
}

#[cfg(feature = "turso-overlay")]
fn structural_index_hit_matches_term(hit: &TursoStructuralIndexSearchHit, term: &str) -> bool {
    let normalized_term = term.to_ascii_lowercase();
    !normalized_term.is_empty()
        && (hit
            .document
            .to_ascii_lowercase()
            .contains(normalized_term.as_str())
            || hit.selector.as_deref().is_some_and(|selector| {
                selector
                    .to_ascii_lowercase()
                    .contains(normalized_term.as_str())
            }))
}

#[cfg(feature = "turso-overlay")]
fn structural_index_generation(document_id: &str) -> Option<String> {
    let mut parts = document_id.splitn(3, ':');
    match (parts.next(), parts.next()) {
        (Some("structural-index"), Some(generation)) if !generation.is_empty() => {
            Some(generation.to_string())
        }
        _ => None,
    }
}

fn contains_executable_line_identity(value: &str) -> bool {
    let mut parts = value.rsplitn(3, ':');
    let end = parts.next();
    let start = parts.next();
    let path = parts.next();
    matches!(
        (path, start, end),
        (Some(path), Some(start), Some(end))
            if !path.is_empty()
                && start.parse::<u32>().is_ok()
                && end.parse::<u32>().is_ok()
    )
}
