//! Graph-turbo view rendering for ASP-owned search pipelines.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::graph::{
    GraphTurboReceiptCapture, GraphTurboReceiptRequest, render_graph_turbo_value_rust_compact,
    write_graph_turbo_receipt,
};
use super::search_config::AspConfig;
use super::search_pipe_dependency_facts::dependency_matches_query;
use super::search_pipe_graph_turbo::{
    GraphTurboSearchPipeRequest, graph_turbo_request, render_graph_turbo_request,
};
use super::search_pipe_model::{Candidate, SearchPipeSourceTrace};
use super::search_pipe_plan::{SearchPipePlanRequest, render_search_pipe_plan};
use super::search_pipe_provider_facts::{ProviderGraphFacts, ProviderGraphFactsContext};
use super::search_pipe_quality::analyze_search_pipe_quality;
use super::search_pipe_query_pack::query_clause_texts;
use super::search_pipe_render::render_ingest_frontier;
use serde_json::Value;

pub(super) struct SearchPipeViewRequest<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) cache_home: &'a Path,
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
    pub(super) provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    pub(super) config: &'a AspConfig,
    pub(super) read_memory_selectors: &'a [String],
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
}

pub(super) fn print_search_pipe_view(request: SearchPipeViewRequest<'_>) -> Result<(), String> {
    let SearchPipeViewRequest {
        language_id,
        project_root,
        locator_root,
        cache_home,
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
        provider_context,
        config,
        read_memory_selectors,
        frontier_receipt,
    } = request;
    let display_candidates = if surface == "search-pipe" {
        normalize_candidates_for_scopes(project_root, locator_root, scopes, candidates)
    } else {
        candidates.to_vec()
    };
    let candidates = display_candidates.as_slice();
    let graph_query_clauses = query
        .map(|query| {
            super::search_pipe_provider_facts::with_query_pack_descriptor(
                provider_context,
                |descriptor| query_clause_texts(language_id, query, descriptor),
            )
        })
        .unwrap_or_default();
    match view {
        "graph-turbo-request" => {
            let request = render_graph_turbo_request(GraphTurboSearchPipeRequest {
                surface,
                language_id,
                dependency_root: project_root,
                cache_home,
                query,
                query_clauses: &graph_query_clauses,
                candidates,
                precomputed_quality: None,
                pipes,
                source,
                candidate_sources,
                source_trace,
                provider_facts,
                provider_context,
                config,
                read_memory_selectors,
                action_frontier: &[],
            })?;
            write_fast_search_frontier_receipt(
                frontier_receipt,
                language_id,
                query,
                request.as_bytes(),
            )?;
            print!("{request}");
        }
        "seeds" => render_search_pipe_seeds_view(SearchPipeSeedsViewRequest {
            language_id,
            project_root,
            locator_root,
            cache_home,
            surface,
            query,
            candidates,
            pipes,
            source,
            candidate_sources,
            source_trace,
            scopes,
            include_pipe_plan,
            provider_facts,
            provider_context,
            config,
            read_memory_selectors,
            frontier_receipt,
            graph_query_clauses: &graph_query_clauses,
        })?,
        _ => {
            reject_non_graph_turbo_receipt(frontier_receipt)?;
            print!("{}", render_ingest_frontier(candidates, pipes));
            if include_pipe_plan && let Some(query) = query {
                let quality = super::search_pipe_provider_facts::with_query_pack_descriptor(
                    provider_context,
                    |descriptor| {
                        analyze_search_pipe_quality(language_id, query, candidates, descriptor)
                    },
                );
                print_search_pipe_header(SearchPipeHeader {
                    surface,
                    language_id,
                    project_root,
                    locator_root,
                    view,
                    source,
                    query,
                    quality: &quality,
                    source_trace,
                });
                print!(
                    "{}",
                    render_search_pipe_plan(SearchPipePlanRequest {
                        language_id,
                        project_root,
                        locator_root,
                        scopes,
                        query,
                        candidates,
                        precomputed_quality: Some(quality.clone()),
                        ranked_compact: None,
                        read_memory_selectors,
                        dependency_action_targets: &[],
                    })
                );
            }
        }
    }
    Ok(())
}

struct SearchPipeSeedsViewRequest<'a> {
    language_id: &'a str,
    project_root: &'a Path,
    locator_root: &'a Path,
    cache_home: &'a Path,
    surface: &'a str,
    query: Option<&'a str>,
    candidates: &'a [Candidate],
    pipes: &'a [String],
    source: &'a str,
    candidate_sources: &'a [String],
    source_trace: &'a [SearchPipeSourceTrace],
    scopes: &'a [PathBuf],
    include_pipe_plan: bool,
    provider_facts: &'a ProviderGraphFacts,
    provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    config: &'a AspConfig,
    read_memory_selectors: &'a [String],
    frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
    graph_query_clauses: &'a [String],
}

fn render_search_pipe_seeds_view(request: SearchPipeSeedsViewRequest<'_>) -> Result<(), String> {
    let render_started_at = Instant::now();
    let SearchPipeSeedsViewRequest {
        language_id,
        project_root,
        locator_root,
        cache_home,
        surface,
        query,
        candidates,
        pipes,
        source,
        candidate_sources,
        source_trace,
        scopes,
        include_pipe_plan,
        provider_facts,
        provider_context,
        config,
        read_memory_selectors,
        frontier_receipt,
        graph_query_clauses,
    } = request;
    let quality_started_at = Instant::now();
    let quality = query.map(|query| {
        super::search_pipe_provider_facts::with_query_pack_descriptor(
            provider_context,
            |descriptor| analyze_search_pipe_quality(language_id, query, candidates, descriptor),
        )
    });
    let quality_elapsed = quality_started_at.elapsed();
    let graph_started_at = Instant::now();
    let request_packet = graph_turbo_request(&GraphTurboSearchPipeRequest {
        surface,
        language_id,
        dependency_root: project_root,
        cache_home,
        query,
        query_clauses: graph_query_clauses,
        candidates,
        precomputed_quality: quality.as_ref(),
        pipes,
        source,
        candidate_sources,
        source_trace,
        provider_facts,
        provider_context,
        config,
        read_memory_selectors,
        action_frontier: &[],
    });
    let graph_elapsed = graph_started_at.elapsed();
    let receipt_started_at = Instant::now();
    if frontier_receipt.is_some() {
        let request_bytes = serde_json::to_vec(&request_packet)
            .map_err(|error| format!("failed to serialize graph turbo request: {error}"))?;
        write_fast_search_frontier_receipt(
            frontier_receipt,
            language_id,
            query,
            request_bytes.as_slice(),
        )?;
    }
    let receipt_elapsed = receipt_started_at.elapsed();
    let seed_started_at = Instant::now();
    let dependency_action_targets = dependency_action_targets_from_graph(&request_packet, query);
    let seed_plan_line = include_pipe_plan
        .then(|| seed_plan_detail_line(&request_packet))
        .flatten();
    let seed_elapsed = seed_started_at.elapsed();
    let compact_started_at = Instant::now();
    let output = render_graph_turbo_value_rust_compact(&request_packet)?;
    let compact_elapsed = compact_started_at.elapsed();
    let ranked_compact = std::str::from_utf8(output.as_ref())
        .ok()
        .map(str::to_string);
    let projection_elapsed = Duration::ZERO;
    let plan_started_at = Instant::now();
    let plan_output = if include_pipe_plan {
        query.map(|query| {
            render_search_pipe_plan(SearchPipePlanRequest {
                language_id,
                project_root,
                locator_root,
                scopes,
                query,
                candidates,
                precomputed_quality: quality.clone(),
                ranked_compact: ranked_compact.as_deref(),
                read_memory_selectors,
                dependency_action_targets: &dependency_action_targets,
            })
        })
    } else {
        None
    };
    let plan_elapsed = plan_started_at.elapsed();
    let should_print_header = include_pipe_plan || surface == "search-pipe";
    if should_print_header && let Some(query) = query {
        let quality = quality.as_ref().expect("quality is computed with query");
        let render_trace = render_phase_source_trace(
            source_trace,
            RenderPhaseTimings {
                total: render_started_at.elapsed(),
                quality: quality_elapsed,
                graph: graph_elapsed,
                receipt: receipt_elapsed,
                seed: seed_elapsed,
                compact: compact_elapsed,
                projection: projection_elapsed,
                plan: plan_elapsed,
            },
        );
        print_search_pipe_header(SearchPipeHeader {
            surface,
            language_id,
            project_root,
            locator_root,
            view: "seeds",
            source,
            query,
            quality,
            source_trace: &render_trace,
        });
    }
    if let Some(seed_plan_line) = seed_plan_line {
        println!("{seed_plan_line}");
    }
    if !include_pipe_plan {
        io::stdout()
            .write_all(output.as_ref())
            .map_err(|error| format!("failed to write graph compact stdout: {error}"))?;
    }
    if let Some(plan_output) = plan_output {
        print!("{plan_output}");
    }
    Ok(())
}

struct RenderPhaseTimings {
    total: Duration,
    quality: Duration,
    graph: Duration,
    receipt: Duration,
    seed: Duration,
    compact: Duration,
    projection: Duration,
    plan: Duration,
}

fn render_phase_source_trace(
    source_trace: &[SearchPipeSourceTrace],
    timings: RenderPhaseTimings,
) -> Vec<SearchPipeSourceTrace> {
    let mut trace = source_trace.to_vec();
    let mut fields = BTreeMap::new();
    let total_ms = elapsed_millis(timings.total);
    fields.insert("totalMs".to_string(), Value::from(total_ms));
    fields.insert("collectMs".to_string(), Value::from(total_ms));
    fields.insert(
        "qualityMs".to_string(),
        Value::from(elapsed_millis(timings.quality)),
    );
    fields.insert(
        "graphMs".to_string(),
        Value::from(elapsed_millis(timings.graph)),
    );
    fields.insert(
        "receiptMs".to_string(),
        Value::from(elapsed_millis(timings.receipt)),
    );
    fields.insert(
        "seedMs".to_string(),
        Value::from(elapsed_millis(timings.seed)),
    );
    fields.insert(
        "compactMs".to_string(),
        Value::from(elapsed_millis(timings.compact)),
    );
    fields.insert(
        "projectionMs".to_string(),
        Value::from(elapsed_millis(timings.projection)),
    );
    fields.insert(
        "planMs".to_string(),
        Value::from(elapsed_millis(timings.plan)),
    );
    trace.push(SearchPipeSourceTrace::new("render", "used", 0, 0, 0).with_fields(fields));
    trace
}

fn elapsed_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

struct SearchPipeHeader<'a> {
    surface: &'a str,
    language_id: &'a str,
    project_root: &'a Path,
    locator_root: &'a Path,
    view: &'a str,
    source: &'a str,
    query: &'a str,
    quality: &'a super::search_pipe_quality_model::SearchPipeQuality,
    source_trace: &'a [SearchPipeSourceTrace],
}

fn print_search_pipe_header(header: SearchPipeHeader<'_>) {
    let SearchPipeHeader {
        surface,
        language_id,
        project_root,
        locator_root,
        view,
        source,
        query,
        quality,
        source_trace,
    } = header;
    println!(
        "[{surface}] lang={language_id} view={view} source={source} ranker=graph-turbo:owner-query"
    );
    println!("query={query}");
    if let Some(workspace) = workspace_label(project_root, locator_root) {
        println!("workspace={workspace}");
    }
    println!(
        "queryPack=clauses={} quality={} raw={}",
        quality.clause_count,
        quality.query_pack_quality,
        shell_quote(query)
    );
    println!("{}", quality.query_terms_line(language_id, query));
    for line in quality.lines() {
        println!("{line}");
    }
    println!("sourceTrace={}", compact_source_trace(source_trace));
    println!("{}", quality.handles_line());
    println!("nextClasses=search-deps,owner-items,treesitter-query,query-selector");
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
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

fn seed_plan_detail_line(packet: &Value) -> Option<String> {
    let seed_plan = packet.get("seedPlan")?;
    let quality = seed_plan.get("seedQuality").and_then(Value::as_str)?;
    let query_owner_seed_count = seed_plan
        .get("queryOwnerSeedCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let selected_seed_count = seed_plan
        .get("selectedSeedCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let risk_factors = compact_string_array(seed_plan.get("riskFactors"));
    let recommended_actions = compact_string_array(seed_plan.get("recommendedActions"));
    let selection_policy = seed_plan.get("selectionPolicy");
    let flow = selection_policy
        .and_then(|policy| policy.get("flow"))
        .and_then(Value::as_str)
        .unwrap_or("-");
    let first_action_matches_evidence_state = compact_bool(
        selection_policy.and_then(|policy| policy.get("firstActionMatchesEvidenceState")),
    );
    let reasoning_tree_route_shown =
        compact_bool(selection_policy.and_then(|policy| policy.get("reasoningTreeRouteShown")));
    let chosen_route_preconditions_met =
        compact_bool(selection_policy.and_then(|policy| policy.get("chosenRoutePreconditionsMet")));
    let unnecessary_seed_count =
        compact_u64(selection_policy.and_then(|policy| policy.get("unnecessarySeedCount")));
    let seed_when_known_owner_count =
        compact_u64(selection_policy.and_then(|policy| policy.get("seedWhenKnownOwnerCount")));
    let seed_when_known_symbol_count =
        compact_u64(selection_policy.and_then(|policy| policy.get("seedWhenKnownSymbolCount")));
    let seed_when_known_selector_count =
        compact_u64(selection_policy.and_then(|policy| policy.get("seedWhenKnownSelectorCount")));
    Some(format!(
        "seedPlanDetail=quality={quality} queryOwnerSeedCount={query_owner_seed_count} selectedSeedCount={selected_seed_count} riskFactors={risk_factors} recommendedActions={recommended_actions} flow={flow} firstActionMatchesEvidenceState={first_action_matches_evidence_state} reasoningTreeRouteShown={reasoning_tree_route_shown} chosenRoutePreconditionsMet={chosen_route_preconditions_met} unnecessarySeedCount={unnecessary_seed_count} seedWhenKnownOwnerCount={seed_when_known_owner_count} seedWhenKnownSymbolCount={seed_when_known_symbol_count} seedWhenKnownSelectorCount={seed_when_known_selector_count}"
    ))
}

fn compact_string_array(value: Option<&Value>) -> String {
    let values = value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(",")
    }
}

fn compact_bool(value: Option<&Value>) -> &'static str {
    match value.and_then(Value::as_bool) {
        Some(true) => "true",
        Some(false) => "false",
        None => "-",
    }
}

fn compact_u64(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn dependency_action_targets_from_graph(packet: &Value, query: Option<&str>) -> Vec<String> {
    let Some(query) = query.filter(|query| !query.trim().is_empty()) else {
        return Vec::new();
    };
    let dependency_route_intent = query_has_dependency_route_intent(query);
    packet
        .get("graph")
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|node| node.get("kind").and_then(Value::as_str) == Some("dependency"))
        .filter_map(|node| node.get("value").and_then(Value::as_str))
        .filter(|dependency| {
            dependency_route_preconditions_met(dependency, query, dependency_route_intent)
        })
        .fold(Vec::new(), |mut targets, dependency| {
            if !targets.iter().any(|target| target == dependency) {
                targets.push(dependency.to_string());
            }
            targets
        })
}

fn dependency_route_preconditions_met(
    dependency: &str,
    query: &str,
    dependency_route_intent: bool,
) -> bool {
    if query_has_search_protocol_meta_intent(query) {
        return false;
    }
    dependency_literal_in_query(dependency, query)
        || (dependency_route_intent && dependency_matches_query(dependency, query))
}

fn dependency_literal_in_query(dependency: &str, query: &str) -> bool {
    let dependency = dependency.to_ascii_lowercase();
    query
        .split(token_boundary_for_dependency_literal)
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .any(|token| token == dependency)
}

fn query_has_dependency_route_intent(query: &str) -> bool {
    if query_has_search_protocol_meta_intent(query) {
        return false;
    }
    dependency_route_query_tokens(query).iter().any(|token| {
        matches!(
            token.as_str(),
            "cargo"
                | "crate"
                | "crates"
                | "dep"
                | "deps"
                | "dependencies"
                | "dependency"
                | "import"
                | "imports"
                | "manifest"
                | "npm"
                | "package"
                | "packages"
                | "pip"
                | "requirements"
                | "uv"
        )
    })
}

fn query_has_search_protocol_meta_intent(query: &str) -> bool {
    let tokens = dependency_route_query_tokens(query);
    let has_meta_term = tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "audit"
                | "conclusion"
                | "conclusions"
                | "evidence"
                | "expected"
                | "frontier"
                | "meta"
                | "not"
                | "plan"
                | "protocol"
                | "reasoning"
                | "router"
                | "routing"
                | "should"
                | "test"
                | "tests"
        )
    });
    let has_search_surface_term = tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "action"
                | "deps"
                | "line"
                | "owner"
                | "pipe"
                | "query"
                | "route"
                | "search"
                | "seed"
                | "selector"
                | "symbol"
        )
    });
    has_meta_term && has_search_surface_term
}

fn dependency_route_query_tokens(query: &str) -> Vec<String> {
    query
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn token_boundary_for_dependency_literal(character: char) -> bool {
    !(character == '-' || character == '_' || character == '.' || character.is_ascii_alphanumeric())
}

fn workspace_label(project_root: &Path, locator_root: &Path) -> Option<String> {
    if project_root == locator_root {
        return None;
    }
    let display = project_root
        .strip_prefix(locator_root)
        .map(slash_path)
        .unwrap_or_else(|_| slash_path(project_root));
    (!display.is_empty()).then_some(display)
}

fn normalize_candidates_for_scopes(
    project_root: &Path,
    locator_root: &Path,
    scopes: &[PathBuf],
    candidates: &[Candidate],
) -> Vec<Candidate> {
    let target_root = if let Some(scope) = scopes.first().filter(|_| scopes.len() == 1) {
        if scope.is_absolute() {
            scope.clone()
        } else {
            project_root.join(scope)
        }
    } else if project_root != locator_root {
        project_root.to_path_buf()
    } else {
        return candidates.to_vec();
    };
    let absolute_prefix = slash_path(&target_root);
    let locator_prefix = target_root
        .strip_prefix(locator_root)
        .map(slash_path)
        .unwrap_or_else(|_| absolute_prefix.clone());
    candidates
        .iter()
        .map(|candidate| {
            let mut candidate = candidate.clone();
            candidate.path = strip_scope_prefix(&candidate.path, &absolute_prefix)
                .or_else(|| strip_scope_prefix(&candidate.path, &locator_prefix))
                .unwrap_or(candidate.path);
            candidate
        })
        .collect()
}

fn strip_scope_prefix(path: &str, prefix: &str) -> Option<String> {
    let prefix = prefix.trim_end_matches('/');
    path.strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('/'))
        .map(ToOwned::to_owned)
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
