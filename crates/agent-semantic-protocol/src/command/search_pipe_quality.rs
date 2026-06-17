//! Query-pack quality gates for ASP-owned search frontiers.

use std::collections::{BTreeSet, HashMap};

use super::search_pipe_model::Candidate;
use super::search_pipe_owner_roles::{has_strong_secondary_owner_intent, secondary_like_owner};
use super::search_pipe_query_evidence::{
    declaration_header_match, finder_handles, handle_paths, high_value_matches, high_value_missing,
    is_high_value_term, parser_handles, path_exact_match, strong_match, weak_match, weak_reason,
};
use super::search_pipe_query_model::{ClauseCoverage, QueryTerm, TermRole};
use super::search_pipe_query_pack::{
    clause_coverages, next_query_pack_hint, query_clauses, role_terms, unique_query_terms,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SearchPipeQuality {
    pub(super) clause_count: usize,
    pub(super) query_pack_quality: String,
    pub(super) global_matched: Vec<String>,
    pub(super) global_missing: Vec<String>,
    pub(super) path_matched: Vec<String>,
    pub(super) path_missing: Vec<String>,
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
    pub(super) finder_handles: Vec<String>,
    pub(super) next_query_pack_hint: Option<String>,
    clause_coverages: Vec<ClauseCoverage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OwnerCoverage {
    pub(super) owner: String,
    pub(super) matched: Vec<String>,
    pub(super) missing: Vec<String>,
}

pub(super) fn analyze_search_pipe_quality(
    language_id: &str,
    query: &str,
    candidates: &[Candidate],
) -> SearchPipeQuality {
    let clauses = query_clauses(language_id, query);
    let terms = unique_query_terms(&clauses);
    let global_matched = matched_terms(&terms, candidates);
    let global_missing = missing_terms(&terms, &global_matched);
    let path_matched = high_value_matches(&terms, candidates, path_exact_match);
    let path_missing = high_value_missing(&terms, &path_matched);
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
    let risks = risks(
        &terms,
        candidates,
        &global_missing,
        &strong_matched,
        &weak_terms,
        &package_cohesion,
        clauses.len(),
    );
    let query_pack_quality = query_pack_quality(&terms, &global_missing, &weak_terms, &risks);
    let allow_query_selector =
        query_pack_quality != "low" && package_cohesion != "low" && weak_terms.is_empty();
    let fd_query = fd_query_terms(&terms, &weak_terms, &strong_matched, &risks);
    let context_terms = role_terms(&terms, TermRole::Context);
    let owner_seed_terms = role_terms(&terms, TermRole::Symbol);
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

pub(super) fn is_generated_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("/generated/")
        || lower.contains("/vendor/")
        || lower.contains("/vendors/")
        || lower.contains("/dist/")
        || lower.contains("/build/")
        || lower.contains("/node_modules/")
        || lower.ends_with("/generated.ts")
        || lower.ends_with("/generated.tsx")
        || lower.ends_with("/generated.js")
        || lower.ends_with("/generated.jsx")
        || lower.ends_with("generated.ts")
        || lower.ends_with("generated.tsx")
        || lower.ends_with("generated.js")
        || lower.ends_with("generated.jsx")
}

pub(super) fn query_allows_generated(query: Option<&str>) -> bool {
    let Some(query) = query else {
        return false;
    };
    query
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .map(str::to_ascii_lowercase)
        .any(|term| matches!(term.as_str(), "generated" | "api" | "schema" | "client"))
}

pub(super) fn compact_fact_value(value: &str) -> String {
    let mut first = value.lines().next().unwrap_or(value).trim().to_string();
    if let Some((prefix, _)) = first.split_once(':')
        && !prefix.trim().is_empty()
        && prefix.len() <= 80
    {
        first = prefix.trim().to_string();
    }
    if first.len() > 96 {
        first.truncate(96);
        first.push_str("...");
    }
    first
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
    let mut packages = BTreeSet::new();
    candidates
        .iter()
        .filter_map(|candidate| packages.insert(package_key(&candidate.path)).then_some(()))
        .take(6)
        .for_each(drop);
    packages.into_iter().collect()
}

fn package_key(path: &str) -> String {
    let parts = path.split('/').collect::<Vec<_>>();
    if let Some(index) = parts.iter().position(|part| *part == "packages") {
        let end = (index + 3).min(parts.len());
        return parts[index..end].join("/");
    }
    parts
        .into_iter()
        .filter(|part| !part.is_empty() && *part != ".")
        .take(2)
        .collect::<Vec<_>>()
        .join("/")
}

fn package_cohesion(
    packages: &[String],
    best_owner: &Option<OwnerCoverage>,
    terms: &[QueryTerm],
) -> String {
    let high_value_terms = terms
        .iter()
        .filter(|term| is_high_value_term(term))
        .collect::<Vec<_>>();
    let high_value_count = high_value_terms.len().max(1);
    let best_owner_high_value_hits = best_owner
        .as_ref()
        .map(|owner| {
            high_value_terms
                .iter()
                .filter(|term| owner.matched.iter().any(|matched| matched == &term.lower))
                .count()
        })
        .unwrap_or_default();
    let package_axis_terms = high_value_terms
        .iter()
        .filter(|term| is_package_axis_term(&term.raw))
        .collect::<Vec<_>>();
    let best_owner_package_axis_hits = best_owner
        .as_ref()
        .map(|owner| {
            package_axis_terms
                .iter()
                .filter(|term| owner.matched.iter().any(|matched| matched == &term.lower))
                .count()
        })
        .unwrap_or_default();
    let has_strong_owner_anchor =
        high_value_terms.len() >= 2 && best_owner_high_value_hits >= high_value_terms.len();
    if (package_axis_terms.len() > 1 && best_owner_package_axis_hits < package_axis_terms.len())
        || (packages.len() > 3 && !has_strong_owner_anchor)
        || best_owner_high_value_hits * 2 < high_value_count
    {
        "low".to_string()
    } else if packages.len() > 1 {
        "medium".to_string()
    } else {
        "high".to_string()
    }
}

fn is_package_axis_term(raw: &str) -> bool {
    raw.matches('-').count() >= 2 && !matches!(raw, "long-field-signatures")
}

fn risks(
    terms: &[QueryTerm],
    candidates: &[Candidate],
    global_missing: &[String],
    strong_matched: &[String],
    weak_terms: &[String],
    package_cohesion: &str,
    clause_count: usize,
) -> Vec<String> {
    let mut risks = Vec::new();
    if clause_count == 1
        && terms.len() >= 5
        && terms.iter().filter(|term| is_high_value_term(term)).count() >= 3
    {
        risks.push("single-broad-clause".to_string());
    }
    if global_missing.is_empty() && !weak_terms.is_empty() {
        risks.push("coverage-inflation".to_string());
    }
    if package_cohesion == "low" {
        risks.push("package-drift".to_string());
    }
    if candidates
        .iter()
        .any(|candidate| is_generated_path(&candidate.path))
    {
        risks.push("generated-match".to_string());
    }
    if terms.iter().any(is_high_value_term) && !weak_terms.is_empty() {
        risks.push("weak-camelcase-match".to_string());
    }
    if candidates
        .iter()
        .any(|candidate| candidate.text.len() > 160 || candidate.text.contains('\n'))
    {
        risks.push("long-field-signatures".to_string());
    }
    if strong_matched.is_empty() && terms.iter().filter(|term| is_high_value_term(term)).count() > 1
    {
        risks.push("no-strong-symbol-coverage".to_string());
    }
    risks
}

fn query_pack_quality(
    terms: &[QueryTerm],
    global_missing: &[String],
    weak_terms: &[String],
    risks: &[String],
) -> String {
    if risks.iter().any(|risk| {
        matches!(
            risk.as_str(),
            "single-broad-clause" | "package-drift" | "no-strong-symbol-coverage"
        )
    }) {
        "low"
    } else if weak_terms.is_empty() && global_missing.is_empty() {
        "high"
    } else if terms.is_empty() {
        "low"
    } else {
        "medium"
    }
    .to_string()
}

fn fd_query_terms(
    terms: &[QueryTerm],
    weak_terms: &[String],
    strong_matched: &[String],
    risks: &[String],
) -> Option<String> {
    let symbol_terms = terms
        .iter()
        .filter(|term| matches!(term.role, TermRole::Symbol))
        .filter(|term| {
            weak_terms.is_empty()
                || weak_terms.iter().any(|weak| weak == &term.raw)
                || strong_matched.iter().any(|matched| matched == &term.raw)
        })
        .map(|term| term.raw.clone())
        .collect::<Vec<_>>();
    if !symbol_terms.is_empty() {
        return Some(symbol_terms.join("|"));
    }
    if !risks
        .iter()
        .any(|risk| matches!(risk.as_str(), "single-broad-clause" | "package-drift"))
    {
        return None;
    }
    let owner_axis_terms = terms
        .iter()
        .filter(|term| !matches!(term.role, TermRole::Symbol))
        .filter(|term| fd_owner_axis_term(&term.raw))
        .map(|term| term.raw.clone())
        .take(8)
        .collect::<Vec<_>>();
    (!owner_axis_terms.is_empty()).then(|| owner_axis_terms.join("|"))
}

fn fd_owner_axis_term(term: &str) -> bool {
    let lower = term.to_ascii_lowercase();
    if lower.len() < 4 {
        return false;
    }
    if matches!(
        lower.as_str(),
        "query"
            | "search"
            | "pipe"
            | "fd"
            | "rg"
            | "owner"
            | "owners"
            | "graph"
            | "turbo"
            | "command"
            | "commands"
            | "frontier"
            | "frontiers"
            | "action"
            | "actions"
            | "result"
            | "results"
            | "quality"
            | "wide"
            | "drift"
            | "handoff"
    ) {
        return false;
    }
    term.chars()
        .all(|ch| ch == '.' || ch == '_' || ch == '-' || ch.is_ascii_alphanumeric())
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
