//! Seed-phase decision model for ASP-owned search pipe graph turbo.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SeedActionIntent {
    SplitQueryPack,
    NarrowOwnerScope,
}

impl SeedActionIntent {
    pub(super) fn from_seed_plan_action(action: &str) -> Option<Self> {
        match action {
            "split-query-pack" => Some(Self::SplitQueryPack),
            "narrow-owner-scope" => Some(Self::NarrowOwnerScope),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SearchEvidenceState {
    Unknown,
    KnownOwner,
    KnownSymbol,
    KnownSelector,
    KnownDependency,
    KnownChangedFile,
    KnownFailure,
}

impl SearchEvidenceState {
    pub(super) fn all() -> &'static [Self] {
        &[
            Self::Unknown,
            Self::KnownOwner,
            Self::KnownSymbol,
            Self::KnownSelector,
            Self::KnownDependency,
            Self::KnownChangedFile,
            Self::KnownFailure,
        ]
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::KnownOwner => "known-owner",
            Self::KnownSymbol => "known-symbol",
            Self::KnownSelector => "known-selector",
            Self::KnownDependency => "known-dependency",
            Self::KnownChangedFile => "known-changed-file",
            Self::KnownFailure => "known-failure",
        }
    }

    pub(super) fn allowed_first_stages(self) -> &'static [&'static str] {
        match self {
            Self::Unknown => &["project-topology", "prime", "seed", "pipe"],
            Self::KnownOwner => &["owner-skeleton", "owner-items", "syntax-outline"],
            Self::KnownSymbol => &["syntax-query", "owner-items", "item-skeleton"],
            Self::KnownSelector => &["item-skeleton", "syntax-outline", "query-code"],
            Self::KnownDependency => &["dependency-topology", "import-usage", "owner-skeleton"],
            Self::KnownChangedFile => &["owner-skeleton", "tests", "policy"],
            Self::KnownFailure => &["failure-frontier", "tests", "owner-skeleton"],
        }
    }

    pub(super) fn disallowed_first_stages(self) -> &'static [&'static str] {
        match self {
            Self::Unknown => &[],
            Self::KnownOwner
            | Self::KnownSymbol
            | Self::KnownDependency
            | Self::KnownChangedFile
            | Self::KnownFailure => &["prime", "seed"],
            Self::KnownSelector => &["prime", "seed", "fd-query", "broad-rg"],
        }
    }

    fn is_known_owner(self) -> bool {
        matches!(self, Self::KnownOwner)
    }

    fn is_known_symbol(self) -> bool {
        matches!(self, Self::KnownSymbol)
    }

    fn is_known_selector(self) -> bool {
        matches!(self, Self::KnownSelector)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SearchActionSelection {
    pub(super) evidence_state: SearchEvidenceState,
    pub(super) first_action_stage: &'static str,
    pub(super) allowed_first_stages: &'static [&'static str],
    pub(super) disallowed_first_stages: &'static [&'static str],
    pub(super) first_action_matches_evidence_state: bool,
    pub(super) reasoning_tree_route_shown: bool,
    pub(super) chosen_route_preconditions_met: bool,
    pub(super) unnecessary_seed_count: usize,
    pub(super) seed_when_known_owner_count: usize,
    pub(super) seed_when_known_symbol_count: usize,
    pub(super) seed_when_known_selector_count: usize,
}

impl SearchActionSelection {
    pub(super) fn for_first_action(
        evidence_state: SearchEvidenceState,
        first_action_stage: &'static str,
    ) -> Self {
        let allowed_first_stages = evidence_state.allowed_first_stages();
        let disallowed_first_stages = evidence_state.disallowed_first_stages();
        let first_action_matches_evidence_state = allowed_first_stages
            .contains(&first_action_stage)
            && !disallowed_first_stages.contains(&first_action_stage);
        let seed_like_first_action = matches!(first_action_stage, "prime" | "seed");
        let unnecessary_seed_count =
            usize::from(seed_like_first_action && !first_action_matches_evidence_state);
        Self {
            evidence_state,
            first_action_stage,
            allowed_first_stages,
            disallowed_first_stages,
            first_action_matches_evidence_state,
            reasoning_tree_route_shown: true,
            chosen_route_preconditions_met: first_action_matches_evidence_state,
            unnecessary_seed_count,
            seed_when_known_owner_count: usize::from(
                seed_like_first_action && evidence_state.is_known_owner(),
            ),
            seed_when_known_symbol_count: usize::from(
                seed_like_first_action && evidence_state.is_known_symbol(),
            ),
            seed_when_known_selector_count: usize::from(
                seed_like_first_action && evidence_state.is_known_selector(),
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SeedPhaseDecision {
    pub(super) query_owner_anchor_budget: usize,
    pub(super) risk_factors: Vec<&'static str>,
}

impl SeedPhaseDecision {
    pub(super) fn from_query_shape(
        query_seed_present: bool,
        query_term_count: usize,
        candidate_owner_count: usize,
    ) -> Self {
        let mut risk_factors = Vec::new();
        let is_flat_query = query_term_count >= 6;
        let has_owner_drift = candidate_owner_count >= 4;
        if is_flat_query {
            risk_factors.push("flat-query");
        }
        if has_owner_drift {
            risk_factors.push("owner-drift");
        }
        let query_owner_anchor_budget = if query_seed_present && is_flat_query && has_owner_drift {
            2
        } else {
            0
        };
        Self {
            query_owner_anchor_budget,
            risk_factors,
        }
    }
}

pub(super) fn recommended_action_for_seed_risk(risk: &str) -> Option<&'static str> {
    match risk {
        "empty-seed-frontier" => Some("inspect-seed-extraction"),
        "fallback-owner" => Some("replace-fallback-owner-seed"),
        "query-seed-missing" => Some("propagate-query-seed"),
        "flat-query" => Some("split-query-pack"),
        "owner-drift" => Some("narrow-owner-scope"),
        _ => None,
    }
}
