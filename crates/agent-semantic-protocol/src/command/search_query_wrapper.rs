//! ASP-owned `fd -query` and `rg -query` query-set wrappers.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::graph::render_graph_turbo_packet;
use super::search_config::AspConfig;
use super::search_pipe_graph_turbo::{GraphTurboSearchPipeRequest, render_graph_turbo_request};
use super::search_pipe_model::{Candidate, SearchPipeSourceTrace};
use super::search_pipe_plan::render_primary_frontier_actions_only;
use super::search_pipe_provider_facts::ProviderGraphFacts;
use super::search_pipe_render::render_ingest_frontier;
use super::search_pipe_surfaces::default_search_surfaces;
use super::search_query_budget::{SearchQueryBudgetBlock, search_terms_budget_block};
use super::search_query_wrapper_candidates::{
    QueryCandidateCollection, QueryCandidateRequest, absolute_scope,
    collect_query_candidate_collection, infer_language_id, owner_candidates, package_clusters,
    query_clauses, rg_scope_next, unique_clause_terms,
};
use super::search_query_wrapper_frontier::{
    print_query_wrapper_empty_receipt, print_query_wrapper_refinement_frontier, query_clauses_line,
    query_display, query_wrapper_action_frontier, render_query_wrapper_action_frontier,
};
use super::search_query_wrapper_model::{
    QueryWrapperClause, QueryWrapperQuality, QueryWrapperSurface, display_terms,
};
use super::search_query_wrapper_quality::analyze_query_wrapper_quality;

#[derive(Debug, Eq, PartialEq)]
struct QueryWrapperArgs {
    queries: Vec<String>,
    scopes: Vec<PathBuf>,
    view: String,
    native_args: Vec<String>,
}

pub(crate) fn is_query_wrapper(command: &str) -> bool {
    QueryWrapperSurface::from_command(command).is_some()
}

pub(crate) fn run_query_wrapper_command(command: &str, args: &[String]) -> Result<(), String> {
    let surface = QueryWrapperSurface::from_command(command)
        .ok_or_else(|| format!("unsupported query wrapper `{command}`"))?;
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!("{}", query_wrapper_usage(surface));
        return Ok(());
    }
    let wrapper_args = parse_query_wrapper_args(surface, args)?;
    let invocation_root =
        std::env::current_dir().map_err(|error| format!("failed to read cwd: {error}"))?;
    let project_root = wrapper_args
        .scopes
        .first()
        .map(|scope| absolute_scope(&invocation_root, scope))
        .unwrap_or_else(|| invocation_root.clone());
    let config = AspConfig::load(&invocation_root, &project_root);
    let clauses = query_clauses(&wrapper_args.queries);
    let terms = unique_clause_terms(&clauses);
    let started_at = Instant::now();
    let collect_started_at = Instant::now();
    let broad_gate = query_wrapper_budget_gate(&wrapper_args, &terms);
    let candidate_collection = if let Some(gate) = broad_gate.as_ref() {
        QueryCandidateCollection::blocked(gate)
    } else {
        collect_query_candidate_collection(QueryCandidateRequest {
            surface,
            project_root: &project_root,
            locator_root: &invocation_root,
            scopes: &wrapper_args.scopes,
            clauses: &clauses,
            terms: &terms,
            config: &config,
            native_args: &wrapper_args.native_args,
        })?
    };
    let collect_elapsed = collect_started_at.elapsed();
    let candidates = candidate_collection.candidates;
    let quality_started_at = Instant::now();
    let quality =
        analyze_query_wrapper_quality(&wrapper_args.scopes, &clauses, &terms, &candidates);
    let quality_elapsed = quality_started_at.elapsed();
    let mut trace_fields = candidate_collection.trace_fields;
    trace_fields.insert("collectMs".to_string(), duration_ms_value(collect_elapsed));
    trace_fields.insert("qualityMs".to_string(), duration_ms_value(quality_elapsed));
    trace_fields.insert(
        "elapsedMs".to_string(),
        duration_ms_value(started_at.elapsed()),
    );
    print_query_wrapper_view(QueryWrapperViewRequest {
        surface,
        project_root: &project_root,
        scopes: &wrapper_args.scopes,
        queries: &wrapper_args.queries,
        clauses: &clauses,
        terms: &terms,
        candidates: &candidates,
        quality: &quality,
        view: &wrapper_args.view,
        native_args: &wrapper_args.native_args,
        trace_fields,
        empty_reason: broad_gate
            .as_ref()
            .map(|gate| gate.reason)
            .unwrap_or("no-candidates"),
    })
}

fn query_wrapper_budget_gate(
    args: &QueryWrapperArgs,
    terms: &[String],
) -> Option<SearchQueryBudgetBlock> {
    search_terms_budget_block(terms, &args.scopes, !args.native_args.is_empty())
}

fn duration_ms_value(duration: Duration) -> serde_json::Value {
    serde_json::Value::from(duration.as_millis().min(u128::from(u64::MAX)) as u64)
}

fn query_wrapper_usage(surface: QueryWrapperSurface) -> String {
    match surface {
        QueryWrapperSurface::Fd => {
            "usage: asp fd -query <owner-or-path-term-a|term-b|term-c> [-query <second-clause>] [--workspace <root>] [scope...] [-- native-fd-argv...]\n\nFinds owner/path/module candidates from repeatable LLM-generated query clauses.".to_string()
        }
        QueryWrapperSurface::Rg => {
            "usage: asp rg -query <content-or-error-term-a|term-b|term-c> [-query <second-clause>] [--workspace <root>] [scope...] [-- native-rg-argv...]\n\nFinds content/hot-block candidates from repeatable LLM-generated query clauses.".to_string()
        }
    }
}

fn parse_query_wrapper_args(
    surface: QueryWrapperSurface,
    args: &[String],
) -> Result<QueryWrapperArgs, String> {
    let mut queries = Vec::new();
    let mut scopes = Vec::new();
    let mut view = "seeds".to_string();
    let mut native_args = Vec::new();
    let mut index = 0;
    let mut native = false;
    while index < args.len() {
        let arg = &args[index];
        if native {
            native_args.push(arg.clone());
            index += 1;
            continue;
        }
        match arg.as_str() {
            "--" => {
                native = true;
                index += 1;
            }
            "-query" | "--query" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("asp {} -query requires a value", surface.label()))?;
                queries.push(value.clone());
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            "--workspace" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--workspace requires a project root".to_string())?;
                if value.starts_with('-') {
                    return Err("--workspace requires a project root".to_string());
                }
                scopes.push(PathBuf::from(value));
                index += 2;
            }
            value if value.starts_with("-query=") => {
                queries.push(value.trim_start_matches("-query=").to_string());
                index += 1;
            }
            value if value.starts_with("--query=") => {
                queries.push(value.trim_start_matches("--query=").to_string());
                index += 1;
            }
            value if value.starts_with("--workspace=") => {
                let value = value.trim_start_matches("--workspace=");
                if value.is_empty() {
                    return Err("--workspace requires a project root".to_string());
                }
                scopes.push(PathBuf::from(value));
                index += 1;
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "unknown asp {} option: {value} (native flags must follow --)",
                    surface.label()
                ));
            }
            value => {
                scopes.push(PathBuf::from(value));
                index += 1;
            }
        }
    }
    if !matches!(view.as_str(), "seeds" | "graph-turbo-request") {
        return Err(format!(
            "asp {} -query supports --view seeds or --view graph-turbo-request",
            surface.label()
        ));
    }
    if queries.is_empty() {
        return Err(format!(
            "asp {} requires -query <query-clause>",
            surface.label()
        ));
    }
    Ok(QueryWrapperArgs {
        queries,
        scopes,
        view,
        native_args,
    })
}

struct QueryWrapperViewRequest<'a> {
    surface: QueryWrapperSurface,
    project_root: &'a Path,
    scopes: &'a [PathBuf],
    queries: &'a [String],
    clauses: &'a [QueryWrapperClause],
    terms: &'a [String],
    candidates: &'a [Candidate],
    quality: &'a QueryWrapperQuality,
    view: &'a str,
    native_args: &'a [String],
    trace_fields: std::collections::BTreeMap<String, serde_json::Value>,
    empty_reason: &'a str,
}

fn print_query_wrapper_view(request: QueryWrapperViewRequest<'_>) -> Result<(), String> {
    let QueryWrapperViewRequest {
        surface,
        project_root,
        scopes,
        queries,
        clauses,
        terms,
        candidates,
        quality,
        view,
        native_args,
        trace_fields,
        empty_reason,
    } = request;
    let language_id = infer_language_id(project_root);
    let pipes = default_search_surfaces();
    let query = query_display(queries);
    let query_clause_texts = clauses
        .iter()
        .map(|clause| clause.raw.clone())
        .collect::<Vec<_>>();
    let source_trace = vec![
        SearchPipeSourceTrace::new(
            surface.source_name(),
            if candidates.is_empty() {
                "empty"
            } else {
                "used"
            },
            candidates.len(),
            usize::from(candidates.is_empty()),
            candidates.len(),
        )
        .with_fields(trace_fields),
    ];
    let action_frontier =
        query_wrapper_action_frontier(surface, scopes, queries, terms, candidates, quality);
    let request = render_graph_turbo_request(GraphTurboSearchPipeRequest {
        surface: surface.graph_surface(),
        language_id,
        dependency_root: project_root,
        query: Some(&query),
        query_clauses: &query_clause_texts,
        candidates,
        pipes: &pipes,
        source: "finder",
        candidate_sources: &["finder".to_string()],
        source_trace: &source_trace,
        provider_facts: &ProviderGraphFacts::default(),
        read_memory_selectors: &[],
        action_frontier: &action_frontier,
    })?;
    if view == "graph-turbo-request" {
        print!("{request}");
        return Ok(());
    }
    println!(
        "[search-{}] view=seeds querySet={} source=finder ranker=graph-turbo:owner-query",
        surface.label(),
        terms.len(),
    );
    println!("query={query}");
    if candidates.is_empty() {
        print_query_wrapper_empty_receipt(
            surface,
            scopes,
            queries,
            terms,
            &source_trace[0].compact(),
            surface.avoid(quality),
            empty_reason,
        );
        return Ok(());
    }
    println!(
        "queryPack=clauses={} quality={} reason={}",
        clauses.len(),
        quality.query_pack_quality,
        if quality.risks.is_empty() {
            "ok".to_string()
        } else {
            display_terms(&quality.risks)
        }
    );
    println!("queryClauses={}", query_clauses_line(clauses));
    println!("terms={}", display_terms(terms));
    println!("scopeQuality={}", quality.scope_quality);
    for coverage in &quality.clause_coverages {
        println!(
            "clauseCoverage=C{} matched={} missing={}",
            coverage.id,
            display_terms(&coverage.matched),
            display_terms(&coverage.missing)
        );
    }
    println!(
        "packageCohesion={} packages={}",
        quality.package_cohesion,
        display_terms(&quality.packages)
    );
    if !quality.noise.is_empty() {
        println!("noise=paths={}", display_terms(&quality.noise));
    }
    if !quality.risks.is_empty() {
        println!("risk={}", display_terms(&quality.risks));
    }
    println!("sourceTrace={}", source_trace[0].compact());
    if surface == QueryWrapperSurface::Fd {
        println!(
            "ownerCandidates={}",
            display_terms(&owner_candidates(candidates))
        );
        println!(
            "packageClusters={}",
            display_terms(&package_clusters(candidates))
        );
        println!("parserIndexNext=owner-items");
        println!("rgScopeNext={}", display_terms(&rg_scope_next(candidates)));
    }
    if !native_args.is_empty() {
        println!("nativeArgs=pass-through count={}", native_args.len());
    }
    if !quality.allow_query_selector
        && (quality.query_pack_quality == "low" || has_exact_owner_candidate(surface, candidates))
    {
        print_query_wrapper_refinement_frontier(
            surface, scopes, queries, terms, candidates, quality,
        );
    } else if let Some(output) = render_graph_turbo_packet(request.as_bytes())? {
        if let Ok(compact) = std::str::from_utf8(output.as_ref()) {
            print!("{}", render_primary_frontier_actions_only(compact));
            print!(
                "{}",
                render_query_wrapper_action_frontier(
                    surface, scopes, queries, terms, candidates, quality
                )
            );
        } else {
            io::stdout()
                .write_all(output.as_ref())
                .map_err(|error| format!("failed to write asp-graph-turbo stdout: {error}"))?;
        }
    } else {
        print!("{}", render_ingest_frontier(candidates, &pipes));
    }
    println!("nextClasses={}", surface.next_classes(quality));
    println!("avoid={}", surface.avoid(quality));
    Ok(())
}

fn has_exact_owner_candidate(surface: QueryWrapperSurface, candidates: &[Candidate]) -> bool {
    surface == QueryWrapperSurface::Fd
        && candidates.iter().any(|candidate| {
            matches!(candidate.confidence.as_str(), "path-exact" | "path")
                && candidate
                    .path
                    .rsplit('/')
                    .next()
                    .is_some_and(|name| name.contains('.'))
        })
}
