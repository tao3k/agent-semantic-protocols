use super::search_pipe_query_model::ClauseCoverage;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SearchPipeQuality {
    pub(super) clause_count: usize,
    pub(super) query_pack_quality: String,
    pub(super) global_matched: Vec<String>,
    pub(super) global_missing: Vec<String>,
    pub(super) path_matched: Vec<String>,
    pub(super) path_missing: Vec<String>,
    pub(super) missing_path_terms: Vec<String>,
    pub(super) declaration_matched: Vec<String>,
    pub(super) declaration_missing: Vec<String>,
    pub(super) strong_matched: Vec<String>,
    pub(super) weak_terms: Vec<String>,
    pub(super) weak_reasons: Vec<String>,
    pub(super) best_owner: Option<OwnerCoverage>,
    pub(super) package_cohesion: String,
    pub(super) packages: Vec<String>,
    pub(super) risks: Vec<String>,
    pub(super) allow_query_selector: bool,
    pub(super) fd_query: Option<String>,
    pub(super) context_terms: Vec<String>,
    pub(super) owner_seed_terms: Vec<String>,
    pub(super) concept_terms: Vec<String>,
    pub(super) page_index_handles: Vec<String>,
    pub(super) parser_handles: Vec<String>,
    pub(super) search_overlay_handles: Vec<String>,
    pub(super) next_query_pack_hint: Option<String>,
    pub(super) clause_coverages: Vec<ClauseCoverage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OwnerCoverage {
    pub(super) owner: String,
    pub(super) matched: Vec<String>,
    pub(super) missing: Vec<String>,
}
