//! ASP-owned `fd -query` and `rg -query` query-set wrappers.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_semantic_search::QUERY_OVERLAY_ROUTE_SOURCE;

use super::provider_roots::client_backend_cache_home;
use super::search_config::AspConfig;
use super::search_pipe_graph_turbo::{GraphTurboSearchPipeRequest, render_graph_turbo_request};
use super::search_pipe_model::{Candidate, SearchPipeSourceTrace};
use super::search_pipe_provider_facts::ProviderGraphFacts;
use super::search_pipe_surfaces::default_search_surfaces;
use super::search_query_budget::{
    SearchQueryBudgetBlock, search_rg_terms_budget_block, search_terms_budget_block,
};
use super::search_query_wrapper_candidates::{
    QueryCandidateCollection, QueryCandidateRequest, absolute_scope,
    collect_query_candidate_collection, owner_candidates, package_clusters, query_clauses,
    rg_scope_next, unique_clause_terms,
};
use super::search_query_wrapper_frontier::{
    print_query_wrapper_empty_receipt, print_query_wrapper_refinement_frontier, query_clauses_line,
    query_display, query_wrapper_action_frontier, render_query_wrapper_next_command,
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
    let cache_home = client_backend_cache_home(&invocation_root, &project_root)?;
    let config = AspConfig::load(&invocation_root, &project_root);
    let clauses = query_clauses(&wrapper_args.queries);
    let terms = unique_clause_terms(&clauses);
    let started_at = Instant::now();
    let collect_started_at = Instant::now();
    let broad_gate = query_wrapper_budget_gate(surface, &wrapper_args, &terms);
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
    let source_trace = candidate_collection.source_trace;
    let candidate_sources = candidate_collection.candidate_sources;
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
        cache_home: &cache_home,
        scopes: &wrapper_args.scopes,
        queries: &wrapper_args.queries,
        clauses: &clauses,
        terms: &terms,
        candidates: &candidates,
        quality: &quality,
        view: &wrapper_args.view,
        native_args: &wrapper_args.native_args,
        trace_fields,
        source_trace,
        candidate_sources,
        empty_reason: broad_gate
            .as_ref()
            .map(|gate| gate.reason)
            .unwrap_or("no-candidates"),
    })
}

fn query_wrapper_budget_gate(
    surface: QueryWrapperSurface,
    args: &QueryWrapperArgs,
    terms: &[String],
) -> Option<SearchQueryBudgetBlock> {
    if let Some(block) =
        search_terms_budget_block(terms, &args.scopes, !args.native_args.is_empty())
    {
        return Some(block);
    }
    if surface == QueryWrapperSurface::Rg
        && args.queries.len() == 1
        && let Some(block) =
            search_rg_terms_budget_block(terms, &args.scopes, !args.native_args.is_empty())
    {
        return Some(block);
    }
    None
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
                    .ok_or_else(|| "asp query-wrapper --view requires seeds".to_string())?
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
    cache_home: &'a Path,
    scopes: &'a [PathBuf],
    queries: &'a [String],
    clauses: &'a [QueryWrapperClause],
    terms: &'a [String],
    candidates: &'a [Candidate],
    quality: &'a QueryWrapperQuality,
    view: &'a str,
    native_args: &'a [String],
    trace_fields: std::collections::BTreeMap<String, serde_json::Value>,
    source_trace: Vec<SearchPipeSourceTrace>,
    candidate_sources: Vec<String>,
    empty_reason: &'a str,
}

fn print_query_wrapper_view(request: QueryWrapperViewRequest<'_>) -> Result<(), String> {
    let QueryWrapperViewRequest {
        surface,
        project_root,
        cache_home,
        scopes,
        queries,
        clauses,
        terms,
        candidates,
        quality,
        view,
        native_args,
        trace_fields,
        source_trace,
        candidate_sources,
        empty_reason,
    } = request;
    let language_id = "query-wrapper";
    let pipes = default_search_surfaces();
    let query = query_display(queries);
    let query_clause_texts = clauses
        .iter()
        .map(|clause| clause.raw.clone())
        .collect::<Vec<_>>();
    let source_trace = query_wrapper_source_trace(surface, candidates, source_trace, trace_fields);
    let source_label = candidate_sources
        .first()
        .map(String::as_str)
        .unwrap_or_else(|| surface.source_name());
    if view == "graph-turbo-request" {
        let action_frontier =
            query_wrapper_action_frontier(surface, scopes, queries, terms, candidates, quality);
        let config = AspConfig::load(project_root, project_root);
        let request = render_graph_turbo_request(GraphTurboSearchPipeRequest {
            surface: surface.graph_surface(),
            language_id,
            dependency_root: project_root,
            cache_home,
            query: Some(&query),
            query_clauses: &query_clause_texts,
            candidates,
            precomputed_quality: None,
            pipes: &pipes,
            source: source_label,
            candidate_sources: &candidate_sources,
            source_trace: &source_trace,
            provider_facts: &ProviderGraphFacts::default(),
            provider_context: None,
            config: &config,
            read_memory_selectors: &[],
            action_frontier: &action_frontier,
        })?;
        print!("{request}");
        return Ok(());
    }
    println!(
        "[search-{}] view=seeds querySet={} source={} ranker=graph-turbo:owner-query",
        surface.label(),
        terms.len(),
        source_label,
    );
    println!("query={query}");
    if candidates.is_empty() {
        let compact_source_trace = compact_source_trace(&source_trace);
        let empty_reason = query_wrapper_empty_reason(empty_reason, &source_trace);
        print_query_wrapper_empty_receipt(
            surface,
            scopes,
            queries,
            terms,
            &compact_source_trace,
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
    println!("sourceTrace={}", compact_source_trace(&source_trace));
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
    } else {
        print!(
            "{}",
            render_query_wrapper_next_command(surface, scopes, queries, terms, candidates, quality)
        );
    }
    println!("nextClasses={}", surface.next_classes(quality));
    println!("avoid={}", surface.avoid(quality));
    Ok(())
}

fn query_wrapper_source_trace(
    surface: QueryWrapperSurface,
    candidates: &[Candidate],
    mut source_trace: Vec<SearchPipeSourceTrace>,
    trace_fields: std::collections::BTreeMap<String, serde_json::Value>,
) -> Vec<SearchPipeSourceTrace> {
    if source_trace.is_empty() {
        return vec![
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
    }
    if let Some(query_overlay) = source_trace
        .iter_mut()
        .find(|trace| trace.source == QUERY_OVERLAY_ROUTE_SOURCE)
    {
        query_overlay.fields.extend(trace_fields);
    } else if let Some(first) = source_trace.first_mut() {
        first.fields.extend(trace_fields);
    }
    source_trace
}

fn compact_source_trace(source_trace: &[SearchPipeSourceTrace]) -> String {
    source_trace
        .iter()
        .map(SearchPipeSourceTrace::compact)
        .collect::<Vec<_>>()
        .join(",")
}

fn query_wrapper_empty_reason<'a>(
    fallback: &'a str,
    source_trace: &[SearchPipeSourceTrace],
) -> &'a str {
    if source_trace
        .iter()
        .any(|trace| trace.source == "sourceIndex" && trace.status == "miss")
    {
        "source-index-miss"
    } else {
        fallback
    }
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
