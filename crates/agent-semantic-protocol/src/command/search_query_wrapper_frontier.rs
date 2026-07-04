//! Low-confidence action frontier for query-wrapper recall probes.

use std::path::PathBuf;

use agent_semantic_search::QUERY_OVERLAY_ROUTE_SOURCE;

use super::search_pipe_action_frontier::{ActionNode, ActionRoute};
use super::search_pipe_model::Candidate;
use super::search_pipe_owner_roles::{has_strong_secondary_owner_intent, secondary_like_owner};
use super::search_query_budget::specific_search_term;
use super::search_query_wrapper_candidates::{owner_candidates, rg_scope_next};
use super::search_query_wrapper_model::{QueryWrapperQuality, QueryWrapperSurface, shell_arg};

pub(super) fn query_display(queries: &[String]) -> String {
    queries.join(" + ")
}

pub(super) fn query_clauses_line(
    clauses: &[super::search_query_wrapper_model::QueryWrapperClause],
) -> String {
    if clauses.is_empty() {
        return "-".to_string();
    }
    clauses
        .iter()
        .map(|clause| format!("C{}={}", clause.id, shell_arg(&clause.raw)))
        .collect::<Vec<_>>()
        .join(";")
}

pub(super) fn print_query_wrapper_refinement_frontier(
    surface: QueryWrapperSurface,
    scopes: &[PathBuf],
    queries: &[String],
    terms: &[String],
    candidates: &[Candidate],
    quality: &QueryWrapperQuality,
) {
    print!(
        "{}",
        render_query_wrapper_next_command(surface, scopes, queries, terms, candidates, quality)
    );
    println!("reason=query-selector-low-confidence,clause-cohesion-required");
}

pub(super) fn print_query_wrapper_empty_receipt(
    surface: QueryWrapperSurface,
    scopes: &[PathBuf],
    queries: &[String],
    terms: &[String],
    source_trace: &str,
    avoid: &str,
    reason: &str,
) {
    println!("noOutput reason={reason} sourceTrace={source_trace}");
    println!(
        "nextCommand={}",
        query_wrapper_empty_next_command(surface, scopes, queries, terms, reason)
    );
    if reason == "query-too-broad" {
        println!(
            "refineHint=use path-or-symbol terms first; example: asp fd -query 'path-or-symbol|error-code' --workspace <scope>"
        );
    } else if reason == "source-index-miss" {
        println!(
            "refineHint=SourceIndex miss in indexed workspace; refresh the index or retry with owner/path/symbol terms"
        );
    }
    println!("avoid={avoid}");
}

pub(super) fn query_wrapper_action_frontier(
    surface: QueryWrapperSurface,
    scopes: &[PathBuf],
    queries: &[String],
    terms: &[String],
    candidates: &[Candidate],
    quality: &QueryWrapperQuality,
) -> Vec<serde_json::Value> {
    query_wrapper_action_nodes(surface, scopes, queries, terms, candidates, quality)
        .into_iter()
        .map(|action| action.as_json())
        .collect()
}

pub(super) fn render_query_wrapper_next_command(
    surface: QueryWrapperSurface,
    scopes: &[PathBuf],
    queries: &[String],
    terms: &[String],
    candidates: &[Candidate],
    quality: &QueryWrapperQuality,
) -> String {
    query_wrapper_action_nodes(surface, scopes, queries, terms, candidates, quality)
        .into_iter()
        .find_map(|action| action.materialized_command())
        .map(|command| format!("nextCommand={command}\n"))
        .unwrap_or_else(|| "nextCommand=-\n".to_string())
}

fn query_wrapper_action_nodes(
    surface: QueryWrapperSurface,
    scopes: &[PathBuf],
    queries: &[String],
    terms: &[String],
    candidates: &[Candidate],
    quality: &QueryWrapperQuality,
) -> Vec<ActionNode> {
    let fd_query = fd_query_for_surface(surface, queries, terms);
    let multi_clause_queries = multi_clause_queries(queries, terms);
    let scope_label = scope_label(scopes);
    let command_scope = scope_args_for_command(scopes);
    let owner = owner_candidates(candidates)
        .into_iter()
        .next()
        .unwrap_or_else(|| "-".to_string());
    match surface {
        QueryWrapperSurface::Rg => {
            let fd_action = ActionNode {
                id: "A1".to_string(),
                kind: "fd-query".to_string(),
                suffix: "query-overlay-owner".to_string(),
                route: ActionRoute::FdQuery {
                    query: fd_query,
                    scope: scope_label.clone(),
                    command_scope: Some(command_scope.clone()),
                },
            };
            let rg_action = ActionNode {
                id: "A2".to_string(),
                kind: "multi-clause-rg-query".to_string(),
                suffix: "query-pack-refine".to_string(),
                route: ActionRoute::RgQuerySet {
                    queries: multi_clause_queries,
                    scope: scope_label,
                    command_scope,
                },
            };
            vec![fd_action, rg_action]
        }
        QueryWrapperSurface::Fd => {
            if allow_strong_owner_items(quality)
                && let Some(owner) = owner_items_candidate(candidates, terms)
                && let Some(language_id) = language_id_for_owner(&owner)
            {
                let owner_query = owner_items_query_for_owner(candidates, &owner, terms)
                    .unwrap_or_else(|| fd_query.clone());
                return vec![
                    ActionNode {
                        id: "A1".to_string(),
                        kind: "owner-items".to_string(),
                        suffix: "query-overlay-exact-owner".to_string(),
                        route: ActionRoute::OwnerItems {
                            language_id: language_id.to_string(),
                            owner,
                            query: owner_query,
                            scope: scope_label,
                        },
                    },
                    ActionNode {
                        id: "A2".to_string(),
                        kind: "scoped-rg-query".to_string(),
                        suffix: "query-overlay-content".to_string(),
                        route: ActionRoute::RgQuerySet {
                            queries: multi_clause_queries,
                            scope: command_scope.clone(),
                            command_scope,
                        },
                    },
                ];
            }
            let rg_scope = best_rg_scope(candidates).unwrap_or_else(|| command_scope.clone());
            vec![
                ActionNode {
                    id: "A1".to_string(),
                    kind: "scoped-rg-query".to_string(),
                    suffix: "query-overlay-content".to_string(),
                    route: ActionRoute::RgQuerySet {
                        queries: multi_clause_queries,
                        scope: rg_scope.clone(),
                        command_scope: shell_arg(&rg_scope),
                    },
                },
                ActionNode {
                    id: "A2".to_string(),
                    kind: "owner-items".to_string(),
                    suffix: "owner-items".to_string(),
                    route: ActionRoute::OwnerItemsHint { owner },
                },
            ]
        }
    }
}

fn allow_strong_owner_items(quality: &QueryWrapperQuality) -> bool {
    quality.package_cohesion != "low"
}

fn owner_items_candidate(candidates: &[Candidate], terms: &[String]) -> Option<String> {
    candidates
        .iter()
        .find(|candidate| owner_items_candidate_is_strong(candidate, terms))
        .map(|candidate| candidate.path.clone())
}

fn owner_items_query_for_owner(
    candidates: &[Candidate],
    owner: &str,
    terms: &[String],
) -> Option<String> {
    let evidence = candidates
        .iter()
        .filter(|candidate| candidate.path == owner)
        .map(owner_candidate_evidence)
        .collect::<Vec<_>>()
        .join(" ");
    if evidence.is_empty() {
        return None;
    }
    let path_terms = terms
        .iter()
        .filter(|term| !owner_items_protocol_term(term))
        .filter(|term| owner_items_term_matches_evidence(term, &evidence))
        .collect::<Vec<_>>();
    let semantic_terms = terms
        .iter()
        .filter(|term| !owner_items_protocol_term(term))
        .filter(|term| !owner_items_term_matches_evidence(term, &evidence))
        .collect::<Vec<_>>();
    let mut selected_terms = Vec::new();
    for term in &semantic_terms {
        for variant in owner_items_term_variants(term) {
            push_owner_items_query_term(&mut selected_terms, variant);
        }
    }
    for term in &semantic_terms {
        push_owner_items_query_term(&mut selected_terms, (*term).clone());
    }
    for term in path_terms {
        push_owner_items_query_term(&mut selected_terms, term.clone());
    }
    let selected_terms = selected_terms.into_iter().take(6).collect::<Vec<_>>();
    (!selected_terms.is_empty()).then(|| selected_terms.join("|"))
}

fn owner_candidate_evidence(candidate: &Candidate) -> String {
    format!("{} {} {}", candidate.path, candidate.symbol, candidate.text).to_ascii_lowercase()
}

fn owner_items_term_matches_evidence(term: &str, evidence: &str) -> bool {
    if evidence.contains(term) {
        return true;
    }
    owner_items_term_axes(term)
        .into_iter()
        .any(|axis| axis.len() >= 3 && evidence.contains(axis.as_str()))
}

fn owner_items_term_axes(term: &str) -> Vec<String> {
    let mut axes = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    for character in term.chars() {
        if !character.is_ascii_alphanumeric() {
            push_owner_items_axis(&mut axes, &mut current);
            previous = None;
            continue;
        }
        if character.is_ascii_uppercase()
            && previous
                .is_some_and(|previous| previous.is_ascii_lowercase() || previous.is_ascii_digit())
        {
            push_owner_items_axis(&mut axes, &mut current);
        }
        current.push(character.to_ascii_lowercase());
        previous = Some(character);
    }
    push_owner_items_axis(&mut axes, &mut current);
    axes
}

fn owner_items_term_variants(term: &str) -> Vec<String> {
    let lower = term.to_ascii_lowercase();
    let mut variants = Vec::new();
    if let Some(stem) = lower.strip_suffix("ing")
        && stem.len() >= 4
    {
        variants.push(format!("{stem}ed"));
    }
    variants
}

fn push_owner_items_query_term(terms: &mut Vec<String>, term: String) {
    if term.len() >= 3 && !terms.iter().any(|seen| seen == &term) {
        terms.push(term);
    }
}

fn push_owner_items_axis(axes: &mut Vec<String>, current: &mut String) {
    if current.len() >= 3 && !axes.iter().any(|axis| axis.as_str() == current.as_str()) {
        axes.push(current.clone());
    }
    current.clear();
}

fn owner_items_protocol_term(term: &str) -> bool {
    matches!(
        term.to_ascii_lowercase().as_str(),
        "query"
            | "search"
            | "fd"
            | "rg"
            | "owner"
            | "owners"
            | "owneritems"
            | "frontier"
            | "action"
            | "actions"
            | "command"
            | "commands"
            | "result"
            | "results"
    )
}

fn owner_items_candidate_is_strong(candidate: &Candidate, terms: &[String]) -> bool {
    if language_id_for_owner(&candidate.path).is_none() {
        return false;
    }
    if matches!(candidate.confidence.as_str(), "path-exact" | "path") {
        return true;
    }
    candidate.source == QUERY_OVERLAY_ROUTE_SOURCE
        && candidate.confidence == "likely"
        && (!secondary_like_owner(&candidate.path)
            || has_strong_secondary_owner_intent(terms.iter().map(String::as_str)))
}

fn language_id_for_owner(owner: &str) -> Option<&'static str> {
    if owner.ends_with(".rs") || owner.ends_with("Cargo.toml") {
        Some("rust")
    } else if owner.ends_with(".ts")
        || owner.ends_with(".tsx")
        || owner.ends_with(".js")
        || owner.ends_with(".jsx")
        || owner.ends_with("package.json")
        || owner.ends_with("tsconfig.json")
    {
        Some("typescript")
    } else if owner.ends_with(".py") || owner.ends_with("pyproject.toml") {
        Some("python")
    } else if owner.ends_with(".jl") || owner.ends_with("Project.toml") {
        Some("julia")
    } else if owner.ends_with(".ss")
        || owner.ends_with(".ssi")
        || owner.ends_with(".scm")
        || owner.ends_with(".sld")
        || owner.ends_with("gerbil.pkg")
        || owner.ends_with("build.ss")
    {
        Some("gerbil-scheme")
    } else {
        None
    }
}

fn query_wrapper_empty_next_command(
    surface: QueryWrapperSurface,
    scopes: &[PathBuf],
    queries: &[String],
    terms: &[String],
    reason: &str,
) -> String {
    let command_scope = scope_args_for_command(scopes);
    if reason == "query-too-broad" {
        let query = specific_budget_terms(queries, terms);
        return format!(
            "asp fd -query {} --workspace {}",
            shell_arg(&query),
            command_scope
        );
    }
    if reason == "source-index-miss" {
        return "asp cache source-index refresh".to_string();
    }
    match surface {
        QueryWrapperSurface::Fd => format!(
            "asp rg {} --workspace {}",
            repeated_query_args(&multi_clause_queries(queries, terms)),
            command_scope
        ),
        QueryWrapperSurface::Rg => {
            let fd_query = fd_query_for_surface(surface, queries, terms);
            format!(
                "asp fd -query {} --workspace {}",
                shell_arg(&fd_query),
                command_scope
            )
        }
    }
}

fn specific_budget_terms(queries: &[String], terms: &[String]) -> String {
    let raw_terms = raw_query_terms(queries);
    let selected = raw_terms
        .iter()
        .map(String::as_str)
        .chain(terms.iter().map(String::as_str))
        .filter(|term| specific_search_term(&term.to_ascii_lowercase()))
        .take(4)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        "path-or-symbol|error-code".to_string()
    } else {
        selected.join("|")
    }
}

fn fd_query_for_surface(
    surface: QueryWrapperSurface,
    queries: &[String],
    terms: &[String],
) -> String {
    match surface {
        QueryWrapperSurface::Rg => {
            let raw_terms = raw_query_terms(queries);
            if raw_terms.is_empty() {
                terms.join("|")
            } else {
                raw_terms.join("|")
            }
        }
        QueryWrapperSurface::Fd => terms.join("|"),
    }
}

fn raw_query_terms(queries: &[String]) -> Vec<String> {
    let mut raw_terms = Vec::new();
    for query in queries {
        for term in query.split(|character: char| {
            character == '|' || character == ',' || character.is_whitespace()
        }) {
            let term = term.trim();
            if term.is_empty()
                || raw_terms
                    .iter()
                    .any(|seen: &String| seen.eq_ignore_ascii_case(term))
            {
                continue;
            }
            raw_terms.push(term.to_string());
        }
    }
    raw_terms
}

fn multi_clause_queries(queries: &[String], terms: &[String]) -> Vec<String> {
    if queries.len() >= 2 {
        return queries.to_vec();
    }
    if terms.len() <= 1 {
        return terms.to_vec();
    }
    let midpoint = terms.len().div_ceil(2);
    vec![terms[..midpoint].join("|"), terms[midpoint..].join("|")]
}

fn repeated_query_args(queries: &[String]) -> String {
    queries
        .iter()
        .map(|query| format!("-query {}", shell_arg(query)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn scope_label(scopes: &[PathBuf]) -> String {
    if scopes.is_empty() {
        return ".".to_string();
    }
    scopes
        .iter()
        .map(|scope| scope.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join(",")
}

fn scope_args_for_command(scopes: &[PathBuf]) -> String {
    if scopes.is_empty() {
        return ".".to_string();
    }
    scopes
        .iter()
        .map(|scope| shell_arg_if_needed(&scope.to_string_lossy().replace('\\', "/")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn best_rg_scope(candidates: &[Candidate]) -> Option<String> {
    rg_scope_next(candidates).into_iter().next()
}

fn shell_arg_if_needed(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':'))
    {
        value.to_string()
    } else {
        shell_arg(value)
    }
}
