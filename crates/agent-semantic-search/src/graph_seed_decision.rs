//! Search-owned seed-phase and first-action decision policy.

use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeedActionIntent {
    SplitQueryPack,
    NarrowOwnerScope,
}

impl SeedActionIntent {
    pub fn from_seed_plan_action(action: &str) -> Option<Self> {
        match action {
            "split-query-pack" => Some(Self::SplitQueryPack),
            "narrow-owner-scope" => Some(Self::NarrowOwnerScope),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchEvidenceState {
    Unknown,
    KnownOwner,
    KnownSymbol,
    KnownSelector,
    KnownDependency,
    KnownChangedFile,
    KnownFailure,
}

impl SearchEvidenceState {
    pub fn all() -> &'static [Self] {
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

    pub fn as_str(self) -> &'static str {
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

    pub fn allowed_first_stages(self) -> &'static [&'static str] {
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

    pub fn disallowed_first_stages(self) -> &'static [&'static str] {
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
pub struct SearchActionSelection {
    pub evidence_state: SearchEvidenceState,
    pub first_action_stage: &'static str,
    pub allowed_first_stages: &'static [&'static str],
    pub disallowed_first_stages: &'static [&'static str],
    pub first_action_matches_evidence_state: bool,
    pub reasoning_tree_route_shown: bool,
    pub chosen_route_preconditions_met: bool,
    pub unnecessary_seed_count: usize,
    pub seed_when_known_owner_count: usize,
    pub seed_when_known_symbol_count: usize,
    pub seed_when_known_selector_count: usize,
}

impl SearchActionSelection {
    pub fn for_first_action(
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
pub struct SeedPhaseDecision {
    pub query_owner_anchor_budget: usize,
    pub risk_factors: Vec<&'static str>,
}

impl SeedPhaseDecision {
    pub fn from_query_shape(
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

pub fn recommended_action_for_seed_risk(risk: &str) -> Option<&'static str> {
    match risk {
        "empty-seed-frontier" => Some("inspect-seed-extraction"),
        "fallback-owner" => Some("replace-fallback-owner-seed"),
        "query-seed-missing" => Some("propagate-query-seed"),
        "flat-query" => Some("split-query-pack"),
        "owner-drift" => Some("narrow-owner-scope"),
        _ => None,
    }
}

pub struct GraphTurboSeedPlanInput<'a> {
    pub query_present: bool,
    pub query_seed_present: bool,
    pub candidate_count: usize,
    pub candidate_owner_count: usize,
    pub query_owner_seed_count: usize,
    pub fallback_owner_seed_count: usize,
    pub seed_ids: &'a [String],
    pub seed_decision: &'a SeedPhaseDecision,
}

pub fn graph_turbo_seed_plan(input: GraphTurboSeedPlanInput<'_>) -> Value {
    let reason = if input.query_seed_present {
        "query"
    } else if input.fallback_owner_seed_count > 0 {
        "fallback-owner"
    } else {
        "empty"
    };
    let mut risk_factors = Vec::new();
    if input.seed_ids.is_empty() {
        risk_factors.push("empty-seed-frontier");
    }
    if input.fallback_owner_seed_count > 0 {
        risk_factors.push("fallback-owner");
    }
    if input.query_present && !input.query_seed_present {
        risk_factors.push("query-seed-missing");
    }
    risk_factors.extend(input.seed_decision.risk_factors.iter().copied());
    let seed_quality = if input.seed_ids.is_empty() {
        "fail"
    } else if risk_factors.is_empty() {
        "good"
    } else {
        "review"
    };
    let recommended_actions = if risk_factors.is_empty() {
        vec!["keep-query-seed"]
    } else {
        risk_factors
            .iter()
            .filter_map(|risk| recommended_action_for_seed_risk(risk))
            .collect::<Vec<_>>()
    };
    let selection = SearchActionSelection::for_first_action(SearchEvidenceState::Unknown, "seed");
    let evidence_states = SearchEvidenceState::all()
        .iter()
        .map(|state| state.as_str())
        .collect::<Vec<_>>();
    json!({
        "phase": "seed-query",
        "algorithm": "asp-search-pipe-v1",
        "reason": reason,
        "seedQuality": seed_quality,
        "queryPresent": input.query_present,
        "querySeedPresent": input.query_seed_present,
        "candidateCount": input.candidate_count,
        "candidateOwnerCount": input.candidate_owner_count,
        "queryOwnerSeedCount": input.query_owner_seed_count,
        "fallbackOwnerSeedCount": input.fallback_owner_seed_count,
        "selectedSeedCount": input.seed_ids.len(),
        "seedIds": input.seed_ids,
        "riskFactors": risk_factors,
        "recommendedActions": recommended_actions,
        "selectionPolicy": {
            "flow": "evidence-state-reasoning-tree",
            "evidenceState": selection.evidence_state.as_str(),
            "knownEvidenceStates": evidence_states,
            "firstActionStage": selection.first_action_stage,
            "allowedFirstStages": selection.allowed_first_stages,
            "disallowedFirstStages": selection.disallowed_first_stages,
            "firstActionMatchesEvidenceState": selection.first_action_matches_evidence_state,
            "reasoningTreeRouteShown": selection.reasoning_tree_route_shown,
            "chosenRoutePreconditionsMet": selection.chosen_route_preconditions_met,
            "unnecessarySeedCount": selection.unnecessary_seed_count,
            "seedWhenKnownOwnerCount": selection.seed_when_known_owner_count,
            "seedWhenKnownSymbolCount": selection.seed_when_known_symbol_count,
            "seedWhenKnownSelectorCount": selection.seed_when_known_selector_count,
        },
    })
}
