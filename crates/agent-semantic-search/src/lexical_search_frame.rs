//! Shared lexical SearchFrame and GraphRouter algorithm surface.

/// Acquisition route selected before graph routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalAcquisitionRoute {
    /// QueryBundle is required before semantic acquisition can run.
    QueryBundleRequired,
    /// Warm search-owned overlay or source-index candidates were available.
    WarmOverlay,
    /// Session dynamic overlay supplied candidates without provider startup.
    SessionDynamicOverlay,
    /// Source index supplied owner/path evidence without bounded selector proof.
    SourceIndexOwnerEvidence,
    /// Provider parser owner-items are required.
    ProviderOwnerItems,
    /// A bounded cold scan is required because warm evidence was missing.
    BoundedColdScan,
    /// Native finder fallback is the only available route.
    DegradedFinder,
}

impl LexicalAcquisitionRoute {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QueryBundleRequired => "query-bundle-required",
            Self::WarmOverlay => "warm-overlay",
            Self::SessionDynamicOverlay => "session-dynamic-overlay",
            Self::SourceIndexOwnerEvidence => "source-index-owner-evidence",
            Self::ProviderOwnerItems => "provider-owner-items",
            Self::BoundedColdScan => "bounded-cold-scan",
            Self::DegradedFinder => "degraded-finder",
        }
    }
}

/// Semantic relation requested by a lexical query bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalQueryRelation {
    QueryBundleRequired,
    Cohesive,
}

impl LexicalQueryRelation {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QueryBundleRequired => "query-bundle-required",
            Self::Cohesive => "cohesive",
        }
    }
}

/// GraphRouter state after lexical evidence projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalEvidenceState {
    OwnerReady,
    ItemReady,
    TestReady,
    NeedsColdScan,
    Degraded,
}

impl LexicalEvidenceState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OwnerReady => "owner-ready",
            Self::ItemReady => "item-ready",
            Self::TestReady => "test-ready",
            Self::NeedsColdScan => "needs-cold-scan",
            Self::Degraded => "degraded",
        }
    }
}

/// Candidate projection consumed by the lexical SearchFrame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexicalSearchFrameCandidate {
    pub path: String,
    pub symbol: String,
    pub source: String,
}

impl From<&crate::DynamicSearchCandidate> for LexicalSearchFrameCandidate {
    fn from(candidate: &crate::DynamicSearchCandidate) -> Self {
        Self {
            path: candidate.path.clone(),
            symbol: candidate.symbol.clone(),
            source: candidate.source.clone(),
        }
    }
}

impl From<&crate::SearchPipeCandidate> for LexicalSearchFrameCandidate {
    fn from(candidate: &crate::SearchPipeCandidate) -> Self {
        Self {
            path: candidate.path.clone(),
            symbol: candidate.symbol.clone(),
            source: candidate.source.clone(),
        }
    }
}

/// Query request for the shared lexical SearchFrame.
pub struct LexicalSearchFrameRequest<'a> {
    pub terms: &'a [String],
    pub warm_candidates: &'a [LexicalSearchFrameCandidate],
    pub session_candidates: &'a [LexicalSearchFrameCandidate],
    pub owner_candidates: &'a [LexicalSearchFrameCandidate],
    pub provider_owner_item_available: bool,
    pub cold_scan_allowed: bool,
}

/// Route selected by the lexical SearchFrame.
#[derive(Debug, Eq, PartialEq)]
pub struct LexicalSearchFrameRoute {
    pub acquisition_route: LexicalAcquisitionRoute,
    pub query_relation: LexicalQueryRelation,
    pub evidence_state: LexicalEvidenceState,
    pub fallback_reason: &'static str,
    pub provider_process_count: u32,
    pub native_finder_process_count: u32,
    pub selected_candidate_count: usize,
    pub query_bundle_count: usize,
    pub covered_seed_count: usize,
    pub cohesive_owner_count: usize,
}

impl LexicalSearchFrameRoute {
    #[must_use]
    pub fn render_receipt(&self) -> String {
        format!(
            "searchFrame=lexical queryBundle={} queryRelation={} coveredSeeds={} cohesiveOwnerCount={} acquisitionRoute={} graphRouter=lexical-v1 evidenceState={} fallbackReason={} providerProcessCount={} nativeFinderProcessCount={} selectedCandidateCount={}",
            self.query_bundle_count,
            self.query_relation.as_str(),
            self.covered_seed_count,
            self.cohesive_owner_count,
            self.acquisition_route.as_str(),
            self.evidence_state.as_str(),
            self.fallback_reason,
            self.provider_process_count,
            self.native_finder_process_count,
            self.selected_candidate_count
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LexicalQueryBundleCoverage {
    query_bundle_count: usize,
    covered_seed_count: usize,
    cohesive_owner_count: usize,
}

fn lexical_route(
    request: &LexicalSearchFrameRequest<'_>,
    acquisition_route: LexicalAcquisitionRoute,
    evidence_state: LexicalEvidenceState,
    fallback_reason: &'static str,
    provider_process_count: u32,
    native_finder_process_count: u32,
    candidates: &[LexicalSearchFrameCandidate],
) -> LexicalSearchFrameRoute {
    let coverage = lexical_query_bundle_coverage(request.terms, candidates);
    LexicalSearchFrameRoute {
        acquisition_route,
        query_relation: lexical_query_relation(request.terms),
        evidence_state,
        fallback_reason,
        provider_process_count,
        native_finder_process_count,
        selected_candidate_count: candidates.len(),
        query_bundle_count: coverage.query_bundle_count,
        covered_seed_count: coverage.covered_seed_count,
        cohesive_owner_count: coverage.cohesive_owner_count,
    }
}

fn lexical_query_relation(terms: &[String]) -> LexicalQueryRelation {
    if terms.len() > 1 {
        LexicalQueryRelation::Cohesive
    } else {
        LexicalQueryRelation::QueryBundleRequired
    }
}

fn lexical_query_bundle_coverage(
    terms: &[String],
    candidates: &[LexicalSearchFrameCandidate],
) -> LexicalQueryBundleCoverage {
    let normalized_terms = terms
        .iter()
        .map(|term| term.to_ascii_lowercase())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    let mut covered_seeds = BTreeSet::new();
    let mut owner_seeds = BTreeMap::<&str, BTreeSet<usize>>::new();
    for candidate in candidates {
        for (seed_index, term) in normalized_terms.iter().enumerate() {
            if lexical_candidate_matches_seed(candidate, term) {
                covered_seeds.insert(seed_index);
                owner_seeds
                    .entry(candidate.path.as_str())
                    .or_default()
                    .insert(seed_index);
            }
        }
    }
    LexicalQueryBundleCoverage {
        query_bundle_count: normalized_terms.len(),
        covered_seed_count: covered_seeds.len(),
        cohesive_owner_count: owner_seeds.values().filter(|seeds| seeds.len() > 1).count(),
    }
}

fn lexical_candidate_matches_seed(candidate: &LexicalSearchFrameCandidate, term: &str) -> bool {
    candidate.symbol.to_ascii_lowercase().contains(term)
        || candidate.path.to_ascii_lowercase().contains(term)
}

/// Choose the lexical acquisition route before compact graph rendering.
#[must_use]
pub fn plan_lexical_search_frame(
    request: LexicalSearchFrameRequest<'_>,
) -> LexicalSearchFrameRoute {
    if request.terms.len() < 2 {
        return lexical_route(
            &request,
            LexicalAcquisitionRoute::QueryBundleRequired,
            LexicalEvidenceState::Degraded,
            "query-bundle-required",
            0,
            0,
            &[],
        );
    }

    if !request.warm_candidates.is_empty() {
        return lexical_route(
            &request,
            LexicalAcquisitionRoute::WarmOverlay,
            infer_evidence_state(request.warm_candidates),
            "none",
            0,
            0,
            request.warm_candidates,
        );
    }

    if !request.session_candidates.is_empty() {
        return lexical_route(
            &request,
            LexicalAcquisitionRoute::SessionDynamicOverlay,
            infer_evidence_state(request.session_candidates),
            "warm-miss",
            0,
            0,
            request.session_candidates,
        );
    }

    if !request.owner_candidates.is_empty() {
        return lexical_route(
            &request,
            LexicalAcquisitionRoute::SourceIndexOwnerEvidence,
            LexicalEvidenceState::OwnerReady,
            "selector-proof-missing",
            0,
            0,
            request.owner_candidates,
        );
    }

    if request.provider_owner_item_available {
        return lexical_route(
            &request,
            LexicalAcquisitionRoute::ProviderOwnerItems,
            LexicalEvidenceState::OwnerReady,
            "warm-miss",
            1,
            0,
            &[],
        );
    }

    if request.cold_scan_allowed {
        return lexical_route(
            &request,
            LexicalAcquisitionRoute::BoundedColdScan,
            LexicalEvidenceState::NeedsColdScan,
            "warm-miss",
            0,
            0,
            &[],
        );
    }

    lexical_route(
        &request,
        LexicalAcquisitionRoute::DegradedFinder,
        LexicalEvidenceState::Degraded,
        "no-parser-facts",
        0,
        1,
        &[],
    )
}

fn infer_evidence_state(candidates: &[LexicalSearchFrameCandidate]) -> LexicalEvidenceState {
    if candidates
        .iter()
        .any(|candidate| candidate.path.contains("test"))
    {
        LexicalEvidenceState::TestReady
    } else if candidates
        .iter()
        .any(|candidate| !candidate.symbol.is_empty())
    {
        LexicalEvidenceState::ItemReady
    } else {
        LexicalEvidenceState::OwnerReady
    }
}
use std::collections::{BTreeMap, BTreeSet};
