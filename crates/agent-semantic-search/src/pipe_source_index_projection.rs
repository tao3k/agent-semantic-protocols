//! Source-index candidate projection for `SearchPipe` acquisition.

use std::path::Path;

use crate::{
    SearchPipeCandidate,
    pipe_source::{
        SearchPipeSourceAcquisitionTrace, SearchPipeSourceIndexAcquisition,
        SearchPipeSourceIndexCandidate, SearchPipeSourceIndexDecision,
    },
};

pub(crate) fn source_index_candidate(
    project_root: &Path,
    intent: &str,
    candidate: &SearchPipeSourceIndexCandidate,
) -> SearchPipeCandidate {
    let line_count = candidate.line_count.unwrap_or(1).max(1) as usize;
    let confidence = source_index_candidate_confidence(project_root, candidate);
    SearchPipeCandidate {
        path: candidate.path.clone(),
        line: 1,
        end_line: line_count,
        symbol: source_index_symbol(intent, candidate),
        text: source_index_candidate_text(candidate),
        source: "source-index".to_string(),
        confidence: confidence.to_string(),
    }
}

pub(crate) fn source_index_candidate_ready(candidate: &SearchPipeCandidate) -> bool {
    candidate.confidence == "selector-ready"
}

pub(crate) fn source_index_trace(
    acquisition: &SearchPipeSourceIndexAcquisition,
) -> SearchPipeSourceAcquisitionTrace {
    let status = match acquisition.decision {
        SearchPipeSourceIndexDecision::QueryGate => "query-gate",
        SearchPipeSourceIndexDecision::DeferBackend => "deferred",
        SearchPipeSourceIndexDecision::UseAndSkipSearchOverlay => "used",
        SearchPipeSourceIndexDecision::Fallthrough => "fallthrough",
    };
    SearchPipeSourceAcquisitionTrace {
        source: "sourceIndex".to_string(),
        status: status.to_string(),
        matched: acquisition.candidates.len(),
        missing: acquisition
            .candidates
            .iter()
            .filter(|candidate| candidate.confidence == "stale-index")
            .count(),
        normalized: acquisition
            .candidates
            .iter()
            .filter(|candidate| source_index_candidate_ready(candidate))
            .count(),
        elapsed: None,
    }
}

fn source_index_candidate_confidence(
    project_root: &Path,
    candidate: &SearchPipeSourceIndexCandidate,
) -> &'static str {
    if candidate.path.trim().is_empty() {
        return "invalid-selector";
    }
    if !project_root.join(&candidate.path).is_file() {
        return "stale-index";
    }
    if source_index_candidate_has_payload_proof(candidate) {
        return "selector-ready";
    }
    "inventory-only"
}

fn source_index_candidate_has_payload_proof(candidate: &SearchPipeSourceIndexCandidate) -> bool {
    let Some(proof) = candidate.selector_proof.as_ref() else {
        return false;
    };
    if !proof.bounded || proof.payload_kind != "code" || proof.structural_selector.trim().is_empty()
    {
        return false;
    }
    structural_selector_owner_path(&proof.structural_selector) == Some(candidate.path.as_str())
}

fn structural_selector_owner_path(selector: &str) -> Option<&str> {
    let (_, rest) = selector.split_once("://")?;
    let (owner, _) = rest.split_once('#')?;
    let owner = owner.trim();
    (!owner.is_empty()).then_some(owner)
}

fn source_index_symbol(intent: &str, candidate: &SearchPipeSourceIndexCandidate) -> String {
    let query_terms = intent
        .split_whitespace()
        .map(normalize_source_index_symbol_key)
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    if let Some(key) = candidate.query_keys.iter().find(|key| {
        let normalized = normalize_source_index_symbol_key(key);
        query_terms.iter().any(|term| term == &normalized)
    }) {
        return key.clone();
    }
    if let Some(key) = candidate.query_keys.iter().find(|key| {
        let normalized = normalize_source_index_symbol_key(key);
        query_terms
            .iter()
            .any(|term| normalized.contains(term) || term.contains(&normalized))
    }) {
        return key.clone();
    }
    candidate
        .query_keys
        .first()
        .cloned()
        .unwrap_or_else(|| symbol_from_path(&candidate.path))
}

fn normalize_source_index_symbol_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .flat_map(char::to_lowercase)
        .collect()
}

fn source_index_candidate_text(candidate: &SearchPipeSourceIndexCandidate) -> String {
    let language = candidate.language_id.as_deref().unwrap_or("unknown");
    let provider = candidate.provider_id.as_deref().unwrap_or("unknown");
    let proof = candidate
        .selector_proof
        .as_ref()
        .map(|proof| {
            if proof.bounded {
                proof.payload_kind.as_str()
            } else {
                "unbounded"
            }
        })
        .unwrap_or("none");
    let keys = candidate
        .query_keys
        .iter()
        .take(8)
        .cloned()
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "source-index path={} language={} provider={} kind={} payloadProof={} queryKeys={}",
        candidate.path, language, provider, candidate.source_kind, proof, keys
    )
}

fn symbol_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("source")
        .to_string()
}
