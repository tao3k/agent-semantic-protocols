//! Shared lexical SearchFrame and GraphRouter algorithm surface.

/// Acquisition route selected before graph routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalAcquisitionRoute {
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
            Self::WarmOverlay => "warm-overlay",
            Self::SessionDynamicOverlay => "session-dynamic-overlay",
            Self::SourceIndexOwnerEvidence => "source-index-owner-evidence",
            Self::ProviderOwnerItems => "provider-owner-items",
            Self::BoundedColdScan => "bounded-cold-scan",
            Self::DegradedFinder => "degraded-finder",
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
    pub evidence_state: LexicalEvidenceState,
    pub fallback_reason: &'static str,
    pub provider_process_count: u32,
    pub native_finder_process_count: u32,
    pub selected_candidate_count: usize,
}

impl LexicalSearchFrameRoute {
    #[must_use]
    pub fn render_receipt(&self) -> String {
        format!(
            "searchFrame=lexical acquisitionRoute={} graphRouter=lexical-v1 evidenceState={} fallbackReason={} providerProcessCount={} nativeFinderProcessCount={} selectedCandidateCount={}",
            self.acquisition_route.as_str(),
            self.evidence_state.as_str(),
            self.fallback_reason,
            self.provider_process_count,
            self.native_finder_process_count,
            self.selected_candidate_count
        )
    }
}

/// Choose the lexical acquisition route before compact graph rendering.
#[must_use]
pub fn plan_lexical_search_frame(
    request: LexicalSearchFrameRequest<'_>,
) -> LexicalSearchFrameRoute {
    if request.terms.is_empty() {
        return LexicalSearchFrameRoute {
            acquisition_route: LexicalAcquisitionRoute::DegradedFinder,
            evidence_state: LexicalEvidenceState::Degraded,
            fallback_reason: "empty-query",
            provider_process_count: 0,
            native_finder_process_count: 0,
            selected_candidate_count: 0,
        };
    }

    if !request.warm_candidates.is_empty() {
        return LexicalSearchFrameRoute {
            acquisition_route: LexicalAcquisitionRoute::WarmOverlay,
            evidence_state: infer_evidence_state(request.warm_candidates),
            fallback_reason: "none",
            provider_process_count: 0,
            native_finder_process_count: 0,
            selected_candidate_count: request.warm_candidates.len(),
        };
    }

    if !request.session_candidates.is_empty() {
        return LexicalSearchFrameRoute {
            acquisition_route: LexicalAcquisitionRoute::SessionDynamicOverlay,
            evidence_state: infer_evidence_state(request.session_candidates),
            fallback_reason: "warm-miss",
            provider_process_count: 0,
            native_finder_process_count: 0,
            selected_candidate_count: request.session_candidates.len(),
        };
    }

    if !request.owner_candidates.is_empty() {
        return LexicalSearchFrameRoute {
            acquisition_route: LexicalAcquisitionRoute::SourceIndexOwnerEvidence,
            evidence_state: LexicalEvidenceState::OwnerReady,
            fallback_reason: "selector-proof-missing",
            provider_process_count: 0,
            native_finder_process_count: 0,
            selected_candidate_count: request.owner_candidates.len(),
        };
    }

    if request.provider_owner_item_available {
        return LexicalSearchFrameRoute {
            acquisition_route: LexicalAcquisitionRoute::ProviderOwnerItems,
            evidence_state: LexicalEvidenceState::OwnerReady,
            fallback_reason: "warm-miss",
            provider_process_count: 1,
            native_finder_process_count: 0,
            selected_candidate_count: 0,
        };
    }

    if request.cold_scan_allowed {
        return LexicalSearchFrameRoute {
            acquisition_route: LexicalAcquisitionRoute::BoundedColdScan,
            evidence_state: LexicalEvidenceState::NeedsColdScan,
            fallback_reason: "warm-miss",
            provider_process_count: 0,
            native_finder_process_count: 0,
            selected_candidate_count: 0,
        };
    }

    LexicalSearchFrameRoute {
        acquisition_route: LexicalAcquisitionRoute::DegradedFinder,
        evidence_state: LexicalEvidenceState::Degraded,
        fallback_reason: "no-parser-facts",
        provider_process_count: 0,
        native_finder_process_count: 1,
        selected_candidate_count: 0,
    }
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
