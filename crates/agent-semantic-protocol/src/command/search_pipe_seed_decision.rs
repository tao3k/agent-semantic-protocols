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
