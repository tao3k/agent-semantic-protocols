use std::collections::BTreeSet;

use crate::{SearchPipeQueryTerm, SearchPipeTermRole};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeCohesionTerm {
    pub raw: String,
    pub lower: String,
}

impl SearchPipeCohesionTerm {
    #[must_use]
    pub fn new(raw: impl Into<String>, lower: impl Into<String>) -> Self {
        Self {
            raw: raw.into(),
            lower: lower.into(),
        }
    }
}

#[must_use]
pub fn search_pipe_package_key(path: &str) -> String {
    let parts = path
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    if let Some(index) = parts.iter().position(|part| *part == "packages") {
        return parts[index..(index + 3).min(parts.len())].join("/");
    }
    if let Some(index) = parts.iter().position(|part| *part == "crates") {
        return parts[index..(index + 2).min(parts.len())].join("/");
    }
    parts.into_iter().take(2).collect::<Vec<_>>().join("/")
}

#[must_use]
pub fn search_pipe_candidate_packages(paths: impl Iterator<Item = String>) -> Vec<String> {
    let mut packages = BTreeSet::new();
    paths
        .filter_map(|path| {
            packages
                .insert(search_pipe_package_key(&path))
                .then_some(())
        })
        .take(6)
        .for_each(drop);
    packages.into_iter().collect()
}

#[must_use]
pub fn search_pipe_package_cohesion(
    packages: &[String],
    best_owner_matched: Option<&[String]>,
    high_value_terms: &[SearchPipeCohesionTerm],
) -> String {
    let high_value_count = high_value_terms.len().max(1);
    let best_owner_high_value_hits = best_owner_matched
        .map(|matched| {
            high_value_terms
                .iter()
                .filter(|term| matched.iter().any(|matched| matched == &term.lower))
                .count()
        })
        .unwrap_or_default();
    let package_axis_terms = high_value_terms
        .iter()
        .filter(|term| is_search_pipe_package_axis_term(&term.raw))
        .collect::<Vec<_>>();
    let best_owner_package_axis_hits = best_owner_matched
        .map(|matched| {
            package_axis_terms
                .iter()
                .filter(|term| matched.iter().any(|matched| matched == &term.lower))
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

#[must_use]
pub fn is_search_pipe_package_axis_term(raw: &str) -> bool {
    raw.matches('-').count() >= 2 && !matches!(raw, "long-field-signatures")
}

#[must_use]
pub fn search_pipe_quality_risks(
    terms: &[SearchPipeQueryTerm],
    mut candidate_texts: impl Iterator<Item = String>,
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
    if terms.iter().any(is_high_value_term) && !weak_terms.is_empty() {
        risks.push("weak-camelcase-match".to_string());
    }
    if candidate_texts.any(|text| text.len() > 160 || text.contains('\n')) {
        risks.push("long-field-signatures".to_string());
    }
    if strong_matched.is_empty() && terms.iter().filter(|term| is_high_value_term(term)).count() > 1
    {
        risks.push("no-strong-symbol-coverage".to_string());
    }
    risks
}

#[must_use]
pub fn search_pipe_query_pack_quality(
    terms: &[SearchPipeQueryTerm],
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

#[must_use]
pub fn search_pipe_missing_path_terms(
    terms: &[SearchPipeQueryTerm],
    global_matched: &[String],
) -> Vec<String> {
    terms
        .iter()
        .filter(|term| crate::search_pipe_is_path_like_token(&term.raw))
        .filter(|term| !global_matched.iter().any(|matched| matched == &term.raw))
        .map(|term| term.raw.clone())
        .collect()
}

#[must_use]
pub fn search_pipe_owner_seed_terms(
    terms: &[SearchPipeQueryTerm],
    missing_path_terms: &[String],
) -> Vec<String> {
    crate::search_pipe_role_terms(terms, SearchPipeTermRole::Symbol)
        .into_iter()
        .filter(|term| !crate::search_pipe_is_path_like_token(term))
        .filter(|term| !missing_path_terms.iter().any(|missing| missing == term))
        .collect()
}

fn is_high_value_term(term: &SearchPipeQueryTerm) -> bool {
    matches!(term.role, SearchPipeTermRole::Symbol)
}
