//! Graph-turbo view rendering for ASP-owned search pipelines.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::graph::{
    GraphTurboReceiptCapture, GraphTurboReceiptRequest, render_graph_turbo_packet,
    write_graph_turbo_receipt,
};
use super::search_pipe_graph_turbo::{GraphTurboSearchPipeRequest, render_graph_turbo_request};
use super::search_pipe_plan::{
    SearchPipePlanRequest, render_primary_frontier_actions_only, render_search_pipe_plan,
};
use super::search_pipe_provider_facts::ProviderGraphFacts;
use super::search_pipe_render::{Candidate, SearchPipeSourceTrace, render_ingest_frontier};

pub(super) struct SearchPipeViewRequest<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) surface: &'a str,
    pub(super) query: Option<&'a str>,
    pub(super) candidates: &'a [Candidate],
    pub(super) pipes: &'a [String],
    pub(super) source: &'a str,
    pub(super) candidate_sources: &'a [String],
    pub(super) source_trace: &'a [SearchPipeSourceTrace],
    pub(super) scopes: &'a [PathBuf],
    pub(super) view: &'a str,
    pub(super) include_pipe_plan: bool,
    pub(super) provider_facts: &'a ProviderGraphFacts,
    pub(super) read_memory_selectors: &'a [String],
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
}

pub(super) fn print_search_pipe_view(request: SearchPipeViewRequest<'_>) -> Result<(), String> {
    let SearchPipeViewRequest {
        language_id,
        project_root,
        locator_root,
        surface,
        query,
        candidates,
        pipes,
        source,
        candidate_sources,
        source_trace,
        scopes,
        view,
        include_pipe_plan,
        provider_facts,
        read_memory_selectors,
        frontier_receipt,
    } = request;
    match view {
        "graph-turbo-request" => {
            let request = render_graph_turbo_request(GraphTurboSearchPipeRequest {
                surface,
                language_id,
                query,
                candidates,
                pipes,
                source,
                candidate_sources,
                source_trace,
                provider_facts,
                read_memory_selectors,
            })?;
            write_fast_search_frontier_receipt(
                frontier_receipt,
                language_id,
                query,
                request.as_bytes(),
            )?;
            print!("{request}");
        }
        "seeds" => {
            if include_pipe_plan && let Some(query) = query {
                print_search_pipe_header(
                    language_id,
                    "seeds",
                    source,
                    query,
                    candidates,
                    source_trace,
                );
            }
            let request = render_graph_turbo_request(GraphTurboSearchPipeRequest {
                surface,
                language_id,
                query,
                candidates,
                pipes,
                source,
                candidate_sources,
                source_trace,
                provider_facts,
                read_memory_selectors,
            })?;
            write_fast_search_frontier_receipt(
                frontier_receipt,
                language_id,
                query,
                request.as_bytes(),
            )?;
            let mut ranked_compact = None;
            if let Some(output) = render_graph_turbo_packet(request.as_bytes())? {
                ranked_compact = std::str::from_utf8(output.as_ref())
                    .ok()
                    .map(str::to_string);
                if include_pipe_plan {
                    if let Some(compact) = ranked_compact.as_deref() {
                        print!("{}", render_primary_frontier_actions_only(compact));
                    } else {
                        io::stdout().write_all(output.as_ref()).map_err(|error| {
                            format!("failed to write asp-graph-turbo stdout: {error}")
                        })?;
                    }
                } else {
                    io::stdout().write_all(output.as_ref()).map_err(|error| {
                        format!("failed to write asp-graph-turbo stdout: {error}")
                    })?;
                }
            } else {
                print!("{}", render_ingest_frontier(candidates, pipes));
            }
            if include_pipe_plan && let Some(query) = query {
                print!(
                    "{}",
                    render_search_pipe_plan(SearchPipePlanRequest {
                        language_id,
                        project_root,
                        locator_root,
                        source,
                        scopes,
                        query,
                        candidates,
                        ranked_compact: ranked_compact.as_deref(),
                    })
                );
            }
        }
        _ => {
            reject_non_graph_turbo_receipt(frontier_receipt)?;
            print!("{}", render_ingest_frontier(candidates, pipes));
            if include_pipe_plan && let Some(query) = query {
                print_search_pipe_header(
                    language_id,
                    view,
                    source,
                    query,
                    candidates,
                    source_trace,
                );
                print!(
                    "{}",
                    render_search_pipe_plan(SearchPipePlanRequest {
                        language_id,
                        project_root,
                        locator_root,
                        source,
                        scopes,
                        query,
                        candidates,
                        ranked_compact: None,
                    })
                );
            }
        }
    }
    Ok(())
}

fn print_search_pipe_header(
    language_id: &str,
    view: &str,
    source: &str,
    query: &str,
    candidates: &[Candidate],
    source_trace: &[SearchPipeSourceTrace],
) {
    println!(
        "[search-pipe] lang={language_id} view={view} source={source} ranker=graph-turbo:owner-query"
    );
    println!("query={query}");
    println!(
        "queryTerms={}",
        display_terms(&query_terms_preserve_case(query))
    );
    println!("{}", coverage_line(query, candidates));
    println!("sourceTrace={}", compact_source_trace(source_trace));
    println!("{}", handles_line(query));
    println!("nextClasses=fd-query,rg-query,owner-items,query-selector");
}

fn coverage_line(query: &str, candidates: &[Candidate]) -> String {
    let terms = query_terms(query);
    if terms.is_empty() {
        return "coverage=matched=- missing=-".to_string();
    }
    let (matched, missing): (Vec<_>, Vec<_>) = terms.into_iter().partition(|term| {
        candidates
            .iter()
            .any(|candidate| candidate_matches(candidate, term))
    });
    format!(
        "coverage=matched={} missing={}",
        display_terms(&matched),
        display_terms(&missing)
    )
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .fold(Vec::new(), |mut terms, term| {
            if !terms.contains(&term) {
                terms.push(term);
            }
            terms
        })
}

fn query_terms_preserve_case(query: &str) -> Vec<String> {
    query
        .split(|character: char| character == ',' || character == '|' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(ToOwned::to_owned)
        .fold(Vec::new(), |mut terms, term| {
            if !terms.iter().any(|seen| seen == &term) {
                terms.push(term);
            }
            terms
        })
}

fn handles_line(query: &str) -> String {
    let terms = query_terms_preserve_case(query);
    let lowercase = terms
        .iter()
        .map(|term| term.to_ascii_lowercase())
        .collect::<Vec<_>>();
    format!(
        "handles=ownerTerms={} pathTerms={} symbolTerms={} testTerms={}",
        display_terms(&terms),
        display_terms(&lowercase),
        display_terms(&terms),
        display_terms(&lowercase),
    )
}

fn candidate_matches(candidate: &Candidate, term: &str) -> bool {
    candidate.symbol.to_ascii_lowercase().contains(term)
        || candidate.path.to_ascii_lowercase().contains(term)
        || candidate.text.to_ascii_lowercase().contains(term)
}

fn display_terms(terms: &[String]) -> String {
    if terms.is_empty() {
        "-".to_string()
    } else {
        terms.join(",")
    }
}

fn compact_source_trace(source_trace: &[SearchPipeSourceTrace]) -> String {
    if source_trace.is_empty() {
        return "-".to_string();
    }
    source_trace
        .iter()
        .map(SearchPipeSourceTrace::compact)
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn reject_non_graph_turbo_receipt(
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
) -> Result<(), String> {
    if frontier_receipt.is_some() {
        return Err(
            "--frontier-receipt-out is supported only for graph-turbo frontier search commands"
                .to_string(),
        );
    }
    Ok(())
}

fn write_fast_search_frontier_receipt(
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
    language_id: &str,
    query: Option<&str>,
    request_bytes: &[u8],
) -> Result<(), String> {
    let Some(receipt) = frontier_receipt else {
        return Ok(());
    };
    let token = frontier_receipt_token(language_id, query);
    write_graph_turbo_receipt(
        request_bytes,
        &GraphTurboReceiptCapture {
            out_path: receipt.out_path.as_path(),
            receipt_id: &format!("asp.search-frontier.{token}"),
            task_fingerprint: &format!("task:asp-fast-search:{token}"),
            command_fingerprint: &format!("command:asp-fast-search:{token}"),
            capture_source: "asp fast search",
            extra_args: &receipt.extra_args,
        },
    )
}

fn frontier_receipt_token(language_id: &str, query: Option<&str>) -> String {
    let mut token = format!("{}-{}", language_id, query.unwrap_or("frontier"))
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while token.contains("--") {
        token = token.replace("--", "-");
    }
    let token = token.trim_matches('-');
    if token.is_empty() {
        return "frontier".to_string();
    }
    token.chars().take(80).collect()
}
