//! Graph-turbo view rendering for ASP-owned search pipelines.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::graph::{
    GraphTurboReceiptCapture, GraphTurboReceiptRequest, render_graph_turbo_packet,
    write_graph_turbo_receipt,
};
use super::search_pipe_graph_turbo::{GraphTurboSearchPipeRequest, render_graph_turbo_request};
use super::search_pipe_model::{Candidate, SearchPipeSourceTrace};
use super::search_pipe_plan::{
    SearchPipePlanRequest, render_search_pipe_decision_projection, render_search_pipe_plan,
};
use super::search_pipe_provider_facts::ProviderGraphFacts;
use super::search_pipe_quality::analyze_search_pipe_quality;
use super::search_pipe_render::render_ingest_frontier;
use super::search_pipe_seed_decision::SeedActionIntent;
use serde_json::Value;

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
    let display_candidates = if surface == "search-pipe" {
        normalize_candidates_for_scopes(project_root, locator_root, scopes, candidates)
    } else {
        candidates.to_vec()
    };
    let candidates = display_candidates.as_slice();
    match view {
        "graph-turbo-request" => {
            let request = render_graph_turbo_request(GraphTurboSearchPipeRequest {
                surface,
                language_id,
                dependency_root: locator_root,
                query,
                candidates,
                pipes,
                source,
                candidate_sources,
                source_trace,
                provider_facts,
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
        "seeds" => {
            let quality =
                query.map(|query| analyze_search_pipe_quality(language_id, query, candidates));
            if include_pipe_plan && let Some(query) = query {
                let quality = quality.as_ref().expect("quality is computed with query");
                print_search_pipe_header(SearchPipeHeader {
                    language_id,
                    project_root,
                    locator_root,
                    view: "seeds",
                    source,
                    query,
                    quality,
                    source_trace,
                });
            }
            let request = render_graph_turbo_request(GraphTurboSearchPipeRequest {
                surface,
                language_id,
                dependency_root: locator_root,
                query,
                candidates,
                pipes,
                source,
                candidate_sources,
                source_trace,
                provider_facts,
                read_memory_selectors,
                action_frontier: &[],
            })?;
            write_fast_search_frontier_receipt(
                frontier_receipt,
                language_id,
                query,
                request.as_bytes(),
            )?;
            let seed_action_intents = seed_action_intents(&request);
            if include_pipe_plan && let Some(seed_plan_line) = seed_plan_detail_line(&request) {
                println!("{seed_plan_line}");
            }
            let mut ranked_compact = None;
            if let Some(output) = render_graph_turbo_packet(request.as_bytes())? {
                ranked_compact = std::str::from_utf8(output.as_ref())
                    .ok()
                    .map(str::to_string);
                if include_pipe_plan {
                    if let Some(compact) = ranked_compact.as_deref() {
                        print!("{}", render_search_pipe_decision_projection(compact));
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
                        scopes,
                        query,
                        candidates,
                        ranked_compact: ranked_compact.as_deref(),
                        seed_action_intents: &seed_action_intents,
                    })
                );
            }
        }
        _ => {
            reject_non_graph_turbo_receipt(frontier_receipt)?;
            print!("{}", render_ingest_frontier(candidates, pipes));
            if include_pipe_plan && let Some(query) = query {
                let quality = analyze_search_pipe_quality(language_id, query, candidates);
                print_search_pipe_header(SearchPipeHeader {
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
                        ranked_compact: None,
                        seed_action_intents: &[],
                    })
                );
            }
        }
    }
    Ok(())
}

struct SearchPipeHeader<'a> {
    language_id: &'a str,
    project_root: &'a Path,
    locator_root: &'a Path,
    view: &'a str,
    source: &'a str,
    query: &'a str,
    quality: &'a super::search_pipe_quality::SearchPipeQuality,
    source_trace: &'a [SearchPipeSourceTrace],
}

fn print_search_pipe_header(header: SearchPipeHeader<'_>) {
    let SearchPipeHeader {
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
        "[search-pipe] lang={language_id} view={view} source={source} ranker=graph-turbo:owner-query"
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
    println!("nextClasses=fd-query,rg-query,owner-items,treesitter-query,query-selector");
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

fn seed_plan_detail_line(request: &str) -> Option<String> {
    let packet: Value = serde_json::from_str(request).ok()?;
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
    Some(format!(
        "seedPlanDetail=quality={quality} queryOwnerSeedCount={query_owner_seed_count} selectedSeedCount={selected_seed_count} riskFactors={risk_factors} recommendedActions={recommended_actions}"
    ))
}

fn seed_action_intents(request: &str) -> Vec<SeedActionIntent> {
    let Ok(packet) = serde_json::from_str::<Value>(request) else {
        return Vec::new();
    };
    packet
        .get("seedPlan")
        .and_then(|seed_plan| seed_plan.get("recommendedActions"))
        .and_then(Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .filter_map(Value::as_str)
                .filter(|action| !action.is_empty())
                .filter_map(SeedActionIntent::from_seed_plan_action)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
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
