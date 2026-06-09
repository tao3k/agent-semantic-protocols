//! Low-confidence action frontier for query-wrapper recall probes.

use std::path::PathBuf;

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
    let multi_clause_display = multi_clause_queries
        .iter()
        .enumerate()
        .map(|(index, query)| format!("C{}={}", index + 1, shell_arg(query)))
        .collect::<Vec<_>>()
        .join(";");
    let scope_label = scope_label(scopes);
    let command_scope = scope_args_for_command(scopes);
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
    match surface {
        QueryWrapperSurface::Rg => {
            println!("actionRank=A1,A2");
            println!("A1=fd-query(query={fd_query},scope={scope_label})!finder-owner");
            println!(
                "A2=multi-clause-rg-query(queryClauses={multi_clause_display},scope={scope_label})!query-pack-refine"
            );
            println!("actionFrontier=A1.fd-query,A2.multi-clause-rg-query");
            println!("recommendedNext=A1.fd-query");
            println!(
                "nextCommand=asp fd -query {} {command_scope}",
                shell_arg(&fd_query)
            );
        }
        QueryWrapperSurface::Fd => {
            let rg_scope = best_rg_scope(candidates).unwrap_or_else(|| command_scope.clone());
            println!("actionRank=A1,A2");
            println!(
                "A1=scoped-rg-query(queryClauses={multi_clause_display},scope={rg_scope})!finder-content"
            );
            println!("A2=owner-items(owner={owner})!owner-items");
            println!("actionFrontier=A1.scoped-rg-query,A2.owner-items");
            println!("recommendedNext=A1.scoped-rg-query");
            println!(
                "nextCommand=asp rg {} {}",
                repeated_query_args(&multi_clause_queries),
                shell_arg(&rg_scope)
            );
        }
    }
    println!("reason=query-selector-low-confidence,clause-cohesion-required");
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
