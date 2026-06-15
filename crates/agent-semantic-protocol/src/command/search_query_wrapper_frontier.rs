//! Low-confidence action frontier for query-wrapper recall probes.

use std::path::PathBuf;

use super::search_pipe_action_frontier::{ActionNode, ActionRoute, render_action_rows};
use super::search_pipe_model::Candidate;
use super::search_query_wrapper_candidates::{owner_candidates, rg_scope_next};
use super::search_query_wrapper_model::{QueryWrapperSurface, display_terms, shell_arg};

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
) {
    let fd_query = terms.join("|");
    let multi_clause_queries = multi_clause_queries(queries, terms);
    let evidence = evidence_preview(candidates);
    let owner = owner_candidates(candidates)
        .into_iter()
        .next()
        .unwrap_or_else(|| "-".to_string());
    println!("rankedEvidence={evidence}");
    println!("evidenceFrontier={evidence}");
    println!(
        "commandHandles=fdQuery={};rgQuery={};ownerItems={}",
        shell_arg(&fd_query),
        repeated_query_args(&multi_clause_queries),
        owner
    );
    let actions = query_wrapper_action_nodes(surface, scopes, queries, terms, candidates);
    print!("{}", render_action_rows(&actions));
    println!("reason=query-selector-low-confidence,clause-cohesion-required");
}

pub(super) fn print_query_wrapper_empty_receipt(
    surface: QueryWrapperSurface,
    scopes: &[PathBuf],
    queries: &[String],
    terms: &[String],
    source_trace: &str,
    avoid: &str,
) {
    println!("noOutput reason=no-candidates sourceTrace={source_trace}");
    println!(
        "nextCommand={}",
        query_wrapper_empty_next_command(surface, scopes, queries, terms)
    );
    println!("avoid={avoid}");
}

pub(super) fn query_wrapper_action_frontier(
    surface: QueryWrapperSurface,
    scopes: &[PathBuf],
    queries: &[String],
    terms: &[String],
    candidates: &[Candidate],
) -> Vec<serde_json::Value> {
    query_wrapper_action_nodes(surface, scopes, queries, terms, candidates)
        .into_iter()
        .map(|action| action.as_json())
        .collect()
}

pub(super) fn render_query_wrapper_action_frontier(
    surface: QueryWrapperSurface,
    scopes: &[PathBuf],
    queries: &[String],
    terms: &[String],
    candidates: &[Candidate],
) -> String {
    render_action_rows(&query_wrapper_action_nodes(
        surface, scopes, queries, terms, candidates,
    ))
}

fn query_wrapper_action_nodes(
    surface: QueryWrapperSurface,
    scopes: &[PathBuf],
    queries: &[String],
    terms: &[String],
    candidates: &[Candidate],
) -> Vec<ActionNode> {
    let fd_query = terms.join("|");
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
                suffix: "finder-owner".to_string(),
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
            if queries.len() > 1 {
                vec![
                    ActionNode {
                        id: "A1".to_string(),
                        ..rg_action
                    },
                    ActionNode {
                        id: "A2".to_string(),
                        ..fd_action
                    },
                ]
            } else {
                vec![fd_action, rg_action]
            }
        }
        QueryWrapperSurface::Fd => {
            if let Some(owner) = exact_owner_candidate(candidates)
                && let Some(language_id) = language_id_for_owner(&owner)
            {
                return vec![
                    ActionNode {
                        id: "A1".to_string(),
                        kind: "owner-items".to_string(),
                        suffix: "finder-exact-owner".to_string(),
                        route: ActionRoute::OwnerItems {
                            language_id: language_id.to_string(),
                            owner,
                            query: fd_query,
                            scope: scope_label,
                        },
                    },
                    ActionNode {
                        id: "A2".to_string(),
                        kind: "scoped-rg-query".to_string(),
                        suffix: "finder-content".to_string(),
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
                    suffix: "finder-content".to_string(),
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

fn exact_owner_candidate(candidates: &[Candidate]) -> Option<String> {
    candidates
        .iter()
        .find(|candidate| {
            matches!(candidate.confidence.as_str(), "path-exact" | "path")
                && language_id_for_owner(&candidate.path).is_some()
        })
        .map(|candidate| candidate.path.clone())
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
) -> String {
    let command_scope = scope_args_for_command(scopes);
    match surface {
        QueryWrapperSurface::Fd => format!(
            "asp rg {} {}",
            repeated_query_args(&multi_clause_queries(queries, terms)),
            command_scope
        ),
        QueryWrapperSurface::Rg => {
            format!(
                "asp fd -query {} {}",
                shell_arg(&terms.join("|")),
                command_scope
            )
        }
    }
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
        .map(|scope| shell_arg(&scope.to_string_lossy().replace('\\', "/")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn best_rg_scope(candidates: &[Candidate]) -> Option<String> {
    rg_scope_next(candidates).into_iter().next()
}

fn evidence_preview(candidates: &[Candidate]) -> String {
    let handles = candidates
        .iter()
        .take(8)
        .enumerate()
        .map(|(index, candidate)| format!("H{}:{}", index + 1, candidate.path))
        .collect::<Vec<_>>();
    display_terms(&handles)
}
