//! Query-pack quality gates for ASP-owned search frontiers.

use std::collections::{BTreeSet, HashMap};

use super::search_pipe_model::Candidate;
use super::search_pipe_owner_roles::{has_strong_secondary_owner_intent, secondary_like_owner};
use super::search_pipe_quality_model::{OwnerCoverage, SearchPipeQuality};
use super::search_pipe_query_evidence::{
    declaration_header_match, finder_handles, handle_paths, high_value_matches, high_value_missing,
    is_high_value_term, parser_handles, path_exact_match, strong_match, weak_match, weak_reason,
};
use super::search_pipe_query_model::{QueryTerm, TermRole};
use super::search_pipe_query_pack::{
    clause_coverages, next_query_pack_hint, query_clauses, role_terms, unique_query_terms,
};

pub(super) fn analyze_search_pipe_quality(
    language_id: &str,
    query: &str,
    candidates: &[Candidate],
) -> SearchPipeQuality {
    let clauses = query_clauses(language_id, query);
    let terms = unique_query_terms(&clauses);
    let search_terms = search_terms_from_protocol(&terms);
    let global_matched = matched_terms(&terms, candidates);
    let global_missing = missing_terms(&terms, &global_matched);
    let path_matched = high_value_matches(&terms, candidates, path_exact_match);
    let path_missing = high_value_missing(&terms, &path_matched);
    let missing_path_terms =
        agent_semantic_search::search_pipe_missing_path_terms(&search_terms, &global_matched);
    let declaration_matched = high_value_matches(&terms, candidates, |candidate, term| {
        declaration_header_match(language_id, candidate, term)
    });
    let declaration_missing = high_value_missing(&terms, &declaration_matched);
    let strong_matched = high_value_matches(&terms, candidates, |candidate, term| {
        strong_match(language_id, candidate, term)
    });
    let weak_terms = weak_terms(&terms, &global_matched, &strong_matched);
    let weak_reasons = terms
        .iter()
        .filter(|term| weak_terms.iter().any(|weak| weak == &term.raw))
        .map(|term| format!("{}:{}", term.raw, weak_reason(term, candidates)))
        .collect::<Vec<_>>();
    let best_owner = best_owner_coverage(&terms, candidates);
    let packages = candidate_packages(candidates);
    let package_cohesion = package_cohesion(&packages, &best_owner, &terms);
    let risks = agent_semantic_search::search_pipe_quality_risks(
        &search_terms,
        candidates.iter().map(|candidate| candidate.text.clone()),
        &global_missing,
        &strong_matched,
        &weak_terms,
        &package_cohesion,
        clauses.len(),
    );
    let query_pack_quality = agent_semantic_search::search_pipe_query_pack_quality(
        &search_terms,
        &global_missing,
        &weak_terms,
        &risks,
    );
    let allow_query_selector =
        query_pack_quality != "low" && package_cohesion != "low" && weak_terms.is_empty();
    let fd_query = agent_semantic_search::search_pipe_fd_query_terms(
        &search_terms,
        &weak_terms,
        &strong_matched,
        &risks,
    );
    let context_terms = role_terms(&terms, TermRole::Context);
    let owner_seed_terms =
        agent_semantic_search::search_pipe_owner_seed_terms(&search_terms, &missing_path_terms);
    let concept_terms = role_terms(&terms, TermRole::Concept);
    let page_index_handles = handle_paths(candidates, |candidate| {
        candidate.source == "finder-path"
            || candidate.source == "fd-query"
            || candidate.confidence == "path-exact"
            || candidate.confidence == "path"
    });
    let parser_handles = parser_handles(language_id, candidates, &terms);
    let finder_handles = finder_handles(candidates, &terms);
    let next_query_pack_hint =
        next_query_pack_hint(&context_terms, &owner_seed_terms, &concept_terms);
    SearchPipeQuality {
        clause_count: clauses.len(),
        query_pack_quality,
        global_matched,
        global_missing,
        path_matched,
        path_missing,
        missing_path_terms,
        declaration_matched,
        declaration_missing,
        strong_matched,
        weak_terms,
        weak_reasons,
        best_owner,
        package_cohesion,
        packages,
        risks,
        allow_query_selector,
        fd_query,
        context_terms,
        owner_seed_terms,
        concept_terms,
        page_index_handles,
        parser_handles,
        finder_handles,
        next_query_pack_hint,
        clause_coverages: clause_coverages(&clauses, candidates),
    }
}

impl SearchPipeQuality {
    pub(super) fn query_terms_line(&self, language_id: &str, query: &str) -> String {
        let terms = unique_query_terms(&query_clauses(language_id, query))
            .into_iter()
            .map(|term| format!("{}:{}", term.raw, term.role.label()))
            .collect::<Vec<_>>();
        format!("queryTerms={}", display_terms(&terms))
    }

    pub(super) fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "globalCoverage=matched={} missing={}",
                display_terms(&self.global_matched),
                display_terms(&self.global_missing)
            ),
            format!(
                "pathCoverage=matched={} missing={}",
                display_terms(&self.path_matched),
                display_terms(&self.path_missing)
            ),
            format!(
                "declarationCoverage=matched={} missing={}",
                display_terms(&self.declaration_matched),
                display_terms(&self.declaration_missing)
            ),
            format!(
                "strongCoverage=matched={} weak={}",
                display_terms(&self.strong_matched),
                display_terms(&self.weak_terms)
            ),
            format!(
                "symbolTextCoverage=matched={} weakReasons={}",
                display_terms(&self.weak_terms),
                display_terms(&self.weak_reasons)
            ),
        ];
        if !self.missing_path_terms.is_empty() {
            lines.push(format!(
                "selectorGuard=missingPathTerms={} usableAsSelector=false usableAsOwner=false next=fd-query",
                display_terms(&self.missing_path_terms)
            ));
        }
        lines.extend(self.clause_coverages.iter().map(|coverage| {
            format!(
                "clauseCoverage=C{} matched={} missing={}",
                coverage.id,
                display_terms(&coverage.matched),
                display_terms(&coverage.missing)
            )
        }));
        lines.push(owner_coverage_line(&self.best_owner));
        lines.push(format!(
            "packageCohesion={} packages={}",
            self.package_cohesion,
            display_terms(&self.packages)
        ));
        lines.push(format!(
            "queryQuality={} reason={}",
            self.query_pack_quality,
            if self.risks.is_empty() {
                "ok".to_string()
            } else {
                display_terms(&self.risks)
            }
        ));
        if !self.risks.is_empty() {
            lines.push(format!("risk={}", display_terms(&self.risks)));
        }
        lines
    }

    pub(super) fn handles_line(&self) -> String {
        let hint = self
            .next_query_pack_hint
            .as_deref()
            .map(shell_quote)
            .unwrap_or_else(|| "-".to_string());
        format!(
            "handles=inputTerms={} contextTerms={} ownerSeedTerms={} conceptTerms={} pageIndexHandles={} parserHandles={} finderHandles={} nextQueryPackHint={}",
            display_terms(&self.input_terms()),
            display_terms(&self.context_terms),
            display_terms(&self.owner_seed_terms),
            display_terms(&self.concept_terms),
            display_terms(&self.page_index_handles),
            display_terms(&self.parser_handles),
            display_terms(&self.finder_handles),
            hint,
        )
    }

    fn input_terms(&self) -> Vec<String> {
        self.context_terms
            .iter()
            .chain(self.concept_terms.iter())
            .chain(self.owner_seed_terms.iter())
            .cloned()
            .collect()
    }
}

fn matched_terms(terms: &[QueryTerm], candidates: &[Candidate]) -> Vec<String> {
    terms
        .iter()
        .filter(|term| {
            candidates
                .iter()
                .any(|candidate| weak_match(candidate, term))
        })
        .map(|term| term.lower.clone())
        .collect()
}

fn missing_terms(terms: &[QueryTerm], matched: &[String]) -> Vec<String> {
    terms
        .iter()
        .filter(|term| !matched.iter().any(|seen| seen == &term.lower))
        .map(|term| term.lower.clone())
        .collect()
}

fn weak_terms(
    terms: &[QueryTerm],
    global_matched: &[String],
    strong_matched: &[String],
) -> Vec<String> {
    terms
        .iter()
        .filter(|term| is_high_value_term(term))
        .filter(|term| global_matched.iter().any(|matched| matched == &term.lower))
        .filter(|term| !strong_matched.iter().any(|matched| matched == &term.raw))
        .map(|term| term.raw.clone())
        .collect()
}

fn best_owner_coverage(terms: &[QueryTerm], candidates: &[Candidate]) -> Option<OwnerCoverage> {
    let mut per_owner: HashMap<String, BTreeSet<String>> = HashMap::new();
    for candidate in candidates {
        let matched = terms
            .iter()
            .filter(|term| !matches!(term.role, TermRole::Context))
            .filter(|term| weak_match(candidate, term))
            .map(|term| term.lower.clone())
            .collect::<Vec<_>>();
        if matched.is_empty() {
            continue;
        }
        per_owner
            .entry(candidate.path.clone())
            .or_default()
            .extend(matched);
    }
    let (owner, matched) = per_owner.into_iter().max_by(|left, right| {
        owner_coverage_score(&left.0, &left.1, terms)
            .cmp(&owner_coverage_score(&right.0, &right.1, terms))
            .then_with(|| right.0.cmp(&left.0))
    })?;
    let matched = matched.into_iter().collect::<Vec<_>>();
    let missing = terms
        .iter()
        .filter(|term| !matches!(term.role, TermRole::Context))
        .map(|term| term.lower.clone())
        .filter(|term| !matched.iter().any(|matched| matched == term))
        .collect::<Vec<_>>();
    Some(OwnerCoverage {
        owner,
        matched,
        missing,
    })
}

fn owner_coverage_score(
    owner: &str,
    matched: &BTreeSet<String>,
    terms: &[QueryTerm],
) -> (usize, usize, usize, usize) {
    let symbol_hits = terms
        .iter()
        .filter(|term| matches!(term.role, TermRole::Symbol))
        .filter(|term| matched.iter().any(|matched| matched == &term.lower))
        .count();
    let config_owner_bonus = usize::from(config_like_owner(owner));
    (
        owner_role_score(owner, terms),
        matched.len(),
        symbol_hits,
        config_owner_bonus,
    )
}

fn owner_role_score(owner: &str, terms: &[QueryTerm]) -> usize {
    if query_has_secondary_owner_intent(terms) || !secondary_like_owner(owner) {
        1
    } else {
        0
    }
}

fn query_has_secondary_owner_intent(terms: &[QueryTerm]) -> bool {
    has_strong_secondary_owner_intent(terms.iter().map(|term| term.lower.as_str()))
}

fn config_like_owner(owner: &str) -> bool {
    owner.ends_with(".pkg")
        || owner.ends_with(".toml")
        || owner.ends_with(".json")
        || owner.ends_with(".yaml")
        || owner.ends_with(".yml")
        || owner.ends_with("package.json")
        || owner.ends_with("Cargo.toml")
}

fn candidate_packages(candidates: &[Candidate]) -> Vec<String> {
    agent_semantic_search::search_pipe_candidate_packages(
        candidates.iter().map(|candidate| candidate.path.clone()),
    )
}

fn package_cohesion(
    packages: &[String],
    best_owner: &Option<OwnerCoverage>,
    terms: &[QueryTerm],
) -> String {
    let high_value_terms = terms
        .iter()
        .filter(|term| is_high_value_term(term))
        .map(|term| agent_semantic_search::SearchPipeCohesionTerm::new(&term.raw, &term.lower))
        .collect::<Vec<_>>();
    agent_semantic_search::search_pipe_package_cohesion(
        packages,
        best_owner.as_ref().map(|owner| owner.matched.as_slice()),
        &high_value_terms,
    )
}

fn owner_coverage_line(best_owner: &Option<OwnerCoverage>) -> String {
    if let Some(owner) = best_owner {
        format!(
            "ownerCoverage=bestOwner={} matched={} missing={}",
            owner.owner,
            display_terms(&owner.matched),
            display_terms(&owner.missing)
        )
    } else {
        "ownerCoverage=bestOwner=- matched=- missing=-".to_string()
    }
}

fn search_terms_from_protocol(
    terms: &[QueryTerm],
) -> Vec<agent_semantic_search::SearchPipeQueryTerm> {
    terms
        .iter()
        .map(|term| agent_semantic_search::SearchPipeQueryTerm {
            raw: term.raw.clone(),
            lower: term.lower.clone(),
            role: search_role_from_protocol(term.role),
        })
        .collect()
}

fn search_role_from_protocol(role: TermRole) -> agent_semantic_search::SearchPipeTermRole {
    match role {
        TermRole::Context => agent_semantic_search::SearchPipeTermRole::Context,
        TermRole::Concept => agent_semantic_search::SearchPipeTermRole::Concept,
        TermRole::Symbol => agent_semantic_search::SearchPipeTermRole::Symbol,
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn display_terms(terms: &[String]) -> String {
    if terms.is_empty() {
        "-".to_string()
    } else {
        terms.join(",")
    }
}
